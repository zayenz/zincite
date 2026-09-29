# Zincite

Zincite is a Rust project for MiniZinc source tooling. The planned tools are
`zincite-fmt`, a formatter, and `zincite-lint`, a static analyser. They will share
a parser and concrete syntax tree that retain source spelling, comments,
whitespace and locations.

The project is at the implementation-planning stage. This repository contains
the agreed design and task backlog; there is no Cargo workspace or runnable
tool yet.

## Scope

The initial target is MiniZinc 2.10.1, covering model (`.mzn`) and data (`.dzn`)
files. The formatter will support explicit files and stdin, with stdout, check
and in-place modes. The first lint rules will check naming conventions and
advise on missing constraint labels.

Formatting will preserve comments and source meaning, with layouts chosen to
keep later diffs small. The default width is 120 columns with four-space
indentation; EditorConfig and explicit command-line options will control the
supported layout settings.

Zincite will own its Rust parser and source representation. Full type checking,
flattening, solving and automatic semantic rewrites are outside the initial
scope.

## Development

The [area brief](.zdev/base/brief.md) records the decisions and detailed behavior.
The [task list](.zdev/base/TASKS.md) records implementation order and dependencies.
Read [AGENTS.md](AGENTS.md) before making changes.

Work is tracked with zdev in the `base` area. To inspect the current state and
validate the planning records:

```sh
zdev status base
zdev check base --format json
```

Once the Cargo workspace exists, the standard checks will be:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Keep tests focused on observable behavior: exact source coverage, recovery after
an error, stable formatting, preserved comments and useful lint diagnostics.
MiniZinc 2.10.1 can provide supplementary acceptance checks for complete models
and model/data pairs. Compiler acceptance alone does not prove that formatting
preserves meaning.

## License

Zincite is dual-licensed under [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE), at your option.
