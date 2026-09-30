+++
schema_version = 1
id = "base-048"
key = "format-on-save"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-046", "base-047"]
+++
# Verify and document an unobtrusive save command

## Outcome

The external formatter command is practical for editor saves and meets the measured save budgets without loading a model project.

## Context

Read brief Performance and format-on-save and ../background/performance.md, including the Ruff, uv and ty references. The existing CLI already supports stdin with --stdin-filepath and produces no partial output on errors. Measure that actual contract after the core improvements; inspect main.rs and config.rs only for demonstrated startup, configuration or I/O overhead.

## Boundaries

- Use the existing one-shot command. An editor-specific extension, daemon, LSP and persistent formatting cache are outside this task.

## Done when

- [ ] Exercise an edited buffer through a fresh formatter process and fully captured stdout with the intended filepath/configuration, preserving the original buffer on invalid input and accepting only successful complete output.
- [ ] Meet the brief per-case latency and memory budgets for valid/invalid representative save inputs; investigate material first-use delays and document their measured source or explicit external limitation. Propose bounded follow-ups if the gate fails rather than marking it complete.
- [ ] Document the release command and generic external-formatter save recipe, including buffer replacement only on success, stdout/stderr handling and filepath configuration. Correct demonstrated command-path overhead locally; no workspace scan, semantic lint or compiler execution enters the save path.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Run the end-to-end benchmark with changed and already-formatted buffers plus invalid syntax/configuration; reuse CLI behavior checks and add tests only for newly fixed observable defects. No new editor plugin is required.
