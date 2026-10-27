+++
schema_version = 1
id = "checker-029"
key = "arithmetic-extensions"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-012", "checker-013"]
+++
# Check Gecode number theory, products and distance extensions

## Outcome

Gecode main mathematical integer extensions have direct checked evaluation.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Read gecode_fzn.mzn and registry relation-code handlers for gcd/divides/product/modular product/nroot, inter-distance and required-minimum-distance bindings.

## Done when

- [x] All assigned gcd/divides/product and fixed/variable modular products with _reif/_imp forms, integer roots/powers and distance/required-distance predicates check their documented mathematical relations.
- [x] Signs, zero divisors/moduli, negative exponents, exact versus floor roots, empty products, degenerate distance arrays and required-pair semantics match the pinned source. Arithmetic does not overflow silently or expand huge domains.
- [x] Unique forward outputs derive; witness or root choices that are not unique stay unresolved. Typed arithmetic/distance findings, malformed-code diagnostics and serialized/Wasm behavior follow the shared contract.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.

## Result

Added all 60 Gecode arithmetic and distance spellings with checked integer evaluation, floor roots, modular products, deterministic completion and typed findings. Two direct family modules integrate with preparation, loading, CLI and Wasm. Independent full review and focused formatting recheck passed.

Validation:

- Workspace fmt, Clippy -D warnings and 658 tests passed; parser-free native 20 tests, supported feature Clippy and locked release Wasm passed.
- Native/Wasm smoke passed 160 comparisons across 18 artifacts and 80 candidates; all 74 old candidate records are byte-identical.
- Pinned MiniZinc 2.10.1/Gecode compile-only control preserves eight predicates, seven definition-marker pairs, labels, paths and application metadata. Both CLI source/load forms agree on valid and three-finding invalid candidates. Control formatting passes; upstream S naming warning and 44 analysis limitations documented.
- Coverage check confirms 959 exact names with all task-029 output roles; Ruff, ty and diff checks passed. Verifier snapshots W352afa5f33889ddb and W544ef0121aa29b08; sole control-format correction accepted by fresh focused recheck.
