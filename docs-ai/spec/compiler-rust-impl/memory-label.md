# メモリレイアウト・ラベル管理

## アドレス抽象化

### 設計方針

Whitespace のヒープアドレスを直接 `i64` で扱うのではなく、専用の型で抽象化します。
これにより型安全性を確保しつつ、`Debug` トレイトで実際の値も確認できるようにします。

```rust
/// ヒープアドレス
/// 
/// Whitespace のヒープ上のアドレスを表す。
/// Debug 出力で実際の数値も確認可能。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeapAddress(pub i64);

impl HeapAddress {
    pub const fn new(addr: i64) -> Self {
        Self(addr)
    }
    
    /// アドレス値を取得（Whitespace 命令生成用）
    pub fn value(&self) -> i64 {
        self.0
    }
    
    /// オフセットを加算した新しいアドレスを返す
    pub fn offset(&self, n: i64) -> Self {
        Self(self.0 + n)
    }
}

impl std::fmt::Debug for HeapAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HeapAddr({})", self.0)
    }
}

impl std::fmt::Display for HeapAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.0)
    }
}
```

### メリット

- **型安全**: `HeapAddress` と `LabelId` を混同するバグをコンパイル時に検出
- **デバッグ可能**: `Debug` で `HeapAddr(8)` のように実際の値が見える
- **意図明確**: コードを読む際に「これはアドレスである」ことが明白

## メモリレイアウト

旧実装のメモリ配置を踏襲します。

### MemoryLayout 構造体

```rust
/// メモリレイアウト管理
/// 
/// Whitespace ヒープの予約領域と変数配置を管理する。
pub struct MemoryLayout {
    /// グローバル変数の数
    global_var_count: i64,
}

impl MemoryLayout {
    /// 新しいメモリレイアウトを作成
    pub fn new() -> Self {
        Self { global_var_count: 0 }
    }
    
    // === 予約アドレス（定数） ===
    
    /// ローカルヒープの開始位置を格納するアドレス
    pub const LOCAL_HEAP_BEGIN: HeapAddress = HeapAddress(2);
    
    /// ローカルヒープの終了位置を格納するアドレス
    pub const LOCAL_HEAP_END: HeapAddress = HeapAddress(3);
    
    /// 一時ポインタ（内部使用）
    pub const TEMP_PTR: HeapAddress = HeapAddress(4);
    
    /// グローバル変数領域の開始アドレス
    pub const GLOBAL_PTR: HeapAddress = HeapAddress(8);
    
    // === 動的アドレス計算 ===
    
    /// グローバル変数を登録し、そのアドレスを返す
    pub fn allocate_global(&mut self) -> HeapAddress {
        let addr = Self::GLOBAL_PTR.offset(self.global_var_count);
        self.global_var_count += 1;
        addr
    }
    
    /// グローバル変数領域のサイズを取得
    pub fn global_size(&self) -> i64 {
        self.global_var_count
    }
    
    /// ローカルヒープ初期値（global領域の直後）
    pub fn initial_local_heap(&self) -> HeapAddress {
        Self::GLOBAL_PTR.offset(self.global_var_count)
    }
}
```

### 後方互換エイリアス

```rust
/// 後方互換性のための定数エイリアス
pub mod heap_layout {
    use super::*;
    pub const LOCAL_HEAP_BEGIN: i64 = MemoryLayout::LOCAL_HEAP_BEGIN.0;
    pub const LOCAL_HEAP_END: i64 = MemoryLayout::LOCAL_HEAP_END.0;
    pub const TEMP_PTR: i64 = MemoryLayout::TEMP_PTR.0;
    pub const GLOBAL_PTR: i64 = MemoryLayout::GLOBAL_PTR.0;
}
```

### メモリマップ

```
アドレス 0   : 予約（無効アドレス）― フリーリストの「空」を表すセンチネル値。
               言語仕様としてアドレス 0 は変数や __alloc のアドレスとして使用されない。
               アドレス 0 への書き込みは未定義動作。
アドレス 1   : 予約（未使用）
アドレス 2   : LocalHeapBegin（現在のローカルスコープ開始位置）
アドレス 3   : LocalHeapEnd（現在のローカルスコープ終了位置）
アドレス 4-7 : 一時作業領域 (TempPtr) / アロケータ内部 (AllocFreeHead=5, AllocHeapTop=6, FsbaTablePtr=7)
アドレス 8+  : グローバル変数領域 (GlobalPtr)

[ローカル変数領域はグローバル領域の後ろに動的に確保]
```

## 予約ラベル

### ラベル定数

```rust
/// 予約ラベル定義
pub mod reserved_labels {
    use super::LabelId;
    
    /// ユーザーコード開始点
    pub const USER_CODE_BEGIN: LabelId = LabelId(0);
    
    /// ゼロ判定ルーチン
    pub const COMPARATOR_ZERO: LabelId = LabelId(2);
    pub const COMPARATOR_ZERO_2: LabelId = LabelId(3);
    
    /// 負数判定ルーチン
    pub const COMPARATOR_NEGATIVE: LabelId = LabelId(4);
    pub const COMPARATOR_NEGATIVE_2: LabelId = LabelId(5);
    
    /// AND ルーチン
    pub const COMPARATOR_AND: LabelId = LabelId(6);
    pub const COMPARATOR_AND_2: LabelId = LabelId(7);
    
    /// OR ルーチン
    pub const COMPARATOR_OR: LabelId = LabelId(8);
    pub const COMPARATOR_OR_2: LabelId = LabelId(9);
    pub const COMPARATOR_OR_3: LabelId = LabelId(10);
    
    /// ユーザーラベルのオフセット
    pub const LABEL_OFFSET: u32 = 16;
}
```

### ラベルマップ

```
ラベル 0     : USER_CODE_BEGIN
ラベル 2-3   : ゼロ判定ルーチン
ラベル 4-5   : 負数判定ルーチン
ラベル 6-7   : AND ルーチン
ラベル 8-10  : OR ルーチン
ラベル 16+   : ユーザーコード（関数、制御構造）
```

## LabelAllocator - ラベル管理

```rust
use std::collections::HashMap;

/// ラベル管理器
#[derive(Debug)]
pub struct LabelAllocator {
    /// 次に割り当てるラベルID
    next_id: u32,
    /// 関数名 → ラベルID のマッピング
    function_labels: HashMap<String, LabelId>,
}

impl LabelAllocator {
    pub fn new() -> Self {
        Self {
            next_id: reserved_labels::LABEL_OFFSET,
            function_labels: HashMap::new(),
        }
    }
    
    /// 新しいラベルを確保 (制御構造用)
    pub fn allocate(&mut self) -> LabelId {
        let id = LabelId(self.next_id);
        self.next_id += 1;
        id
    }
    
    /// 連続したラベルを確保
    /// 返り値は範囲の先頭ラベル
    pub fn allocate_range(&mut self, count: u32) -> LabelId {
        let base = LabelId(self.next_id);
        self.next_id += count;
        base
    }
    
    /// 関数用ラベルを取得または作成
    /// 関数は2つのラベルを使用 (エントリ点 + スキップ先)
    pub fn get_or_create_function_label(&mut self, name: &str) -> LabelId {
        if let Some(&label) = self.function_labels.get(name) {
            label
        } else {
            let label = self.allocate_range(2);
            self.function_labels.insert(name.to_string(), label);
            label
        }
    }
    
    /// 関数ラベルが存在するか確認
    pub fn has_function(&self, name: &str) -> bool {
        self.function_labels.contains_key(name)
    }
}

impl Default for LabelAllocator {
    fn default() -> Self {
        Self::new()
    }
}
```

## ラベル使用パターン

### 関数定義

各関数は2つのラベルを使用：

```
[label]     : 関数エントリポイント
[label + 1] : 関数定義のスキップ先
```

### while 文

```
[label]     : ループ先頭
[label + 1] : ループ終了
```

### if 文

```
[label]     : if ブロック先頭
[label + 1] : if ブロック終了（else/elsif の先頭）
```

## テスト例

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_allocate_labels() {
        let mut alloc = LabelAllocator::new();
        
        let l1 = alloc.allocate();
        let l2 = alloc.allocate();
        
        assert_eq!(l1.0, 16);
        assert_eq!(l2.0, 17);
    }
    
    #[test]
    fn test_allocate_range() {
        let mut alloc = LabelAllocator::new();
        
        let base = alloc.allocate_range(3);
        let next = alloc.allocate();
        
        assert_eq!(base.0, 16);
        assert_eq!(next.0, 19);
    }
    
    #[test]
    fn test_function_labels() {
        let mut alloc = LabelAllocator::new();
        
        let main1 = alloc.get_or_create_function_label("main");
        let foo = alloc.get_or_create_function_label("foo");
        let main2 = alloc.get_or_create_function_label("main");
        
        assert_eq!(main1, main2); // 同じ関数は同じラベル
        assert_ne!(main1, foo);   // 異なる関数は異なるラベル
    }
}
```
