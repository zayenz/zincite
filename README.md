# Zincite

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/zincite-logo-light.svg">
  <source media="(prefers-color-scheme: light)" srcset="assets/zincite-logo-dark.svg">
  <img alt="Zincite — orange crystal Z logo" src="assets/zincite-logo-dark.svg" width="420">
</picture>

Zincite formats, lints and queries MiniZinc model (`.mzn`) and data (`.dzn`)
files. It also checks supplied candidates against compiled FlatZinc models.
Use it to keep layout consistent, get feedback on modelling choices, inspect
source and data, or validate manual edits without running a solver. The Rust
tools target MiniZinc 2.10.1.
Formatting and interfaces may change in the 0.x series.

The [documentation site](https://zayenz.se/zincite/) introduces the tools.
Start with the [staffing walkthrough](https://zayenz.se/zincite/guides/first-model/): format a model,
give its constraints useful labels, then find a day where the data cannot
meet the demand.

## Start here

| Task | Documentation |
| --- | --- |
| Install the commands | [Installation](https://zayenz.se/zincite/guides/getting-started/) |
| Follow a model through formatting, linting and a data query | [Staffing walkthrough](https://zayenz.se/zincite/guides/first-model/) |
| Format files and configure layout | [Formatting guide](https://zayenz.se/zincite/guides/formatting/), [style gallery](https://zayenz.se/zincite/guides/formatting-style/) |
| Set up formatting in an editor | [Format on save](https://zayenz.se/zincite/guides/format-on-save/) |
| Select lint rules, understand categories and apply fixes | [Linting guide](https://zayenz.se/zincite/guides/linting/) |
| Inspect source, declarations and uses | [Query guide](https://zayenz.se/zincite/guides/queries/) |
| Extract counts, histograms, types and literal shapes | [Feature-extraction cookbook](https://zayenz.se/zincite/guides/query-cookbook/) |
| Change data values, filter elements or reduce an enum | [Data-edit guide](https://zayenz.se/zincite/guides/data-edits/) |
| Check candidates or integrate checking into an application | [Checker integration](docs/checker.md), [browser workshop](examples/checker-browser/README.md) |
| Look up command statuses and write restrictions | [Command reference](https://zayenz.se/zincite/guides/cli/) |
| Understand Unknown, failures and coverage limits | [Support limits](https://zayenz.se/zincite/guides/support/) |

## Installation

Install Zincite 0.4.0 with a current stable Rust toolchain:

```sh
cargo install --locked --version '=0.4.0' zincite
```

The root command supplies `zincite fmt`, `zincite lint`, `zincite query` and
`zincite check`.
The [installation guide](https://zayenz.se/zincite/guides/getting-started/) covers standalone commands
and a versioned checkout for the saved examples.

## Formatting

The [formatting guide](https://zayenz.se/zincite/guides/formatting/) covers stdout, check and file-write
workflows, configuration and layout directives. Use the
[editor recipe](https://zayenz.se/zincite/guides/format-on-save/) for buffer replacement.

## Linting

The [linting guide](https://zayenz.se/zincite/guides/linting/) explains default rules, families, presets,
model context, suppression and explicit fixes. Rule family, severity,
advisory wording and fix eligibility are separate properties.

## Querying items

Start with the [query guide](https://zayenz.se/zincite/guides/queries/) for source selection and semantic
inspection, or the [data-edit guide](https://zayenz.se/zincite/guides/data-edits/) for transformations.
The [cookbook](https://zayenz.se/zincite/guides/query-cookbook/) contains reproducible extraction recipes.

Reports accept `[value for pattern in source if condition]` and the equivalent
MiniZinc-style `[value | pattern in source where condition]`. Filters use named
arguments such as `row => row.weight > 0` or the implicit `_` subject. See the
[query guide](https://zayenz.se/zincite/guides/queries/#select-values-with-a-predicate)
for patterns, native subjects and data-edit limits.

Zincite's guides cover query syntax, collection order, optional values and
validated data edits. Zirql owns common native composition and its prepared
execution; Zincite supplies MiniZinc operations, identity, source/model ownership
and edit safety. Zirql's
[values and comprehensions guide](https://zayenz.se/zirql/guides/values/)
offers additional examples and background.

## Checking candidates

Use `zincite check` or `zincite-check` for checking supplied
candidates against MiniZinc 2.10.1 text FlatZinc or official FlatZinc-JSON.
Prepare a model once, then check edits locally, including infeasible or incomplete
candidates. Supported definitions derive auxiliary values; reports contain typed
violations and independent completeness. Checking starts no compiler or solver.
The standalone command and Rust library ship in the `zincite-check` package.

Start with the [checker integration guide](docs/checker.md) for native use,
prepared artifacts and the two pinned offline compiler profiles. The
[browser workshop](examples/checker-browser/README.md) maps reports to editable
jobs, while the [Wasm example](examples/checker-wasm/README.md) explains exact
integer transport and the shared native/Wasm runtime. The
[predicate reference](crates/zincite-check/README.md) specifies mathematical
semantics and numeric limits; the [coverage inventory](docs/checker-coverage.md)
records signatures, aliases and source compiler limits. For implementation work,
read [checker architecture and testing](docs/checker-architecture.md).

The [2026-10-28 measurement report](scripts/checker-performance.md) describes
preparation, loading and repeated checks for its named native/browser build.

## Exit codes

See the [command reference](https://zayenz.se/zincite/guides/cli/#exit-codes).

## Current limitations

The [support guide](https://zayenz.se/zincite/guides/support/) distinguishes current behavior, supported
Unknown, located limitations, input failures and future work. The
[save checkpoint](https://github.com/zayenz/zincite/blob/v0.1.2/scripts/save-performance-current.md) applies the 100 ms p95
target to named ordinary editing cases and reports dense data separately.
The [current lint measurements](https://github.com/zayenz/zincite/blob/v0.1.2/scripts/lint-performance-current.md) report
fresh-process runtime, memory and analysis completeness for five model roots.

## Libraries

| Rust crate | Guide | Rust API |
| --- | --- | --- |
| `zincite-syntax` | [Source, CST and parsing APIs](https://zayenz.se/zincite/guides/reference/#syntax-library) | [0.4.0](https://docs.rs/zincite-syntax/0.4.0) |
| `zincite-fmt` | [Formatting APIs](https://zayenz.se/zincite/guides/reference/#formatter-library) | [0.4.0](https://docs.rs/zincite-fmt/0.4.0) |
| `zincite-lint` | [Analysis, settings and fix APIs](https://zayenz.se/zincite/guides/reference/#lint-library) | [0.4.0](https://docs.rs/zincite-lint/0.4.0) |
| `zincite-query` | [Inspection and data-edit APIs](https://zayenz.se/zincite/guides/reference/#query-library) | [0.4.0](https://docs.rs/zincite-query/0.4.0) |
| `zincite-check` | [Candidate checking APIs](https://zayenz.se/zincite/guides/reference/#checker-library) | [0.4.0](https://docs.rs/zincite-check/0.4.0) |

The library crates are available at version `0.4.0`. From a checkout, use `cargo doc --workspace --no-deps --open` for local API documentation.

## Development

Read [AGENTS.md](https://github.com/zayenz/zincite/blob/v0.4.0/AGENTS.md) for repository guidance and the
[project brief](https://github.com/zayenz/zincite/blob/v0.4.0/.zdev/base/brief.md) for behavior contracts and planned work.
Run the workspace checks with:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo test --workspace --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
```

The [developer scripts](https://github.com/zayenz/zincite/blob/v0.4.0/scripts/README.md) document corpus checks and performance
measurements. The [semantic processing plan](https://github.com/zayenz/zincite/blob/v0.4.0/docs/semantic-processing-plan.md)
is developer material; it does not establish completed support. Reader
documentation lives in this README and `docs/*.md`. The
[Astro documentation site](https://github.com/zayenz/zincite/blob/v0.4.0/site/README.md) renders these same guides.
Website build commands are separate from the Rust workspace checks.

See the [0.4.0 release notes](CHANGELOG.md) and
[release procedure](RELEASING.md) for package verification and publication order.

## License

Zincite is dual-licensed under [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE), at your option.
