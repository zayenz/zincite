# zincite-query

Select MiniZinc source, inspect semantic facts, calculate reports and construct explicit
data-edit candidates. Native results retain source locations and completeness; shared
expressions are read and evaluated by Zirql. Both `for/in/if` and `|/where`
comprehensions share tuple/list patterns and scoped predicates. Native model
facts and source-preserving Data edits stay with Zincite.

Text programs use Zirql's direct expression steps for filtering, matching,
ordering, native dictionaries, existence, fixed points and paths.
`Program::from_values` uses direct core ordering, mapping, existence and
fixed-point steps alongside typed `ProgramNative::Query` leaves. Native
filtering remains available through those Query leaves. Direct typed `Filter`,
`Match` and `Paths` payloads are unsupported. `NativeBody` retains nested native
results.
Zincite's adapter supplies kinds, subjects, identity, scalar keys, result shapes
and complete-observation policy.
It retains statement/binding bookkeeping, model ownership and edit validation.
See the [adapter boundary](https://zayenz.se/zincite/guides/reference/#native-composition-boundary)
and [Zirql embedding guide](https://zayenz.se/zirql/guides/embedding-programs/#build-native-composition-steps)
for the development API beyond registry Zirql 0.2.0.

Use a current stable Rust toolchain. Add the library with:

```sh
cargo add zincite-query@0.1.2
```

The standalone `zincite-query` command uses the same options and exit codes as
`zincite query`:

```sh
cargo install --locked --version '=0.1.2' zincite-query
zincite-query --help
```

See the [reader guides](https://zayenz.se/zincite/),
[library reference](https://zayenz.se/zincite/guides/reference/#query-library)
and [Rust API](https://docs.rs/zincite-query/0.1.2).

The archive includes the query examples under `tests/fixtures/`. Repository guide
commands using `crates/zincite-query/tests/fixtures/` run from a v0.1.2 checkout;
inside this crate, use `tests/fixtures/` instead. The `examples/` directory is
included in the package too.

The package includes `examples/fixtures/grow-wedding.zq` with a
complete synthetic model and data file. The query grows guest members, records,
groups, tables and capacities as one validated candidate, using native source
copies to retain comments and spelling. The
[wedding growth guide](https://zayenz.se/zincite/guides/data-edits/#grow-a-wedding-instance)
introduces the input, query, supported shapes and preview/save/write commands.
These operations are available since Zincite 0.1.2. Build the v0.1.2 checkout
with `cargo build --release -p zincite -p zincite-query` from the repository root.
The `grow-wedding` Rust example remains an optional illustration of direct literal
views and source-edit APIs, with its own fixed data shape and limits.

Zincite targets MiniZinc 2.10.1 and does not run a compiler or solver. Read the
[support limits](https://zayenz.se/zincite/guides/support/) before relying on
semantic results. Formatting and interfaces may change in the 0.1 series.

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

## Collection value nodes

Development `ProgramValue` reports build Zirql's `Filter`, `Head`, `Tail`,
`Reverse` and `Check` nodes directly. Bound/check arguments are expressions;
`Reduce { operator: ValueReducer::Length, .. }` supplies count/length. Native
query leaves keep their owner/completeness contracts. See the
[host collection reference](../../docs/reference.md#collection-selection-and-cardinality)
and [runnable recipe](../../docs/query-cookbook.md#select-constructed-rows).
Data admission retains its closed subset.
