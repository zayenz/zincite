# Repository guidance

## Keep changes simple

As an agentic engineering system, you have a strong tendency to drastically over-engineer everything.
Adding test can be good, but is also very often a net-negative due to lock-in to a design and testing costs.
Even worse, the tests you write are often not testing anything interesting, or testing that the change you did just happened.

At all points, ask yourself if this could be done in a simpler way.
Ask yourself if this is the right amount of abstractions, the right amount of tests, if what you've added is actually valuable, or just ceremony.
If you are unsure about the right level, ask the the user, and do that in plain language.

As an example, if you ever find yourself thinking about provenance, then you are almost guaranteed to be overengineering.

## Project context

Zincite is being built in Rust as a Cargo workspace. It will provide
`zincite-fmt` and `zincite-lint` over a shared source-preserving parser and CST.
The language target is MiniZinc 2.10.1, including `.mzn` and `.dzn` files.

Read [the area brief](.zdev/base/brief.md) before implementation. It is the
authority for language coverage, formatting, lint behavior and testing. Read
its linked background only when relevant to the work. Keep detailed product
rules there rather than copying them into this file.

- Build Zincite's own parser and tools. Shackle is a design reference only:
  do not copy its code, depend on its crates or plan integration with it.
- Prefer simple modules and direct tree traversal. Introduce crates and
  abstractions when a concrete consumer needs them.
- Keep library APIs usable independently of thin command-line entry points.
- Preserve source spelling, comments, whitespace and precise ranges in the
  syntax representation. Diagnose malformed or unsupported input explicitly.
- Keep unsupported syntax documented as implementation progresses. Do not
  claim full language support or working commands before they exist.

## Work and validation

Durable work lives in `.zdev/base/`. For zdev task work, use the zdev workflow
and the selected task's context, boundaries and completion conditions.
`TASKS.md` is generated; do not edit it by hand. Preserve unrelated changes.

Use the focused testing level in the brief. Prefer a few checks of public
behavior over tests that mirror implementation details. Documentation and
metadata changes do not need new tests.

Run `zdev check base --format json` after changes to zdev records. Once the
Cargo workspace exists, validate Rust changes with:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run additional MiniZinc acceptance checks when the task calls for them. They
supplement source-preservation checks and do not require a solver run.
Preserve intentional whitespace and line endings in test fixtures.

Keep `Cargo.lock` tracked when it is introduced; the workspace will contain
command-line applications.
