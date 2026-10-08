+++
schema_version = 1
id = "base-089"
key = "enum-set-cleanup-and-reindexing"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-088"]
+++
# Prune removed enum members from sets and repair supported group indices

## Outcome

Enum reduction also prunes supported set memberships, removes newly empty groups and updates supported dependent indices.

## Context

Read [the query expansion](../brief.md#query-and-data-transformation-expansion), Enum reduction and best effort. The seating model's same_table and different_tables are array[int] of set of Guests, so empty groups can be dropped and their implicit list indices compacted. Extend the existing enum reduction operation end to end; use its instance associations and remaining-dependency reporting.

## Boundaries

- Retain singleton groups and required empty record fields/fixed-index cells unless established coverage permits their removal. Do not delete unrelated assignments/records to conceal dangling scalar references.
- Update only established index relationships; do not treat arbitrary integers as guest/group IDs or silently guess computed references.

## Done when

- [ ] Removed members are pruned recursively from supported typed/member-resolved sets, including sets nested inside records and arrays.
- [ ] Newly empty entries in variable-length group arrays disappear and surviving group order, contents and comments remain correct; singleton groups remain.
- [ ] Integer group-index mappings drive supported dependent accesses/data changes; enum ordinals follow reduced declaration order, while retained member names and unrelated numbers stay unchanged.
- [ ] Reduced arrays retain required key coverage and rectangularity. Valid required empty record fields/fixed-index set cells remain present and may belong to a complete result; only unresolved dependencies or unsatisfied type/coverage obligations keep the result incomplete.
- [ ] A supported seating reduction has no remaining removed-member references, matches enum/record coverage and can pass through the ordinary preview/write path as a complete result.

## Validation

- Use a few synthetic public cases combining nested set pruning, an empty and a singleton group, shifted group indices, a valid required fixed-index empty set that stays complete and an unsupported numeric dependency that stays incomplete. Verify retained data/indices directly and supplement the complete seating-shaped case with MiniZinc instance checking.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace; use the brief's focused testing level.
