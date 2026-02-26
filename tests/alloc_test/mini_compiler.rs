//! テスト用ミニコンパイラ
//!
//! テスト仕様を Whitespace プログラムにコンパイルする。

use std::collections::HashMap;

use nospace20::compiler_ws::alloc_runtime::{AllocRuntime, BumpAllocRuntime, FsbaFirstFitAllocRuntime};
use nospace20::compiler_ws::label::reserved_labels;
use nospace20::compiler_ws::memory::heap_layout;
use nospace20::compiler_ws::program::WsProgram;
use nospace20::whitespace::{Instruction, LabelId, WsNumber};

use super::test_spec::{AllocStep, AllocTestSpec};

/// テスト用のラベル管理（TEST_FAIL 用 + ループ用）
const LABEL_TEST_FAIL: LabelId = LabelId(14);

/// ミニコンパイラの状態
pub struct MiniCompiler {
    /// 変数名 → ヒープアドレスのマッピング
    var_map: HashMap<String, i64>,
    /// カウンタのヒープアドレス
    counter_addr: i64,
    /// 各変数の最後の alloc サイズ（load_print で使用）
    alloc_sizes: HashMap<String, i64>,
    /// 次に使えるラベル ID（ループ用）
    next_label: u32,
}

impl MiniCompiler {
    pub fn new(vars: &[String], global_heap_size: i64) -> Self {
        // ヒープレイアウト:
        //   heap[GLOBAL_PTR + 0 .. + global_heap_size]  ← ユーザー指定のグローバル変数領域
        //   heap[GLOBAL_PTR + global_heap_size]          ← カウンタ (初期値 1)
        //   heap[GLOBAL_PTR + global_heap_size + 1 .. ]  ← 変数ストレージ
        let counter_addr = heap_layout::GLOBAL_PTR + global_heap_size;
        let mut var_map = HashMap::new();
        for (i, name) in vars.iter().enumerate() {
            var_map.insert(name.clone(), counter_addr + 1 + i as i64);
        }

        Self {
            var_map,
            counter_addr,
            alloc_sizes: HashMap::new(),
            next_label: reserved_labels::LABEL_OFFSET as u32,
        }
    }

    /// テスト全体の effective global_heap_size を計算
    fn effective_global_size(&self) -> i64 {
        // 元の global_heap_size + 1(カウンタ) + var_count
        // counter_addr = GLOBAL_PTR + global_heap_size なので
        // effective_size = (counter_addr - GLOBAL_PTR) + 1 + var_count
        let var_count = self.var_map.len() as i64;
        (self.counter_addr - heap_layout::GLOBAL_PTR) + 1 + var_count
    }

    /// 新しいラベルを割り当て
    fn alloc_label(&mut self) -> LabelId {
        let id = self.next_label;
        self.next_label += 1;
        LabelId(id)
    }

    /// テスト仕様を WS プログラムにコンパイル
    pub fn compile(&mut self, spec: &AllocTestSpec) -> WsProgram {
        let bump_runtime = BumpAllocRuntime;
        let fsba_runtime = FsbaFirstFitAllocRuntime;
        let alloc_runtime: &dyn AllocRuntime = match spec.config.allocator.as_str() {
            "fsba" => &fsba_runtime,
            _ => &bump_runtime,
        };
        let mut prog = WsProgram::new();

        // 1. アロケータ初期化
        prog.append(alloc_runtime.generate_memory_init(self.effective_global_size()));

        // Note: FSBA uses ALLOC_HEAP_TOP instead of LOCAL_HEAP_END for bump pointer

        // 2. カウンタ初期化 (heap[counter_addr] = 1)
        prog.extend([
            Instruction::Push(WsNumber(self.counter_addr)),
            Instruction::Push(WsNumber(1)),
            Instruction::Store,
        ]);

        // 3. テスト操作を WS 命令に変換
        for step in &spec.steps {
            self.compile_step(&mut prog, step);
        }

        // 4. Exit
        prog.push(Instruction::Exit);

        // 5. サブルーチン定義
        prog.append(alloc_runtime.generate_subroutines());
        // Note: FSBA generates ~145 instructions for __rt_alloc + __rt_free

        // 6. テスト失敗ハンドラ
        //    __test_fail: Exit(異常終了 — VM の run が Error を返す代わりに
        //    ここでは不正な状態で Exit する。assert_var_ne の失敗時にジャンプ)
        //    方法: 不正なヒープアクセスでランタイムエラーを起こす
        //    → Whitespace の仕様上、Exit で正常終了してしまうので、
        //      代わりに「失敗マーカー」を stdout に出力してから Exit する
        prog.extend([
            Instruction::Label(LABEL_TEST_FAIL),
            // "ASSERTION_FAILED\n" を出力
            Instruction::Push(WsNumber(65)), // 'A'
            Instruction::OutputChar,
            Instruction::Push(WsNumber(70)), // 'F'
            Instruction::OutputChar,
            Instruction::Push(WsNumber(10)), // '\n'
            Instruction::OutputChar,
            Instruction::Exit,
        ]);

        prog
    }

    /// 単一ステップをコンパイル
    fn compile_step(&mut self, prog: &mut WsProgram, step: &AllocStep) {
        match step {
            AllocStep::Alloc { var, size } => {
                self.compile_alloc(prog, var, *size);
            }
            AllocStep::Free { var } => {
                self.compile_free(prog, var);
            }
            AllocStep::LoadPrint { var } => {
                self.compile_load_print(prog, var);
            }
            AllocStep::Print { value } => {
                self.compile_print(prog, *value);
            }
            AllocStep::AssertVarNe { var1, var2 } => {
                self.compile_assert_var_ne(prog, var1, var2);
            }
            AllocStep::HeapPrint { address } => {
                self.compile_heap_print(prog, *address);
            }
            AllocStep::Loop { count, body } => {
                self.compile_loop(prog, *count, body);
            }
        }
    }

    /// alloc 操作: __rt_alloc(size) → heap[var_addr] = ptr; 要素を counter で初期化
    fn compile_alloc(&mut self, prog: &mut WsProgram, var: &str, size: i64) {
        let var_addr = self.var_map[var];

        // __rt_alloc(size) → ptr がスタックトップ
        prog.extend([
            Instruction::Push(WsNumber(size)),
            Instruction::Call(reserved_labels::RT_ALLOC),
        ]);

        // heap[var_addr] = ptr (ptr はスタックに残す必要があるので Duplicate)
        prog.extend([
            Instruction::Duplicate,
            Instruction::Push(WsNumber(var_addr)),
            Instruction::Swap,
            Instruction::Store,
            // スタック: [ptr]
        ]);

        // 要素初期化: heap[ptr + i] = counter_val + i  (i = 0..size-1)
        // カウンタをロード
        // 各要素 i について展開
        for i in 0..size {
            if i == size - 1 {
                // 最後の要素: ptr を消費
                prog.extend([
                    // スタック: [ptr]
                    Instruction::Push(WsNumber(i)),
                    Instruction::Add,
                    // スタック: [ptr+i]
                    Instruction::Push(WsNumber(self.counter_addr)),
                    Instruction::Retrieve,
                    Instruction::Push(WsNumber(i)),
                    Instruction::Add,
                    // スタック: [ptr+i, counter+i]
                    Instruction::Store,
                ]);
            } else {
                // ptr を保持
                prog.extend([
                    // スタック: [ptr]
                    Instruction::Duplicate,
                    Instruction::Push(WsNumber(i)),
                    Instruction::Add,
                    // スタック: [ptr, ptr+i]
                    Instruction::Push(WsNumber(self.counter_addr)),
                    Instruction::Retrieve,
                    Instruction::Push(WsNumber(i)),
                    Instruction::Add,
                    // スタック: [ptr, ptr+i, counter+i]
                    Instruction::Store,
                    // スタック: [ptr]
                ]);
            }
        }

        // カウンタ更新: counter += size
        prog.extend([
            Instruction::Push(WsNumber(self.counter_addr)),
            Instruction::Push(WsNumber(self.counter_addr)),
            Instruction::Retrieve,
            Instruction::Push(WsNumber(size)),
            Instruction::Add,
            Instruction::Store,
        ]);

        // alloc サイズを記録 (load_print で使用)
        self.alloc_sizes.insert(var.to_string(), size);
    }

    /// free 操作: __rt_free(heap[var_addr])
    fn compile_free(&self, prog: &mut WsProgram, var: &str) {
        let var_addr = self.var_map[var];
        prog.extend([
            Instruction::Push(WsNumber(var_addr)),
            Instruction::Retrieve,
            Instruction::Call(reserved_labels::RT_FREE),
        ]);
    }

    /// load_print 操作: heap[ptr + i] を全要素出力
    fn compile_load_print(&self, prog: &mut WsProgram, var: &str) {
        let var_addr = self.var_map[var];
        let size = self.alloc_sizes.get(var).copied().unwrap_or(0);

        for i in 0..size {
            prog.extend([
                Instruction::Push(WsNumber(var_addr)),
                Instruction::Retrieve, // ptr
                Instruction::Push(WsNumber(i)),
                Instruction::Add,      // ptr + i
                Instruction::Retrieve, // heap[ptr + i]
                Instruction::OutputNumber,
                Instruction::Push(WsNumber(10)),
                Instruction::OutputChar, // '\n'
            ]);
        }
    }

    /// print 操作: 即値を出力
    fn compile_print(&self, prog: &mut WsProgram, value: i64) {
        prog.extend([
            Instruction::Push(WsNumber(value)),
            Instruction::OutputNumber,
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
        ]);
    }

    /// assert_var_ne 操作: heap[var1] != heap[var2] でなければ __test_fail
    fn compile_assert_var_ne(&self, prog: &mut WsProgram, var1: &str, var2: &str) {
        let addr1 = self.var_map[var1];
        let addr2 = self.var_map[var2];
        prog.extend([
            Instruction::Push(WsNumber(addr1)),
            Instruction::Retrieve,
            Instruction::Push(WsNumber(addr2)),
            Instruction::Retrieve,
            Instruction::Sub,
            Instruction::JumpIfZero(LABEL_TEST_FAIL),
        ]);
    }

    /// heap_print 操作: heap[address] を出力
    fn compile_heap_print(&self, prog: &mut WsProgram, address: i64) {
        prog.extend([
            Instruction::Push(WsNumber(address)),
            Instruction::Retrieve,
            Instruction::OutputNumber,
            Instruction::Push(WsNumber(10)),
            Instruction::OutputChar,
        ]);
    }

    /// loop 操作: body を count 回繰り返し
    fn compile_loop(&mut self, prog: &mut WsProgram, count: i64, body: &[AllocStep]) {
        // ループカウンタはスタック上で管理
        let label_loop_start = self.alloc_label();
        let label_loop_end = self.alloc_label();

        // カウンタ = count をスタックに push
        prog.push(Instruction::Push(WsNumber(count)));

        // ループ開始
        prog.push(Instruction::Label(label_loop_start));

        // カウンタが 0 なら終了
        prog.extend([
            Instruction::Duplicate,
            Instruction::JumpIfZero(label_loop_end),
        ]);

        // body を実行
        for step in body {
            self.compile_step(prog, step);
        }

        // カウンタ--
        prog.extend([
            Instruction::Push(WsNumber(1)),
            Instruction::Sub,
            Instruction::Jump(label_loop_start),
        ]);

        // ループ終了: カウンタをスタックから除去
        prog.extend([
            Instruction::Label(label_loop_end),
            Instruction::Discard,
        ]);
    }
}
