+++
schema_version = 1
id = "checker-027"
key = "set-aggregate-globals"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-017", "checker-012"]
+++
# Check aggregate, ordered and weighted set globals

## Outcome

Gecode aggregate-set and extremum relations support compact exact values and useful outputs.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Read aggregate-set and set-extremum declarations/registrations. In particular, the empty Gecode array intersection uses its finite solver set universe, not an unbounded i64 universe.

## Done when

- [x] All assigned array union/intersection/partition, disjoint, convex/hull, seq/seq_union, set_min/max reif/imp, fixed-cardinality atmost_one and weighted-sum forms check their exact contracts.
- [x] Empty-array/intersection identities, empty-set extrema, weights, pairwise cardinalities and set order are correct. Unique aggregate/extremum outputs derive without enumerating huge ranges for storage.
- [x] Typed aggregate findings and explicit Gecode universe constants survive native/loading/Wasm paths; large cardinalities/weighted arithmetic are checked and missing values stay incomplete.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.

## Result

Added 48 set-global aliases with compact aggregate, order, cardinality, extremum and weighted checks, marked scalar completion and typed findings. Empty intersection uses the finite Gecode universe; arbitrary weight-row order uses the first listed weight per element.

Validation:

- Initial full review Wa8230491998bf5c5 accepted runtime contracts and requested one coverage-alias correction. Independently checked correction passed fresh focused recheck Wca78bc204a01237d; snapshot comparison equal.
- Workspace formatting, Clippy and all 646 tests passed; parser-free check, 18 tests and locked build passed. Parser-free native and Wasm Clippy passed with warnings denied.
- Locked release Wasm, Node/Web bindings and syntax-free dependency trees passed; 138 native/Wasm comparisons across 16 artifacts and 69 candidates passed, preserving the 64 earlier candidate bytes and compatibility loading 15 original artifacts.
- Pinned compile-only controls and direct/native candidate calculations passed 48 both-CLI source/load comparisons. Four expected public-profile export rejections document annotation and output-marker gaps for checker-030/031.
- Coverage reports 869 names with all 48 output rules checked and four corrected output cells. Ruff, ty, MiniZinc formatting/default lint, zdev and diff checks passed.
