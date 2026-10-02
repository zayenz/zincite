+++
schema_version = 1
id = "base-048"
key = "format-on-save"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-046", "base-047", "base-077"]
+++
# Verify and document an unobtrusive save command

## Outcome

The external formatter command is practical for editor saves and meets the measured save budgets without loading a model project.

## Context

Read brief Performance and format-on-save and ../background/performance.md, including the Ruff, uv and ty references. The existing CLI already supports stdin with --stdin-filepath and produces no partial output on errors. Measure that actual contract after the core improvements; inspect main.rs and config.rs only for demonstrated startup, configuration or I/O overhead.

## Boundaries

- Use the existing one-shot command. An editor-specific extension, daemon, LSP and persistent formatting cache are outside this task.

## Done when

- [x] Exercise an edited buffer through a fresh formatter process and fully captured stdout with the intended filepath/configuration, preserving the original buffer on invalid input and accepting only successful complete output.
- [x] Meet the brief per-case latency and memory budgets for valid/invalid representative save inputs; investigate material first-use delays and document their measured source or explicit external limitation. Propose bounded follow-ups if the gate fails rather than marking it complete.
- [x] Document the release command and generic external-formatter save recipe, including buffer replacement only on success, stdout/stderr handling and filepath configuration. Correct demonstrated command-path overhead locally; no workspace scan, semantic lint or compiler execution enters the save path.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Run the end-to-end benchmark with changed and already-formatted buffers plus invalid syntax/configuration; reuse CLI behavior checks and add tests only for newly fixed observable defects. No new editor plugin is required.

## Result

Verified and documented the release stdin editor-save command, with success-only complete buffer replacement and per-case save budgets passing; host first-execution delay remains an explicit limitation.

Validation:

- Independent verifier PASS at W4dbc817b90202d80 with equal final comparisons; six CLI tests and six independent edited-buffer controls pass.
- All 26 variants x 50 saves plus 100 invalid-configuration saves and native RSS controls checked; dense p95 93.275/93.267 ms and 63.968750 MiB RSS meet unchanged budgets.
- Four identical-copy first-use controls narrow the delay outside configuration/parsing/formatting, with exact host cause unresolved; source, driver, external inputs and six foreign files unchanged.
