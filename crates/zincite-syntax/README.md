# zincite-syntax

Parse MiniZinc 2.10.1 models and data while retaining source bytes, comments, token
spelling and byte ranges. The crate supplies the CST shared by Zincite's formatter,
linter and query library.

Use a current stable Rust toolchain. Add the library with:

```sh
cargo add zincite-syntax@0.1.0
```

See the [reader guides](https://zayenz.se/zincite/),
[library reference](https://zayenz.se/zincite/guides/reference/#syntax-library)
and [Rust API](https://docs.rs/zincite-syntax/0.1.0).

Zincite targets MiniZinc 2.10.1 and does not run a compiler or solver. Read the
[support limits](https://zayenz.se/zincite/guides/support/) before relying on
semantic results. Formatting and interfaces may change in the 0.1 series.

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
