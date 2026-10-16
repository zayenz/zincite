# zincite-query

Select MiniZinc source, inspect semantic facts, calculate reports and construct explicit
data-edit candidates. Native results retain source locations and completeness; shared
expressions execute through Zirql.

Use a current stable Rust toolchain. Add the library with:

```sh
cargo add zincite-query@0.1.0
```

The standalone `zincite-query` command uses the same options and exit codes as
`zincite query`:

```sh
cargo install --locked --version '=0.1.0' zincite-query
zincite-query --help
```

See the [reader guides](https://zayenz.se/zincite/),
[library reference](https://zayenz.se/zincite/guides/reference/#query-library)
and [Rust API](https://docs.rs/zincite-query/0.1.0).

The archive includes the query examples under `tests/fixtures/`. Repository guide
commands using `crates/zincite-query/tests/fixtures/` run from a v0.1.0 checkout;
inside this crate, use `tests/fixtures/` instead.

Zincite targets MiniZinc 2.10.1 and does not run a compiler or solver. Read the
[support limits](https://zayenz.se/zincite/guides/support/) before relying on
semantic results. Formatting and interfaces may change in the 0.1 series.

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
