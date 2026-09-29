+++
schema_version = 1
id = "base-001"
key = "source-tokens"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = []
+++
# Establish lossless source tokenization

## Outcome

The syntax library exposes source-owned tokens, ranges and lexical diagnostics as the foundation for parsing.

## Context

The checkout has only zdev records. Read brief.md and ../background/syntax-and-reuse.md before choosing the smallest source/token representation. Create the Rust workspace and zincite-syntax; this is the one foundational library task before the first formatter path.

## Boundaries

- No generic parsing framework, semantic machinery, empty formatter/linter crates or Shackle reuse.
- Tokenize plain strings now; recognize interpolation-bearing strings as unsupported without losing bytes. Their internal expression tokenization belongs to string-interpolation.

## Done when

- [ ] An owned UTF-8 source and ordered token stream retain every byte exactly once, including comments, whitespace and erroneous input, with precise byte ranges and readable lexical diagnostics.
- [ ] Identifiers including quoted names, keywords, numerals, plain strings/escapes, punctuation/operators and comments needed by the pinned grammar have token kinds; interpolation is explicitly diagnosed until its task.
- [ ] The library has direct token/range access suitable for a parser; the workspace builds and contains MIT/Apache-2.0 license texts and Cargo metadata.

## Validation

- Run the area Cargo checks from brief.md. Reuse existing fixtures; add only the focused behavior evidence named here.
- Use a compact token-coverage fixture with Unicode comments, escaped/quoted text and bad input; reconstruct from token ranges and check a few significant kinds/ranges. No tests solely for scaffolding.
