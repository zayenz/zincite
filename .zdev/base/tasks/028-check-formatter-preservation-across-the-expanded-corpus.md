+++
schema_version = 1
id = "base-028"
key = "format-corpus-close"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-025", "base-027"]
+++
# Check formatter preservation across the expanded corpus

## Outcome

A complete formatting integration report identifies any remaining bounded fixes needed before acceptance.

## Context

Read brief Corpus and thesis expansion and ../background/corpus-coverage.md. Use the corpus runner after syntax expansion. Start with remaining formatter mismatch groups, including matrices, interpolated strings and include groups only when the report shows failures.

## Boundaries

- This is an integration checkpoint, not an unbounded formatter repair task; remaining failures stay visible until their follow-ups pass.

## Done when

- [ ] Run preservation, reparse and second-pass checks on every available valid corpus input; reconcile every failure by cause and confirm reduced examples for each distinct remaining defect.
- [ ] Check known complete model/data pairs with staged formatted include trees, report unavailable pairings, and propose bounded implementation follow-ups for confirmed Zincite failures as blockers of corpus-acceptance.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Run all valid corpus inputs through preservation/idempotence and known no-solve compiler pair checks. New regressions belong with confirmed follow-up fixes.
