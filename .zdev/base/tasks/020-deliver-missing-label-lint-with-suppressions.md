+++
schema_version = 1
id = "base-020"
key = "label-lint"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-003"]
+++
# Deliver missing-label lint with suppressions

## Outcome

A zincite-lint library and command report missing constraint labels with exact locations and next-item suppression.

## Context

Read brief Initial lint contract and CLI contract and ../background/linting-and-literature.md. Create zincite-lint around the shared parser. This first usable lint rule is independent of completing all grammar families; later syntax work extends the same parser.

## Boundaries

- Implement missing-constraint-label here; reserve naming as a known suppression ID but report no naming warnings until naming-lint.
- No rule/plugin framework, semantic analysis, fixes or file rewriting. Unsupported syntax reports parse errors and skips rules for that file.

## Done when

- [x] Direct constraint-header string annotations count as labels; nested strings and unrelated expression annotations do not. Warnings are framed as modelling advice.
- [x] Standalone next-item suppressions validate both agreed rule IDs, apply to the target item and reject bad syntax, placement, unknown IDs or missing targets.
- [x] The thin CLI supports explicit files/stdin and the currently available language modes, emits located rule/severity diagnostics and exits 0/1/2 as specified; later data-mode support is consumed through the shared parser.
- [x] Help and usage describe the implemented rule, suppression syntax and current grammar limitations.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use labelled/unlabelled constraints, a nested string and a suppressed next item; check positive/negative warnings, locations and status.
- Check malformed suppression and source errors without writing files; no diagnostic-rendering snapshot matrix.

## Result

Delivered missing-label lint library and CLI with validated next-item suppressions and located modelling advice.

Validation:

- Independent verifier PASS against Wcf261f58020724c9; unchanged snapshot confirmed.
- cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace: all 51 tests passed.
- Focused public behavior probes passed for labels, suppressions, locations, CLI modes, error precedence and unchanged source bytes.
