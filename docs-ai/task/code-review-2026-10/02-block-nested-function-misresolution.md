# 意味解析: ブロック式内で宣言した関数の呼び出しが別のグローバル関数に解決される

- 重要度: **high**
- 状態: 未着手

## 進捗

- [ ] 仕様方針の決定（禁止 or 許可）
- [ ] 修正・テスト追加

## 問題点

`src/semantic_analyzer/mod.rs:53-80`（`analyze_block_for_expression`）のコメントは「ブロック式の中では新しい関数宣言は許可されない」としているが、実際には禁止していない。ブロック内の関数は一時的な空の Vec（`temp_global_functions`）に登録されるため `global_idx` が 0 から振られ、解析後にその Vec は捨てられる。結果として呼び出しは本来のグローバル関数表の index 0（多くは `__main`）などを指す。**エラーにならず誤ったコードが生成される。**

### 再現（確認済み）

```
func: __main(){ __putc(65); { func: g(){ __putc(66); return: 5; } g(); }; }
→ "AAAA..." を無限出力（B は出ない。__main が再帰している）
```

- インタプリタ・WS コンパイル結果（whitespace20）共に同じ
- `return: { func: g(){return:1;} g(); };` → stack overflow
- `if:` / `while:` の本体でも同様
- `func: h(){return:9;}` を先に置くと `h` が呼ばれる

## 違反している原則

- フェイルセーフ設計: 想定外の構文を黙って受理し、誤ったプログラムを生成する
- AGENTS.md「ドメイン上の想定される失敗は戻り値で返す」
- コメントと実装の不一致

## 解決策

| 案 | 内容 | 影響範囲 |
|---|---|---|
| A | `analyze_block_for_expression` と `ScopeType::Block` の解析で `Statement::FunctionDeclaration` を検出し、`function declaration is not allowed in block` エラーを返す。仕様に明記 | `semantic_analyzer`、`docs/spec.md`、`resources/tests/fails/semantic` にテスト追加 |
| B | ネスト関数を許可する仕様にし、`ctx.global_functions` を一時 Vec ではなく親から共有する（`for` の init ブロックでは既にそうしている） | `semantic_analyzer` の context 受け渡しを全面見直し。`compiler_ws` にも影響の可能性 |

推奨: A を先行して誤コード生成を止め、B は必要になれば別タスクで検討。
