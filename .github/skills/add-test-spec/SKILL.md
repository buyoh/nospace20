---
name: add-test-spec
description: /resources/test/ 以下にテストケースを追加するときに使う
---

言語 `nospace` は、コード中の任意の箇所のスペース、改行、タブを許容する、遊びを目的としたプログラミング言語である。

- 言語 `nospace` の仕様は `spec.md` に記載
- /spec.md に基づき 、/resources/test/ 以下にテストケースを追加
- 追加したテストケースは `/tests/code_test.rs` へ登録
- 仕様に基づいている場合、未実装のテストケースを追加しても良い。その場合は `/tests/code_test.rs` にはコメントアウトした状態で追加