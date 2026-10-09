+++
schema_version = 1
id = "readability-002"
key = "global-use-traversal"
area = "readability"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Make global-use traversal responsibilities visible

## Outcome

Global-use traversal visibly separates calls and conditional branches while preserving Boolean enforcement and diagnostic behavior.

## Context

crates/zincite-lint/src/global_uses.rs Walker::walk combines call recording, core-forall argument traversal, conditional context selection and ordinary recursive traversal. The call and conditional blocks each already consume file, item, node and enforcement context and terminate their own traversal path. Extracting private methods for those jobs makes the controlling walk readable without a new interface. The reviewer and coordinator checked these boundaries against existing code and global-use tests. Begin at Walker::walk and tests/global_uses.rs.

## Boundaries

- Own global_uses.rs only; existing tests/global_uses.rs are validation evidence. Confirm reservation and shared-main source/commit window with the implementation chat before editing.
- Retain Walker state, traversal order, recursive walk, tri-state enforcement, standard/core/user callable distinctions, and current limitations. Do not introduce a visitor framework, new crate, public interface or semantic rewrite.
- Extract only distinct call and conditional traversal jobs; no function-size target and no unrelated helper renaming.

## Done when

- [ ] Walker::walk exposes the call and conditional paths through clearly named private methods with their existing inputs and effects.
- [ ] Existing global-use behavior is unchanged for enforced globals, quantifiers, decision-dependent/unknown branches, annotations and let-local constraints.
- [ ] The relevant baseline and required workspace validation pass, with no new tests.

## Validation

- Run existing global-use tests before and after the refactor; compare moved code and early returns directly.
- Run the workspace validation commands in the readability brief. No corpus or performance campaign is required for this behavior-preserving extraction.
