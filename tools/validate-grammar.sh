#!/bin/bash
# validate-grammar.sh - BNF ファイルの整合性を検証するスクリプト
#
# 使用方法:
#   ./tools/validate-grammar.sh [bnf-file]
#   ./tools/validate-grammar.sh docs/grammar.bnf
#
# 検証内容:
#   1. BNF 構文の基本チェック（::= の形式）
#   2. 未定義の非終端記号の検出
#   3. 未使用の規則の検出
#   4. 既存パーサーテストとの整合性（オプション）

set -uo pipefail

# デフォルトのBNFファイル
BNF_FILE="${1:-docs/grammar.bnf}"

# カウンター
errors=0
warnings=0

echo "=== BNF Validation: $BNF_FILE ==="
echo

# ファイル存在チェック
if [[ ! -f "$BNF_FILE" ]]; then
  echo "Error: File not found: $BNF_FILE"
  exit 1
fi

# 1. 基本構文チェック
echo "1. Checking basic syntax..."

line_num=0
while IFS= read -r line || [[ -n "$line" ]]; do
  line_num=$((line_num + 1))
  # 空行をスキップ
  [[ -z "$line" ]] && continue
  [[ "$line" =~ ^[[:space:]]*$ ]] && continue
  # コメント行（#で始まる）をスキップ
  [[ "$line" =~ ^[[:space:]]*# ]] && continue
  # 継続行（| で始まる）をスキップ
  [[ "$line" =~ ^[[:space:]]*\| ]] && continue
  # 規則定義行は ::= を含むべき
  if [[ "$line" =~ ^[a-zA-Z_] ]] && [[ ! "$line" =~ ::= ]]; then
    echo "Warning: Line $line_num may be malformed: $line"
    warnings=$((warnings + 1))
  fi
done < "$BNF_FILE"

echo "  Basic syntax check completed"

# 2. 非終端記号の収集
echo "2. Collecting non-terminals..."

# 定義された非終端記号を抽出
defined_symbols=$(grep -E '^[a-zA-Z_][a-zA-Z0-9_]*[[:space:]]*::=' "$BNF_FILE" | \
  sed 's/[[:space:]]*::=.*//' | sort -u) || true

# 参照された非終端記号を抽出（引用符内を除く）
# 簡易的な抽出：小文字で始まる識別子
referenced_symbols=$(grep -oE '\b[a-z_][a-z0-9_]*\b' "$BNF_FILE" | \
  grep -v -E '^(if|else|while|return|break|continue|func|let)$' | \
  sort -u) || true

defined_count=$(echo "$defined_symbols" | grep -c . || echo 0)
echo "  Found $defined_count defined rules"

# 3. 未定義の非終端記号をチェック
echo "3. Checking for undefined non-terminals..."

for symbol in $referenced_symbols; do
  # 定義されているか確認
  if ! echo "$defined_symbols" | grep -q "^${symbol}$"; then
    # トークン定義（正規表現）やキーワードは除外
    case "$symbol" in
      # 予約語・キーワード
      func|let|if|else|while|return|break|continue)
        ;;
      # トークン関連
      integer|char|ident|space|comment)
        ;;
      # その他の既知の識別子
      expr*|stmt*|block|program|global*)
        # パターンマッチで定義を探す
        if ! grep -qE "^${symbol}[[:space:]]*::=" "$BNF_FILE"; then
          echo "Warning: '$symbol' may be undefined"
          warnings=$((warnings + 1))
        fi
        ;;
      *)
        # 無視するパターン
        ;;
    esac
  fi
done

# 4. 未使用の規則をチェック
echo "4. Checking for unused rules..."

for symbol in $defined_symbols; do
  # program は開始記号なので除外
  if [[ "$symbol" == "program" ]]; then
    continue
  fi
  # 暗黙的に使用されるトークン定義を除外
  if [[ "$symbol" == "space" ]] || [[ "$symbol" == "comment" ]]; then
    continue
  fi
  # 他の場所で参照されているか確認
  # 定義行以外で出現するかチェック
  ref_count=$(grep -v "^${symbol}[[:space:]]*::=" "$BNF_FILE" | grep -c "\b${symbol}\b" || true)
  if [[ "$ref_count" -eq 0 ]]; then
    echo "Warning: '$symbol' is defined but never referenced"
    warnings=$((warnings + 1))
  fi
done

# 5. 既存パーサーテストとの整合性チェック（オプション）
echo "5. Checking parser test coverage..."

test_dir="resources/tests/passes"
if [[ -d "$test_dir" ]]; then
  test_count=$(find "$test_dir" -name "*.ns" | wc -l)
  echo "  Found $test_count test files in $test_dir"
  
  # 基本的な構文要素がテストされているか確認
  constructs=("if:" "while:" "func:" "let:" "return:" "break;" "continue;")
  for construct in "${constructs[@]}"; do
    if grep -rq "$construct" "$test_dir" 2>/dev/null; then
      echo "  ✓ '$construct' is tested"
    else
      echo "  ○ '$construct' may not be tested"
    fi
  done
else
  echo "  Test directory not found: $test_dir"
fi

# 結果サマリー
echo
echo "=== Summary ==="
if [[ $errors -gt 0 ]]; then
  echo "Errors: $errors"
fi
if [[ $warnings -gt 0 ]]; then
  echo "Warnings: $warnings"
fi
if [[ $errors -eq 0 ]] && [[ $warnings -eq 0 ]]; then
  echo "No issues found!"
fi

# 終了コード
if [[ $errors -gt 0 ]]; then
  exit 1
fi
exit 0
