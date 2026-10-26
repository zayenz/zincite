+++
schema_version = 1
id = "checker-029"
key = "arithmetic-extensions"
area = "checker"
status = "open"
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

- [ ] All assigned gcd/divides/product and fixed/variable modular products with _reif/_imp forms, integer roots/powers and distance/required-distance predicates check their documented mathematical relations.
- [ ] Signs, zero divisors/moduli, negative exponents, exact versus floor roots, empty products, degenerate distance arrays and required-pair semantics match the pinned source. Arithmetic does not overflow silently or expand huge domains.
- [ ] Unique forward outputs derive; witness or root choices that are not unique stay unresolved. Typed arithmetic/distance findings, malformed-code diagnostics and serialized/Wasm behavior follow the shared contract.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.
