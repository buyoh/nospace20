# テスト戦略・実装計画

## テスト戦略

### ユニットテスト

各モジュールの単体テスト：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    // === 数値エンコードのテスト ===
    
    #[test]
    fn test_encode_number_positive() {
        let n = WsNumber(5);
        assert_eq!(
            n.encode(),
            vec![WsChar::Space, WsChar::Tab, WsChar::Space, WsChar::Tab, WsChar::Lf]
        );
    }
    
    #[test]
    fn test_encode_number_zero() {
        let n = WsNumber(0);
        assert_eq!(n.encode(), vec![WsChar::Space, WsChar::Lf]);
    }
    
    // === 命令エンコードのテスト ===
    
    #[test]
    fn test_encode_push() {
        let inst = Instruction::Push(WsNumber(1));
        let encoded = inst.encode();
        // SP SP SP TB LF (push 1)
        assert_eq!(encoded[0], WsChar::Space);
        assert_eq!(encoded[1], WsChar::Space);
    }
    
    #[test]
    fn test_encode_add() {
        let inst = Instruction::Add;
        assert_eq!(
            inst.encode(),
            vec![WsChar::Tab, WsChar::Space, WsChar::Space, WsChar::Space]
        );
    }
    
    // === ラベル管理のテスト ===
    
    #[test]
    fn test_label_allocator() {
        let mut alloc = LabelAllocator::new();
        let l1 = alloc.allocate();
        let l2 = alloc.allocate();
        assert_eq!(l1.0, 16);
        assert_eq!(l2.0, 17);
    }
}
```

### 統合テスト

既存のテストケースを使用：

```rust
// tests/compile_test.rs

use nospace20::{
    parse_to_tokens, parse_to_tree, syntactic_analyze,
    compile_to_whitespace, compile_to_whitespace_debug
};

#[test]
fn test_compile_simple() {
    let source = r#"
        func: main() {
            return: 42;
        }
    "#.to_string();
    
    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast);
    
    let result = compile_to_whitespace(&scope);
    assert!(result.is_ok());
    
    let ws_code = result.unwrap();
    // Whitespace コードが生成されていることを確認
    assert!(!ws_code.is_empty());
    // 使用されている文字が空白のみであることを確認
    assert!(ws_code.chars().all(|c| c == ' ' || c == '\t' || c == '\n'));
}
```

### Whitespace 実行テスト

外部の Whitespace インタプリタと連携：

```rust
#[test]
#[ignore] // 外部依存のため通常は無視
fn test_compile_and_run() {
    let source = r#"
        func: main() {
            __puti(42);
        }
    "#.to_string();
    
    // コンパイル
    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast);
    let ws_code = compile_to_whitespace(&scope).unwrap();
    
    // 一時ファイルに書き出し
    std::fs::write("/tmp/test.ws", &ws_code).unwrap();
    
    // Whitespace インタプリタで実行
    let output = std::process::Command::new("wspace")
        .arg("/tmp/test.ws")
        .output()
        .expect("Failed to run whitespace interpreter");
    
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42");
}
```

## 実装優先順位

### Phase 1: 基盤（1-2週間）

- [ ] Whitespace 命令定義 (`Instruction` enum)
- [ ] 数値エンコーダ (`WsNumber::encode`)
- [ ] 命令エンコーダ (`Instruction::encode`)
- [ ] プログラム構造 (`WsProgram`)
- [ ] ラベル管理 (`LabelAllocator`)
- [ ] メモリレイアウト定数

**成果物:** 空のプログラム（exit のみ）を生成できる

### Phase 2: 基本的なコード生成（2-3週間）

- [ ] 組み込みルーチン（ヘッダー/フッター）
- [ ] 変数アクセス（グローバル/ローカル）
- [ ] 算術演算 (+, -, *, /, %)
- [ ] リテラル値の評価

**成果物:** 単純な算術式を計算できる

### Phase 3: 制御構造（2週間）

- [ ] 比較演算子 (==, !=, <, <=, >, >=)
- [ ] 論理演算子 (&&, ||, !)
- [ ] if/else
- [ ] while

**成果物:** 条件分岐とループが動作する

### Phase 4: 関数（2週間）

- [ ] 関数定義
- [ ] 関数呼び出し
- [ ] 引数渡し
- [ ] スタックフレーム管理
- [ ] return 文

**成果物:** 再帰関数が動作する

### Phase 5: 高度な機能（2週間）

- [ ] I/O 関数 (__puti, __putc, __geti, __getc)
- [ ] 配列
- [ ] ポインタ

**成果物:** 完全なコンパイラ

### Phase 6: 最適化・改善（継続的）

- [ ] 不要な命令の削除
- [ ] 定数畳み込み
- [ ] if 条件の直接分岐最適化
- [ ] エラーメッセージの改善

## マイルストーン

| マイルストーン | 目標 | 期間 |
|---------------|------|------|
| M1 | 空プログラム生成 | Phase 1 完了 |
| M2 | `return: 42;` が動作 | Phase 2 完了 |
| M3 | FizzBuzz が動作 | Phase 3-4 完了 |
| M4 | 全テストケース通過 | Phase 5 完了 |

## 既存テストケースの活用

`resources/tests/` のテストケースを使用：

```rust
fn test_with_existing_testcase(name: &str) {
    let source = std::fs::read_to_string(format!("resources/tests/{}.ns", name)).unwrap();
    let check: serde_json::Value = 
        serde_json::from_str(&std::fs::read_to_string(format!("resources/tests/{}.check.json", name)).unwrap()).unwrap();
    
    // コンパイル
    let tokens = parse_to_tokens(&source).unwrap();
    let ast = parse_to_tree(&tokens).unwrap();
    let scope = syntactic_analyze(&ast);
    let ws_code = compile_to_whitespace(&scope).unwrap();
    
    // Whitespace で実行して結果を比較
    // ...
}
```

## 開発のヒント

1. **インタプリタの出力と比較** - 同じテストケースをインタプリタとコンパイラ両方で実行し、結果を比較

2. **デバッグ出力の活用** - `to_debug_string()` で生成されたコードを確認

3. **段階的な実装** - 最小限の機能から始めて徐々に拡張

4. **回帰テストの維持** - 新機能追加時に既存テストが壊れないことを確認
