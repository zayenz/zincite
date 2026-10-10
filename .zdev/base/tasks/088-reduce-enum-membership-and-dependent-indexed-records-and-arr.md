+++
schema_version = 1
id = "base-088"
key = "enum-dependent-data-reduction"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-087"]
+++
# Reduce enum membership and dependent indexed records and array slices

## Outcome

An explicit enum reduction produces a smaller enum and removes corresponding supported indexed instance data while exposing unresolved dependencies.

## Context

Read [the query expansion](../brief.md#query-and-data-transformation-expansion), Enum reduction and best effort. Reuse public ModelContext, binding/callable/type/domain facts in zincite-lint; the existing loader does not pair .dzn assignments with model declarations, so add only the association needed by this consumer. The seating model declares Guests, array[Guests] of GuestInfo: guests and nested array[Topics] record fields. Its keyed records demonstrate the first useful path.

## Boundaries

- Keep model/include sources read-only. No full type checker, solver, general evaluation, fabricated positional alignment or dependency on general semantic query navigation.
- Set-member cleanup and group-array compaction are delivered by the following task; this checkpoint must report those remaining dependencies as incomplete rather than claim a finished wedding reduction.

## Done when

- [x] reduce_enum with keep and keep_first selects explicit .dzn member lists deterministically, preserves retained names/order and rejects unknown requested members or unsupported constructed enums with located diagnostics.
- [x] Enum-keyed dependent entries, including complete records, are removed; supplied model declarations establish enum identity and alignment for supported positional data.
- [x] Supported enum-indexed dimensions are sliced consistently, including nested literal arrays and rectangular multidimensional data; unrelated axes/fields/assignments remain unchanged.
- [x] The library and CLI return a reparsed candidate plus complete/incomplete status and located remaining dependencies, including scalar references and unavailable alignment facts. Strings are never rewritten as identifiers.
- [x] Incomplete candidates are inspectable as stdout/diff with status 1 but cannot be written in place; actual errors remain status 2. No stage claims model satisfiability or lint-fix safety.

## Validation

- Use a few synthetic public model/data cases for keyed records, an interior positional removal, nested/rectangular axes and unknown alignment/scalar dependence. Check expected retained identities/values directly and supplement complete supported cases with MiniZinc instance checks.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace; use the brief's focused testing level.

## Result

Implemented source-preserving enum retention and supported dependent array reduction with optional read-only model facts and located complete/incomplete candidates.

Validation:

- Independent whole-task review passed; snapshot W6dffd2230fe45e0f comparison equal true.
- cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace passed.
- Direct retained identities, values and axes checked; six MiniZinc 2.10.1 original/candidate instance checks passed without solving. Computed model/include accesses are incomplete and refuse writes.
