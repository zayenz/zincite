# zincite-lint

Inspect MiniZinc source for modelling concerns. The library supplies rule selection,
bounded semantic facts, located diagnostics and explicit fix candidates. Ordinary
analysis is read-only.

Use a current stable Rust toolchain. Add the library with:

```sh
cargo add zincite-lint@0.1.0
```

The standalone `zincite-lint` command uses the same options and exit codes as
`zincite lint`:

```sh
cargo install --locked --version '=0.1.0' zincite-lint
zincite-lint --help
```

See the [reader guides](https://zayenz.se/zincite/),
[library reference](https://zayenz.se/zincite/guides/reference/#lint-library)
and [Rust API](https://docs.rs/zincite-lint/0.1.0).

Zincite targets MiniZinc 2.10.1 and does not run a compiler or solver. Read the
[support limits](https://zayenz.se/zincite/guides/support/) before relying on
semantic results. Formatting and interfaces may change in the 0.1 series.

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
