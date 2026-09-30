+++
schema_version = 1
id = "base-038"
key = "compact-if-advice"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-034"]
+++
# Recommend compact forms for eligible if expressions

## Outcome

Thesis rule compact-if recognises both zero-branch forms without unsafe equivalence claims.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement section 4.2 from typed condition/branch facts. Preserve source and supply text advice only.

## Done when

- [x] Recognise both branch orders with a decision Boolean condition and eligible integer expressions; place diagnostics on the conditional expression.
- [x] Exclude optional, unknown-typed or potentially partial forms when the replacement would change definedness; explain the possible formulation without promising performance.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check both positive branch orders and parameter condition, noninteger/optional branch and division-by-zero partiality negatives.

## Result

Added compact-if advice for supported decision Boolean zero-branch integer conditionals; independently verified Wf564354743c7579e.

Validation:

- Independent whole-task verification PASS; cargo fmt, clippy and all 94 workspace tests pass.
- Pinned MiniZinc 2.10.1 acceptance and installed-core CLI confirm both orders, quiet controls and explicit divisor limitation; definition regression preserved.
- zdev check and diff check pass; original inputs and six foreign paths unchanged.
