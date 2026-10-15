# Install Zincite

Install from a checkout with a current stable Rust toolchain:

```sh
git clone https://github.com/zayenz/zincite.git
cd zincite
cargo install --locked --path .
```

The root command provides `zincite fmt`, `zincite lint` and `zincite query`. To install the
standalone commands instead, use:

```sh
cargo install --locked --path crates/zincite-fmt
cargo install --locked --path crates/zincite-lint
cargo install --locked --path crates/zincite-query
```

Both invocation forms use the same options, defaults and exit codes. Run
`zincite --help` for the command list.
Run `zincite --version` to report the package version and the build's Git revision
when available. Builds from source without Git still report the package version.

The `0.0.0-pre` packages on crates.io are empty name reservations. Install from
source to use the tools.

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
[queries](queries.md) or [data edits](data-edits.md). Read the
[support limits](support.md) before applying the tools to a whole project.
