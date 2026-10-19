use super::*;
use crate::parse_to_tokens;
use crate::parse_to_tree;
use crate::semantic_analyzer::analyze;

fn parse_and_analyze(code: &str) -> Scope {
    let code_string = code.to_string();
    let tokens = parse_to_tokens(&code_string).expect("Failed to parse tokens");
    let tree = parse_to_tree(&tokens).expect("Failed to parse tree");
    analyze(&tree).expect("Failed to analyze")
}

fn local_ref(scope_depth: usize, local_index: usize) -> IdentifierRef {
    IdentifierRef {
        scope_depth,
        local_index,
        is_global: false,
        owning_func_index: None,
    }
}

/// 関数 `func_idx` の関数スコープにある変数 `var` のスロット番号
fn slot_of(scope: &Scope, func_idx: usize, var: &str) -> usize {
    *scope.functions[func_idx]
        .block
        .scope
        .variable_indices
        .get(var)
        .unwrap_or_else(|| panic!("variable {} not found", var))
}

const CODE: &str = r#"
func: f() {
  let: a(1);
  static: s(2);
  static: arr[3];
  let: b(3);
  { let: t; t = s; };
  return: s + a + b;
}
func: g() {
  static: gs(5);
  func: inner() {
    func: deep() { return: gs; }
    return: deep();
  }
  return: inner();
}
func: h() { return: 0; }
func: __main() { return: f() + g() + h(); }
"#;

struct Fixture {
    scope: Scope,
    storage: FunctionStaticStorage,
    f: usize,
    g: usize,
    h: usize,
}

fn fixture() -> Fixture {
    let scope = parse_and_analyze(CODE);
    let mut allocator = InterpreterAllocator::new();
    let storage = FunctionStaticStorage::allocate(&scope, &mut allocator, false);
    let f = scope.get_function_index("f").unwrap();
    let g = scope.get_function_index("g").unwrap();
    let h = scope.get_function_index("h").unwrap();
    Fixture {
        scope,
        storage,
        f,
        g,
        h,
    }
}

#[test]
fn test_allocate_only_functions_with_static() {
    let fx = fixture();
    assert!(fx.storage.base_addr(fx.f).is_some());
    assert!(fx.storage.base_addr(fx.g).is_some());
    assert!(fx.storage.base_addr(fx.h).is_none());
    assert_ne!(fx.storage.base_addr(fx.f), fx.storage.base_addr(fx.g));
}

#[test]
fn test_resolve_static_in_function_scope() {
    let fx = fixture();
    let base = fx.storage.base_addr(fx.f).unwrap();
    let s = slot_of(&fx.scope, fx.f, "s");
    // 関数スコープは scope_stack の 3 番目（下に呼び出し元の 2 段がある）
    let frame = FuncFrame {
        func_idx: fx.f,
        scope_index: 2,
    };
    assert_eq!(
        fx.storage.resolve(&local_ref(0, s), Some(frame), 3),
        Some(base + s as i64)
    );
}

#[test]
fn test_resolve_static_from_block() {
    let fx = fixture();
    let base = fx.storage.base_addr(fx.f).unwrap();
    let s = slot_of(&fx.scope, fx.f, "s");
    let frame = FuncFrame {
        func_idx: fx.f,
        scope_index: 2,
    };
    // ブロックの中（scope_stack が 1 段多い）から 1 段外の関数スコープを参照
    assert_eq!(
        fx.storage.resolve(&local_ref(1, s), Some(frame), 4),
        Some(base + s as i64)
    );
}

#[test]
fn test_resolve_non_static_and_block_variable() {
    let fx = fixture();
    let a = slot_of(&fx.scope, fx.f, "a");
    let b = slot_of(&fx.scope, fx.f, "b");
    let s = slot_of(&fx.scope, fx.f, "s");
    let frame = FuncFrame {
        func_idx: fx.f,
        scope_index: 2,
    };
    // 関数スコープの非 static 変数
    assert_eq!(fx.storage.resolve(&local_ref(0, a), Some(frame), 3), None);
    assert_eq!(fx.storage.resolve(&local_ref(0, b), Some(frame), 3), None);
    // ブロック変数（static と同じスロット番号でも、参照先が関数スコープでない）
    assert_eq!(fx.storage.resolve(&local_ref(0, s), Some(frame), 4), None);
}

#[test]
fn test_resolve_static_array_slots() {
    let fx = fixture();
    let base = fx.storage.base_addr(fx.f).unwrap();
    let arr = slot_of(&fx.scope, fx.f, "arr");
    let frame = FuncFrame {
        func_idx: fx.f,
        scope_index: 0,
    };
    for i in 0..3 {
        assert_eq!(
            fx.storage.resolve(&local_ref(0, arr + i), Some(frame), 1),
            Some(base + (arr + i) as i64)
        );
    }
}

#[test]
fn test_resolve_owning_func_index() {
    let fx = fixture();
    let base = fx.storage.base_addr(fx.g).unwrap();
    let gs = slot_of(&fx.scope, fx.g, "gs");
    // deep から祖父関数 g の static を参照（実行中の関数や scope_stack に依存しない）
    let id = IdentifierRef {
        owning_func_index: Some(fx.g),
        ..local_ref(2, gs)
    };
    // 実行中の関数は任意（規則 2 は current を見ない）
    let any_frame = FuncFrame {
        func_idx: fx.h,
        scope_index: 5,
    };
    assert_eq!(
        fx.storage.resolve(&id, Some(any_frame), 6),
        Some(base + gs as i64)
    );
    assert_eq!(fx.storage.resolve(&id, None, 1), Some(base + gs as i64));
}

#[test]
fn test_resolve_without_current_function() {
    let fx = fixture();
    let s = slot_of(&fx.scope, fx.f, "s");
    // static 初期化子の実行中（実行中の関数が無い）は scope_stack で解決する
    assert_eq!(fx.storage.resolve(&local_ref(0, s), None, 1), None);
}

#[test]
fn test_resolve_global_and_out_of_range() {
    let fx = fixture();
    let s = slot_of(&fx.scope, fx.f, "s");
    let frame = FuncFrame {
        func_idx: fx.f,
        scope_index: 0,
    };
    let global = IdentifierRef {
        is_global: true,
        ..local_ref(0, s)
    };
    assert_eq!(fx.storage.resolve(&global, Some(frame), 1), None);
    // scope_depth が scope_stack より深い
    assert_eq!(fx.storage.resolve(&local_ref(3, s), Some(frame), 1), None);
}

#[test]
fn test_resolve_without_storage() {
    let fx = fixture();
    let storage = FunctionStaticStorage::new();
    let s = slot_of(&fx.scope, fx.f, "s");
    let frame = FuncFrame {
        func_idx: fx.f,
        scope_index: 0,
    };
    assert_eq!(storage.resolve(&local_ref(0, s), Some(frame), 1), None);
    let id = IdentifierRef {
        owning_func_index: Some(fx.g),
        ..local_ref(1, s)
    };
    assert_eq!(storage.resolve(&id, Some(frame), 2), None);
}
