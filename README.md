# Zincite

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/zincite-logo-light.svg">
  <source media="(prefers-color-scheme: light)" srcset="assets/zincite-logo-dark.svg">
  <img alt="Zincite — orange crystal Z logo" src="assets/zincite-logo-dark.svg" width="420">
</picture>

Zincite formats, lints and queries MiniZinc model (`.mzn`) and data (`.dzn`)
files. The Rust commands share a source-preserving parser and need no compiler
or solver. Zincite is under development, targeting MiniZinc 2.10.1; formatting
may change before a stable release.

## Start here

| Task | Documentation |
| --- | --- |
| Install and try the commands | [Getting started](docs/getting-started.md) |
| Format files and configure layout | [Formatting guide](docs/formatting.md), [style gallery](docs/formatting-style.md) |
| Set up formatting in an editor | [Format on save](docs/format-on-save.md) |
| Select lint rules, understand categories and apply fixes | [Linting guide](docs/linting.md) |
| Inspect source, declarations and uses | [Query guide](docs/queries.md) |
| Extract counts, histograms, types and literal shapes | [Feature-extraction cookbook](docs/query-cookbook.md) |
| Change data values, filter elements or reduce an enum | [Data-edit guide](docs/data-edits.md) |
| Look up command statuses and write restrictions | [Command reference](docs/cli.md) |
| Understand Unknown, failures and coverage limits | [Support limits](docs/support.md) |

## Installation

[Install from a checkout](docs/getting-started.md) with a stable Rust toolchain.
The root command supplies `zincite fmt`, `zincite lint` and `zincite query`;
standalone commands are also available.

## Formatting

The [formatting guide](docs/formatting.md) covers stdout, check and file-write
workflows, configuration and layout directives. Use the
[editor recipe](docs/format-on-save.md) for buffer replacement.

## Linting

The [linting guide](docs/linting.md) explains default rules, families, presets,
model context, suppression and explicit fixes. Rule family, warning severity,
advisory wording and fix eligibility are separate properties.

## Querying items

Start with the [query guide](docs/queries.md) for source selection and semantic
inspection, or the [data-edit guide](docs/data-edits.md) for transformations.
The [cookbook](docs/query-cookbook.md) contains reproducible extraction recipes.

## Exit codes

See the [command reference](docs/cli.md#exit-codes).

## Current limitations

The [support guide](docs/support.md) distinguishes current behavior, supported
Unknown, located limitations, input failures and future work. The
[save checkpoint](scripts/save-performance-current.md) applies the 100 ms p95
target to named ordinary editing cases and reports dense data separately.

## Libraries

| Rust crate | Reference |
| --- | --- |
| `zincite-syntax` | [Source, CST and parsing APIs](docs/reference.md#syntax-library) |
| `zincite-fmt` | [Formatting APIs](docs/reference.md#formatter-library) |
| `zincite-lint` | [Analysis, settings and fix APIs](docs/reference.md#lint-library) |
| `zincite-query` | [Inspection and data-edit APIs](docs/reference.md#query-library) |

Generate local API documentation with `cargo doc --workspace --no-deps --open`.

## Development

Read [AGENTS.md](AGENTS.md) for repository guidance and the
[project brief](.zdev/base/brief.md) for behavior contracts and planned work.
Run the workspace checks with:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The [developer scripts](scripts/README.md) document corpus checks and performance
measurements. The [semantic processing plan](docs/semantic-processing-plan.md)
is developer material; it does not establish completed support. Reader
documentation lives in this README and `docs/*.md`. The
[Astro documentation site](site/README.md) renders these same guides.
Website build commands are separate from the Rust workspace checks.

## License

Zincite is dual-licensed under [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE), at your option.
