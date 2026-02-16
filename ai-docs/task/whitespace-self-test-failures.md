# whitespace-self テスト失敗調査

## 概要

`whitespace-self` モード（独自 WhitespaceVM で nospace コンパイル結果を実行）を追加した際に、15件のテストが失敗した。
これらは新規テストであり、既存テストへの影響はない。

## 失敗パターン

### パターン1: ステップ数上限超過 (Suspended)

実行が 1,000,000 ステップ以内に完了しない。無限ループまたは非常に多いステップ数を要するケース。

| テスト名 | パス |
|---|---|
| test_example_puts_ws_self | examples/e0-00-puts |
| test_legacy_015_ws_self | legacy/legacy_015 |
| test_scope_func_shadowing_nested_001_ws_self | scope/func_shadowing_nested_001 |
| test_scope_func_shadowing_siblings_001_ws_self | scope/func_shadowing_siblings_001 |
| test_scope_scope_nested_func_001_ws_self | scope/scope_nested_func_001 |
| test_scope_scope_static_counter_factory_001_ws_self | scope/scope_static_counter_factory_001 |
| test_scope_scope_static_mixed_001_ws_self | scope/scope_static_mixed_001 |
| test_scope_scope_static_multi_decl_001_ws_self | scope/scope_static_multi_decl_001 |
| test_scope_scope_static_nested_001_ws_self | scope/scope_static_nested_001 |

### パターン2: 出力不一致

コンパイルと実行は完了するが、出力が期待値と一致しない。

| テスト名 | パス | 期待値 | 実際の出力 |
|---|---|---|---|
| test_example_fibonacci_ws_self | examples/e0-01-fibonacci | (要調査) | (要調査) |
| test_example_qsort_ws_self | examples/e1-00-qsort | (要調査) | (要調査) |
| test_legacy_011_ws_self | legacy/legacy_011 | (要調査) | (要調査) |
| test_legacy_012_ws_self | legacy/legacy_012 | 1-12-2 | 8-81-1 |
| test_legacy_014_ws_self | legacy/legacy_014 | (要調査) | (要調査) |
| test_legacy_020_ws_self | legacy/legacy_020 | 0111;1010;1010;0111;0101; | 0111;0111;1010; |

## 考えられる原因

1. **Whitespace コンパイラのバグ**: 生成されたコードが正しくない可能性
2. **WhitespaceVM のバグ**: VM の命令実行に問題がある可能性
3. **ステップ数の制限**: 一部のテストが大きなステップ数を必要とする（特にスコープ・関数系）

## ステータス

未調査 - 既存の whitespace コンパイラ・VM の問題として別途調査が必要
