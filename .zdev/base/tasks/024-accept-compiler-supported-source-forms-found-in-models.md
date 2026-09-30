+++
schema_version = 1
id = "base-024"
key = "compiler-syntax"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-023"]
+++
# Accept compiler-supported source forms found in models

## Outcome

Ordinary user and Challenge models using demonstrated MiniZinc 2.10.1 extensions parse, format and reach linting.

## Context

Read brief Corpus and thesis expansion and ../background/corpus-coverage.md. Start in zincite-syntax lexer/parser and zincite-fmt layout. Verified compiler-accepted failures include infinity range bounds, output annotations and singleton record literals without a comma; connect models additionally exercise local annotations.

## Done when

- [ ] Support these forms with meaningful lossless nodes and formatter/linter traversal; inspect remaining non-software corpus failures to distinguish the same families from malformed inputs.
- [ ] Document grammar/compiler extensions and retain rejection of malformed records, invalid annotation placements and declarations in default data mode.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Add minimal regressions for the confirmed forms and nearby invalid syntax; check original/formatted probes with MiniZinc 2.10.1 and rerun affected corpus groups.
