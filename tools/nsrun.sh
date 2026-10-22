#!/usr/bin/env bash
# nsrun.sh - nospace のソースをコンパイルして、そのまま Whitespace VM で実行する
#
# usage: tools/nsrun.sh [--ignore-debug] [NOSPACE20_ARGS...] FILE.ns [-- WHITESPACE20_ARGS...]
#
#   - プログラムの stdin は、このスクリプトの stdin
#   - debug 拡張は既定で有効（コンパイラと VM の両方に --std-ext debug を渡す）。--ignore-debug で無効
#   - `--` より前の引数は nospace20 に、後の引数は whitespace20 に渡す（例: -- --debug --max-steps 1000）
#   - nospace20 に -o / --output / --target を渡さないこと（出力を stdout から受け取るため）
#   - バイナリは環境変数 NOSPACE20 / WHITESPACE20 で指定できる（既定は PATH 上の nospace20 / whitespace20）
#
# 一時ファイルは使わない。コンパイル結果はシェル変数に入れ、fd 4 のパイプで whitespace20 に渡す。
# /dev/fd が必要（Linux・macOS）。

set -u

nospace20=${NOSPACE20:-nospace20}
whitespace20=${WHITESPACE20:-whitespace20}

debug_args=(--std-ext debug)
ns_args=()
ws_args=()
after_separator=0
for arg in "$@"; do
    if [ "$after_separator" = 1 ]; then
        ws_args+=("$arg")
    elif [ "$arg" = "--" ]; then
        after_separator=1
    elif [ "$arg" = "--ignore-debug" ]; then
        debug_args=()
    else
        ns_args+=("$arg")
    fi
done

# FILE を忘れると nospace20 がプログラムへの入力（stdin）をソースとして読んでしまうため、
# 既存のファイルを指す引数がなければ usage を出して終了する
has_file=0
for arg in ${ns_args[@]+"${ns_args[@]}"}; do
    if [ -f "$arg" ]; then
        has_file=1
    fi
done
if [ "$has_file" = 0 ]; then
    echo "usage: nsrun.sh [--ignore-debug] [NOSPACE20_ARGS...] FILE.ns [-- WHITESPACE20_ARGS...]" >&2
    exit 2
fi

# コンパイルに失敗したら VM を起動せずに終了する（パイプで直接つなぐと、空のプログラムが exit 0 で終わる）。
# 末尾の LF は Whitespace の命令の一部なので、$(...) で削られないよう番兵 x を付けて後で外す
# nospace20 の終了コード（引数エラーの 2 など）はそのまま返す
ws=$("$nospace20" ${debug_args[@]+"${debug_args[@]}"} ${ns_args[@]+"${ns_args[@]}"} && echo x) || exit $?
ws=${ws%x}

# 元の stdin を fd 3 に退避し、パイプ（ソース）を fd 4 に移して /dev/fd/4 から読ませる。
# ヒアドキュメント・ヒアストリングは古い bash では一時ファイルを作るため使わない
exec 3<&0
printf '%s' "$ws" |
    "$whitespace20" ${debug_args[@]+"${debug_args[@]}"} ${ws_args[@]+"${ws_args[@]}"} /dev/fd/4 4<&0 0<&3 3<&-
