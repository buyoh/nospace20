# Nospace20

## What is Nospace20?

Nospace is a toy programming language that allows arbitrary spaces, newlines, and tabs anywhere in the code, inspired by the esoteric programming language Whitespace. Nospace20 can interpret Nospace code. ~~It can also compile Nospace code into Whitespace.~~

Nospace とは、改行、タブ・半角スペース等の空白に影響を受けることなく記述できるプログラミング言語です。esolang である whitespace と対になる言語を目指しています。Nospace20 は interpreter として動作する他、~~whitespace へのコンパイルも可能です~~。

## run

```
cargo run --bin nospace20
cargo run --release --bin nospace20
```

```
A nospace language interpreter

Usage: nospace20 [OPTIONS] [FILE]

Arguments:
  [FILE]  Source file to execute (reads from stdin if not provided)

Options:
  -d, --debug    Show trace results after execution
  -h, --help     Print help
  -V, --version  Print version
```

## docs

TODO: English docs

- [spec.md](./spec.md) : nospace language specification
- [tutorial.md](./tutorial.md) : A simple tutorial
