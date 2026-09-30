+++
schema_version = 1
id = "base-028"
key = "format-corpus-close"
area = "base"
status = "done"
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

- [x] Run preservation, reparse and second-pass checks on every available valid corpus input; reconcile every failure by cause and confirm reduced examples for each distinct remaining defect.
- [x] Check known complete model/data pairs with staged formatted include trees, report unavailable pairings, and propose bounded implementation follow-ups for confirmed Zincite failures as blockers of corpus-acceptance.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Run all valid corpus inputs through preservation/idempotence and known no-solve compiler pair checks. New regressions belong with confirmed follow-up fixes.

## Result

Reconciled the complete formatter corpus and explicit staged compiler pairs, retaining existing syntax and performance acceptance blockers.

Validation:

- All 6,417 current paths and hashes reconcile with the unchanged full scan: 6,345 correctness passes and 27 known timeouts, with all other input outcomes classified.
- Independent replay of eight original/formatted compiler entries audited staged dependencies; six existing syntax reductions remain compiler-positive and Zincite-diagnosed.
- Report links, retained artifact ownership and acceptance gates verified; documentation-only work needed no new tests or benchmark run.
- Independent whole-task PASS and exact snapshot comparison W56efc4ee7f8a8e0f.
