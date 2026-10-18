# zincite-fmt

Format MiniZinc models and data while preserving comments, names and literal spelling.
The library supports configurable layout, protected regions and complete output before
file replacement.

Use a current stable Rust toolchain. Add the library with:

```sh
cargo add zincite-fmt@0.1.2
```

The standalone `zincite-fmt` command uses the same options and exit codes as
`zincite fmt`:

```sh
cargo install --locked --version '=0.1.2' zincite-fmt
zincite-fmt --help
```

See the [reader guides](https://zayenz.se/zincite/),
[library reference](https://zayenz.se/zincite/guides/reference/#formatter-library)
and [Rust API](https://docs.rs/zincite-fmt/0.1.2).

Zincite targets MiniZinc 2.10.1 and does not run a compiler or solver. Read the
[support limits](https://zayenz.se/zincite/guides/support/) before relying on
semantic results. Formatting and interfaces may change in the 0.1 series.

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
