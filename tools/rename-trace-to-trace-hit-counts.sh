#!/bin/bash
# check.json ファイルの "trace" を "trace_hit_counts" に一括変換

set -eu

cd "$(dirname "$0")/.."

echo "Converting 'trace' to 'trace_hit_counts' in check.json files..."

# resources/tests ディレクトリ内の全ての *.check.json ファイルを検索して変換
find resources/tests -name '*.check.json' -type f | while read -r file; do
    if grep -q '"trace":' "$file"; then
        echo "  Processing: $file"
        if [[ "$OSTYPE" == "darwin"* ]]; then
            # macOS の場合は sed -i '' を使用
            sed -i '' 's/"trace":/"trace_hit_counts":/g' "$file"
        else
            # Linux の場合は sed -i を使用
            sed -i 's/"trace":/"trace_hit_counts":/g' "$file"
        fi
    fi
done

echo "Conversion completed!"
