# Syntax foundation and design references

Inspected 2026-09-29. These are research findings and recommendations, not selected
dependencies. The area brief owns scope and decisions.

Decision: Zincite owns its Rust parser, CST, formatter, and linter. Shackle is a
research and design reference only. Do not reuse its code, depend on its crates,
or plan integration with it. The observations below inform independent design.

## Zirium

Local checkout: `/Users/zayenz/projects/zirium`, HEAD
`00f5cc6c1001cb60ac6dadb980d309e57449a702`.

- [Syntax representation](https://github.com/zayenz/zirium/blob/00f5cc6c1001cb60ac6dadb980d309e57449a702/docs/architecture/syntax-representation.md):
  source bytes are owned separately; tokens reference byte ranges. Start/token/finish
  events compact into a flat preorder node table. Parent links are built lazily.
- [Parse result](https://github.com/zayenz/zirium/blob/00f5cc6c1001cb60ac6dadb980d309e57449a702/crates/zirium/src/parser.rs):
  `ParsedFile` owns source, syntax, diagnostics, and limits. Recoverable syntax
  errors leave a usable result. This is a useful contract for editor-facing tools.
- [Recovery](https://github.com/zayenz/zirium/blob/00f5cc6c1001cb60ac6dadb980d309e57449a702/crates/zirium/src/parser/grammar/recovery.rs):
  error nodes and explicit progress checks keep recovery from getting stuck.

Borrow the separation of source, syntax, and optional semantics. Retain all tokens
once in source order, including trivia and bad tokens, in Zincite's lexer.
Typed views can make lint rules readable without constructing a second tree.
Do not begin with Zirium's compact event encoding, retention profiles, registry,
query engine, or semantic editing machinery. Those solve additional MLIR needs.
Invalid UTF-8 preservation is a Zirium capability, not yet a requirement here.

## Shackle: inspect current source rather than cached layout

The inspected `develop` tree was
[`d9f5d8243ec3b0424febf0247f83d30706369b00`](https://github.com/shackle-rs/shackle/tree/d9f5d8243ec3b0424febf0247f83d30706369b00).
The [README](https://github.com/shackle-rs/shackle) describes its components as
early-stage and unstable. The current [workspace manifest](https://github.com/shackle-rs/shackle/blob/d9f5d8243ec3b0424febf0247f83d30706369b00/Cargo.toml)
splits syntax, diagnostics, HIR, types, typed HIR, formatting, and language-server
code. Cached manifests showing `shackle-compiler` describe an older layout.

| Component | Observed behavior | Use here |
| --- | --- | --- |
| `tree-sitter-minizinc` | MiniZinc grammar with operator precedence, comments, interpolation, and Rust bindings | Reference for grammar questions and edge cases |
| `shackle-syntax` | Tree-sitter CST wrapper and typed MiniZinc views | Reference for typed syntax access |
| `shackle-diagnostics` | Source and diagnostic types used by syntax and formatting | Reference for diagnostic design |
| `shackle-fmt` | Public `format` and `format_str`; builds CST, checks errors, formats typed model | Reference for formatting behavior |
| HIR/type crates | Separate semantic layers in the workspace | Reference if Zincite's lint rules require semantic analysis |

The formatter's [entry point](https://github.com/shackle-rs/shackle/blob/d9f5d8243ec3b0424febf0247f83d30706369b00/crates/shackle-fmt/src/lib.rs)
rejects syntax errors before formatting. Its [manifest](https://github.com/shackle-rs/shackle/blob/d9f5d8243ec3b0424febf0247f83d30706369b00/crates/shackle-fmt/Cargo.toml)
uses syntax and diagnostics as ordinary dependencies; `shackle-hir` is a development
dependency. This separation is a useful architectural reference for Zincite.
Comment-formatting tests exist, but we have not run them or assessed our preferred style.

The [CST wrapper](https://github.com/shackle-rs/shackle/blob/d9f5d8243ec3b0424febf0247f83d30706369b00/crates/shackle-syntax/src/cst.rs)
stores a Tree-sitter `Tree`, takes `&str`, and accepts external source text for text
access. Its constructor currently parses with no old tree. Tree-sitter support
does not by itself establish an incremental API in this wrapper.
The [grammar](https://github.com/shackle-rs/shackle/blob/d9f5d8243ec3b0424febf0247f83d30706369b00/parsers/tree-sitter-minizinc/grammar.js)
lists whitespace and comments as extras. Check how consumers recover whitespace
gaps from retained source; do not assume every byte is an explicit leaf token.

## Design cases for Zincite

Ground the parser design in a handful of representative cases:
comments between operands, trailing comments, string interpolation, nested
comprehensions, quoted operators, and an error followed by a valid declaration.
Check source coverage, ranges, typed traversal, error recovery, formatting behavior,
and build requirements. Keep these checks focused on Zincite's source-tooling
contract rather than reproducing another project's architecture.

The upstream workspace declares MPL-2.0. Zirium declares MIT OR Apache-2.0.
No source has been copied and no dependency has been added here. These license
facts describe the references; Zincite's own license remains undecided.

Use the [MiniZinc specification](https://docs.minizinc.dev/en/stable/spec.html)
as language authority. The inspected stable page identified handbook 2.10.1.
Pin the selected release when defining compatibility; the `stable` URL can change.
