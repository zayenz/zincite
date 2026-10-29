# Install Zincite

Use a current stable Rust toolchain to install Zincite 0.5.0:

```sh
cargo install --locked --version '=0.5.0' zincite
```

The root command provides `zincite fmt`, `zincite lint`, `zincite query` and
`zincite check`.
To install the standalone commands instead, use:

```sh
cargo install --locked --version '=0.5.0' zincite-fmt
cargo install --locked --version '=0.5.0' zincite-lint
cargo install --locked --version '=0.5.0' zincite-query
cargo install --locked --version '=0.5.0' zincite-check
```

Both invocation forms use the same options, defaults and exit codes. Run
`zincite --help` for the command list, or a standalone command's `--help` for
its options. `zincite --version` reports `0.5.0` and the build's Git revision
when available. A build without Git reports just the package version.

## Build from a checkout

For contributions or the repository's saved examples, check out the matching
release and run commands from its root:

```sh
git clone --branch v0.5.0 https://github.com/zayenz/zincite.git
cd zincite
cargo install --locked --path .
```

To build a standalone command from this checkout, use its crate path, for
example `cargo install --locked --path crates/zincite-query`.
The query examples live in `crates/zincite-query/tests/fixtures/`. They also
ship in the `zincite-query` crate under `tests/fixtures/`; adjust the path when
running from that unpacked crate.

## Try the commands

The [staffing walkthrough](first-model.md) takes one model through formatting,
lint feedback and a query that finds a shortage in its data. It includes all
the input files and explains the results. Start there for a first look at what
the tools do.

For a quick installation check, these stdin examples need no model files or
standard library:

```sh
printf 'int: capacity=20;\n' | zincite fmt
printf 'int: capacity = 20;\n' | zincite lint --isolated --rules naming,missing-constraint-label
printf 'capacity = 20;\n' | zincite query --stdin-filepath data.dzn 'filter(kind("assignment")) | count'
```

The formatter prints `int: capacity = 20;`, the lint command produces no
warnings, and the query prints `1`. All three exit 0.

Continue with [formatting](formatting.md), [linting](linting.md),
[queries](queries.md), [data edits](data-edits.md) or
[candidate checking](checker.md). Read the
[support limits](support.md) before applying the tools to a whole project.
