# コード生成

## CodeGenContext - 生成コンテキスト

```rust
use std::collections::HashMap;
use crate::semantic_analyzer::Scope;

/// 変数のスコープ種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarScope {
    Global,
    Local,
}

/// 変数情報
#[derive(Debug, Clone)]
pub struct VarInfo {
    pub scope: VarScope,
    pub offset: i64,
}

/// コード生成コンテキスト
pub struct CodeGenContext<'a> {
    /// 元の Scope 構造
    scope: &'a Scope,
    
    /// ラベル管理
    labels: LabelAllocator,
    
    /// 現在のスコープがグローバルか
    is_global: bool,
    
    /// 現在の関数のローカル変数サイズ
    local_heap_size: i64,
    
    /// 変数マッピング (変数名 → VarInfo)
    variables: HashMap<String, VarInfo>,
}

impl<'a> CodeGenContext<'a> {
    pub fn new(scope: &'a Scope) -> Self {
        Self {
            scope,
            labels: LabelAllocator::new(),
            is_global: true,
            local_heap_size: 0,
            variables: HashMap::new(),
        }
    }
    
    /// グローバル変数を登録
    pub fn register_global_variable(&mut self, name: &str, offset: i64) {
        self.variables.insert(name.to_string(), VarInfo {
            scope: VarScope::Global,
            offset,
        });
    }
    
    /// ローカル（関数内）コンテキストを作成
    pub fn enter_function(&self, local_vars: &[String]) -> CodeGenContext<'a> {
        let mut ctx = CodeGenContext {
            scope: self.scope,
            labels: self.labels.clone(), // TODO: 適切なラベル共有方法を検討
            is_global: false,
            local_heap_size: local_vars.len() as i64,
            variables: self.variables.clone(), // グローバル変数を継承
        };
        
        // ローカル変数を登録
        for (i, name) in local_vars.iter().enumerate() {
            ctx.variables.insert(name.clone(), VarInfo {
                scope: VarScope::Local,
                offset: i as i64,
            });
        }
        
        ctx
    }
    
    /// 変数のアドレス情報を取得
    pub fn get_variable(&self, name: &str) -> Option<&VarInfo> {
        self.variables.get(name)
    }
    
    /// グローバルヒープサイズを取得
    pub fn global_heap_size(&self) -> i64 {
        self.variables
            .values()
            .filter(|v| v.scope == VarScope::Global)
            .count() as i64
    }
    
    /// 新しいラベルを確保
    pub fn new_label(&mut self) -> LabelId {
        self.labels.allocate()
    }
    
    /// ラベル範囲を確保
    pub fn new_label_range(&mut self, count: u32) -> LabelId {
        self.labels.allocate_range(count)
    }
    
    /// 関数ラベルを取得
    pub fn get_function_label(&mut self, name: &str) -> LabelId {
        self.labels.get_or_create_function_label(name)
    }
}
```

## 式のコード生成

```rust
use crate::semantic_analyzer::ExecExpression;

/// 式を評価するコードを生成
/// 評価結果はスタックトップに残る
pub fn generate_expression(
    ctx: &mut CodeGenContext,
    expr: &ExecExpression,
) -> Result<WsProgram, CompileError> {
    match expr {
        // リテラル値
        ExecExpression::Factor(value) => {
            let mut prog = WsProgram::new();
            prog.push(Instruction::Push(WsNumber(*value)));
            Ok(prog)
        }
        
        // 変数参照
        ExecExpression::Variable(var_id) => {
            generate_load_variable(ctx, var_id)
        }
        
        // 単項演算
        ExecExpression::Operation1(op, inner) => {
            generate_unary_op(ctx, op, inner)
        }
        
        // 二項演算
        ExecExpression::Operation2(op, left, right) => {
            generate_binary_op(ctx, op, left, right)
        }
        
        // 関数呼び出し
        ExecExpression::Function(func_id, args) => {
            generate_function_call(ctx, func_id, args)
        }
        
        // if 式
        ExecExpression::If(cond, then_block, else_block) => {
            generate_if_expression(ctx, cond, then_block, else_block)
        }
        
        // while 式
        ExecExpression::While(cond, body) => {
            generate_while_expression(ctx, cond, body)
        }
        
        // 代入
        ExecExpression::Assign(var_id, value) => {
            generate_assign(ctx, var_id, value)
        }
    }
}
```

## 変数アクセス

```rust
/// 変数の値をロード（スタックにプッシュ）
fn generate_load_variable(
    ctx: &CodeGenContext,
    var_name: &str,
) -> Result<WsProgram, CompileError> {
    let var = ctx.get_variable(var_name)
        .ok_or_else(|| CompileError::UndefinedVariable(var_name.to_string()))?;
    
    let mut prog = WsProgram::new();
    
    // アドレス計算
    prog.append(generate_var_address(var)?);
    
    // 値を取得
    prog.push(Instruction::Retrieve);
    
    Ok(prog)
}

/// 変数のアドレスをスタックにプッシュ
fn generate_var_address(var: &VarInfo) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    
    match var.scope {
        VarScope::Global => {
            // グローバル: GlobalPtr + offset
            let addr = heap_layout::GLOBAL_PTR + var.offset;
            prog.push(Instruction::Push(WsNumber(addr)));
        }
        VarScope::Local => {
            // ローカル: heap[LocalHeapBegin] + offset
            prog.push(Instruction::Push(WsNumber(var.offset)));
            prog.push(Instruction::Push(WsNumber(heap_layout::LOCAL_HEAP_BEGIN)));
            prog.push(Instruction::Retrieve);
            prog.push(Instruction::Add);
        }
    }
    
    Ok(prog)
}
```

## 二項演算子

```rust
fn generate_binary_op(
    ctx: &mut CodeGenContext,
    op: &Operator2,
    left: &ExecExpression,
    right: &ExecExpression,
) -> Result<WsProgram, CompileError> {
    let mut prog = WsProgram::new();
    
    match op {
        // 算術演算
        Operator2::Add => {
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Add);
        }
        Operator2::Sub => {
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Sub);
        }
        Operator2::Mul => {
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Mul);
        }
        Operator2::Div => {
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Div);
        }
        Operator2::Mod => {
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Mod);
        }
        
        // 比較演算
        Operator2::Equal => {
            prog.push(Instruction::Push(WsNumber(1))); // zero → true
            prog.push(Instruction::Push(WsNumber(0))); // non-zero → false
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Sub);
            prog.push(Instruction::Call(reserved_labels::COMPARATOR_ZERO));
        }
        Operator2::NotEqual => {
            prog.push(Instruction::Push(WsNumber(0))); // zero → false
            prog.push(Instruction::Push(WsNumber(1))); // non-zero → true
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Sub);
            prog.push(Instruction::Call(reserved_labels::COMPARATOR_ZERO));
        }
        Operator2::Less => {
            prog.push(Instruction::Push(WsNumber(1))); // negative → true
            prog.push(Instruction::Push(WsNumber(0))); // non-negative → false
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Sub);
            prog.push(Instruction::Call(reserved_labels::COMPARATOR_NEGATIVE));
        }
        // ... 他の比較演算子
        
        // 論理演算
        Operator2::And => {
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Call(reserved_labels::COMPARATOR_AND));
        }
        Operator2::Or => {
            prog.append(generate_expression(ctx, left)?);
            prog.append(generate_expression(ctx, right)?);
            prog.push(Instruction::Call(reserved_labels::COMPARATOR_OR));
        }
    }
    
    Ok(prog)
}
```

## 文のコード生成

```rust
use crate::semantic_analyzer::ExecStatement;

/// 文を実行するコードを生成
pub fn generate_statement(
    ctx: &mut CodeGenContext,
    stmt: &ExecStatement,
) -> Result<WsProgram, CompileError> {
    match stmt {
        // 式文（結果を破棄）
        ExecStatement::Expression(expr) => {
            let mut prog = generate_expression(ctx, expr)?;
            prog.push(Instruction::Discard);
            Ok(prog)
        }
        
        // 変数宣言（初期値を代入）
        ExecStatement::VariableDeclaration(name, init_expr) => {
            generate_var_declaration(ctx, name, init_expr)
        }
        
        // 関数宣言
        ExecStatement::FunctionDeclaration(name, args, body) => {
            generate_function_def(ctx, name, args, body)
        }
        
        // return 文
        ExecStatement::Return(expr) => {
            generate_return(ctx, expr)
        }
        
        // break/continue
        ExecStatement::Break => {
            // TODO: ループラベルの管理が必要
            Err(CompileError::InvalidOperation("break".to_string()))
        }
        ExecStatement::Continue => {
            Err(CompileError::InvalidOperation("continue".to_string()))
        }
    }
}
```
