# 既存メモリ管理からアロケータへの移行設計

## 現状のメモリ管理構造

### Environment（永続データ）

```rust
pub struct Environment {
    // グローバル変数の値（ルートスコープ）
    pub(crate) global_variables: Vec<i64>,
    // 関数内 static 変数の永続化ストレージ
    // 関数インデックス → 変数配列
    pub(crate) function_static_storage: BTreeMap<usize, Vec<i64>>,
}
```

### LocalEnvironment（関数実行時）

```rust
pub(super) struct LocalEnvironment<'a, 'aenv> {
    pub(super) env: &'aenv mut Environment,
    pub(super) root_scope: &'a Scope,
    // 末尾が現在のスコープ。各要素は变数の値を保持する Vec<i64>
    pub(super) scope_stack: Vec<Vec<i64>>,
}
```

### アドレスモデル

- `resolve_address(id)`: グローバル変数は `local_index`、ローカル変数は `global_count + sum(scope sizes before target) + local_index`
- `get_by_address(addr)` / `set_by_address(addr, value)`: 線形走査でグローバル → 各スコープを順にチェック

## 移行後のメモリ管理構造

### Environment（永続データ）

```rust
pub struct Environment {
    // メモリアロケータ（全メモリを管理）
    pub(crate) allocator: InterpreterAllocator,
    // グローバル変数のベースアドレス（アロケータ上）
    pub(crate) global_base_addr: i64,
    // 関数内 static 変数のベースアドレス
    // 関数インデックス → アロケータ上のベースアドレス
    pub(crate) function_static_addrs: BTreeMap<usize, i64>,
}
```

### LocalEnvironment（関数実行時）

```rust
pub(super) struct LocalEnvironment<'a, 'aenv> {
    pub(super) env: &'aenv mut Environment,
    pub(super) root_scope: &'a Scope,
    // 各スコープのベースアドレス（アロケータ上）
    pub(super) scope_stack: Vec<i64>,
}
```

### 新しいアドレスモデル

- `resolve_address(id)`: `base_addr + local_index`
  - グローバル: `global_base_addr + local_index`
  - ローカル: `scope_stack[scope_idx] + local_index`
- `get_by_address(addr)` / `set_by_address(addr, value)`: `allocator.get(addr)` / `allocator.set(addr, value)` を直接呼び出し

## 変更詳細

### Phase 2: Environment 変更

#### グローバル変数

**変更前:**
```rust
// interpret_global
env.global_variables = create_uninit_vec(scope.variable_count, env.config.randomize_uninit);
```

**変更後:**
```rust
// interpret_global
env.global_base_addr = env.allocator.alloc_uninit(scope.variable_count, env.config.randomize_uninit);
```

#### グローバル変数アクセス

**変更前:**
```rust
fn get_variable(&self, id: &IdentifierRef) -> i64 {
    if id.is_global {
        self.env.global_variables[id.local_index]
    } else { ... }
}
```

**変更後:**
```rust
fn get_variable(&self, id: &IdentifierRef) -> i64 {
    if id.is_global {
        self.env.allocator.get(self.env.global_base_addr + id.local_index as i64)
    } else { ... }
}
```

#### Static 変数ストレージ

**変更前:**
```rust
// function_static_storage: BTreeMap<usize, Vec<i64>>
env.function_static_storage.insert(func_key, scope_data);
```

**変更後:**
```rust
// function_static_addrs: BTreeMap<usize, i64>
let static_addr = env.allocator.alloc_uninit(variable_count, env.config.randomize_uninit);
env.function_static_addrs.insert(func_key, static_addr);
// 値のコピーはアロケータ経由:
env.allocator.set(static_addr + slot_idx as i64, value);
```

### Phase 3: LocalEnvironment 変更

#### スコープの enter / leave

**変更前:**
```rust
fn enter_block(&mut self, scope: &Scope) {
    let vars = create_uninit_vec(scope.variable_count, self.env.config.randomize_uninit);
    self.scope_stack.push(vars);
}

fn leave_block(&mut self) {
    self.scope_stack.pop();
}
```

**変更後:**
```rust
fn enter_block(&mut self, scope: &Scope) {
    let base_addr = self.env.allocator.alloc_uninit(
        scope.variable_count,
        self.env.config.randomize_uninit,
    );
    self.scope_stack.push(base_addr);
}

fn leave_block(&mut self) {
    let base_addr = self.scope_stack.pop().unwrap();
    self.env.allocator.free(base_addr);
}
```

#### 変数アクセスの変更

**変更前:**
```rust
fn get_variable(&self, id: &IdentifierRef) -> i64 {
    if id.is_global {
        self.env.global_variables[id.local_index]
    } else {
        let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
        self.scope_stack[scope_idx][id.local_index]
    }
}
```

**変更後:**
```rust
fn get_variable(&self, id: &IdentifierRef) -> i64 {
    let addr = if id.is_global {
        self.env.global_base_addr + id.local_index as i64
    } else {
        let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
        self.scope_stack[scope_idx] + id.local_index as i64
    };
    self.env.allocator.get(addr)
}
```

#### resolve_address / get_by_address / set_by_address

**変更前:**
```rust
fn resolve_address(&self, id: &IdentifierRef) -> i64 {
    if id.is_global {
        id.local_index as i64
    } else {
        let global_count = self.env.global_variables.len() as i64;
        let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
        let mut addr = global_count;
        for i in 0..scope_idx {
            addr += self.scope_stack[i].len() as i64;
        }
        addr + id.local_index as i64
    }
}

fn get_by_address(&self, addr: i64) -> i64 {
    let addr = addr as usize;
    let global_count = self.env.global_variables.len();
    if addr < global_count {
        self.env.global_variables[addr]
    } else {
        let mut remaining = addr - global_count;
        for scope in &self.scope_stack {
            if remaining < scope.len() {
                return scope[remaining];
            }
            remaining -= scope.len();
        }
        panic!("runtime error: invalid address {}", addr);
    }
}
```

**変更後:**
```rust
fn resolve_address(&self, id: &IdentifierRef) -> i64 {
    if id.is_global {
        self.env.global_base_addr + id.local_index as i64
    } else {
        let scope_idx = self.scope_stack.len() - 1 - id.scope_depth;
        self.scope_stack[scope_idx] + id.local_index as i64
    }
}

fn get_by_address(&self, addr: i64) -> i64 {
    self.env.allocator.get(addr)
}

fn set_by_address(&mut self, addr: i64, value: i64) {
    self.env.allocator.set(addr, value);
}
```

**注目点**: `resolve_address` が O(scope_depth) から O(1) に改善される。
`get_by_address` / `set_by_address` も線形走査 O(total_variables) からアロケータの O(log B) に改善される。

#### 関数呼び出し時のスタック管理

**変更前:**
```rust
// new_func
let mut variables = create_uninit_vec(func.block.scope.variable_count, ...);
// 引数のセット
for (i, arg_val) in args.iter().enumerate() {
    variables[func.arg_indices[i]] = *arg_val;
}
// scope_stack に push
self.scope_stack.push(variables);
```

**変更後:**
```rust
// new_func
let base_addr = env.allocator.alloc_uninit(func.block.scope.variable_count, ...);
// 引数のセット
for (i, arg_val) in args.iter().enumerate() {
    env.allocator.set(base_addr + func.arg_indices[i] as i64, *arg_val);
}
// scope_stack に push
scope_stack.push(base_addr);
```

#### Static 変数の保存・復元

**変更前:**
```rust
// 復元
if let Some(storage) = self.env.function_static_storage.get(&func_key) {
    for var in &func.block.scope.variables {
        if var.is_static {
            variables[slot_idx + i] = storage[slot_idx + i];
        }
    }
}
// 保存
let scope_data = self.scope_stack.last().unwrap().clone();
self.env.function_static_storage.insert(func_key, scope_data);
```

**変更後:**
```rust
// 復元
if let Some(&static_addr) = self.env.function_static_addrs.get(&func_key) {
    for var in &func.block.scope.variables {
        if var.is_static {
            for i in 0..slot_count {
                let val = self.env.allocator.get(static_addr + (slot_idx + i) as i64);
                self.env.allocator.set(base_addr + (slot_idx + i) as i64, val);
            }
        }
    }
}
// 保存
if let Some(&static_addr) = self.env.function_static_addrs.get(&func_key) {
    let base_addr = *self.scope_stack.last().unwrap();
    for var in &func.block.scope.variables {
        if var.is_static {
            for i in 0..slot_count {
                let val = self.env.allocator.get(base_addr + (slot_idx + i) as i64);
                self.env.allocator.set(static_addr + (slot_idx + i) as i64, val);
            }
        }
    }
}
```

### Phase 4: __alloc / __free

**変更前:**
```rust
BuiltinFunctionKind::Alloc | BuiltinFunctionKind::Free => {
    panic!(
        "runtime error: __alloc/__free are not supported in interpreter mode. \
         Use --mode=compile --std=ws --std-ext alloc instead."
    );
}
```

**変更後:**
```rust
BuiltinFunctionKind::Alloc => {
    let size = try_expr!(self.interpret_expression(args.first().unwrap()));
    let addr = self.env.allocator.alloc(size as usize);
    ExpressionFlow::Value(addr)
}
BuiltinFunctionKind::Free => {
    let addr = try_expr!(self.interpret_expression(args.first().unwrap()));
    self.env.allocator.free(addr);
    ExpressionFlow::Value(0)
}
```

## 互換性

- **アドレス値の変化**: `resolve_address` が返すアドレス値はアロケータのアドレスになるため、具体的な値は変わる。ただし `&x` で取得したアドレスを `*ptr` で参照する動作は変わらない
- **配列アクセス**: `arr[i]` は `*(base_addr + offset + i)` として動作。アロケータの同一ブロック内であれば問題なし
- **ブロック境界を跨ぐアクセス**: 異なるアロケーションブロック間のポインタ演算（例: ローカル変数のアドレスから別スコープへアクセス）は、アロケータのブロック境界チェックにより実行時エラーとなる可能性がある。これは意図された動作であり、メモリ安全性の向上に寄与する
