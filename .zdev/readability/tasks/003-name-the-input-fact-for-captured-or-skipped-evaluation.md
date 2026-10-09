+++
schema_version = 1
id = "readability-003"
key = "input-obligation-name"
area = "readability"
status = "open"
complexity = "routine"
afk = true
priority = "normal"
blocked_by = []
+++
# Name the input fact for captured or skipped evaluation

## Outcome

The public CallableInputFact Boolean accurately names and documents both default-captured undefinedness and evaluation skipped by proved empty iteration.

## Context

In crates/zincite-lint/src/input_preconditions.rs, CallableInputFact.captured is documented as supported core-default capture, but its producer computes captured_by_default(...) || empty_iteration(...). Consumers only use it to exclude a precondition finding. Rename the field and local binding to captured_or_skipped and document both existing reasons. The only current external field access is in tests/input_preconditions.rs. The reviewer and coordinator checked this producer/consumer mismatch; it is a naming/contract correction, not a warning behavior change.

## Boundaries

- Own input_preconditions.rs and the mechanical access rename in tests/input_preconditions.rs; confirm reservation and shared-main source/commit window first.
- The public field rename is intentional. Preserve its bool type, computation, facts, warning policy and all other public names/signatures.
- Do not replace the Boolean with an enum, add compatibility aliases, new tests or semantic machinery.

## Done when

- [ ] CallableInputFact.captured_or_skipped and its producer binding accurately identify the existing two cases, and the documentation distinguishes capture from skipped evaluation.
- [ ] All in-repository field accesses use the new name; precondition analysis and warnings remain unchanged.
- [ ] Existing input-precondition and required workspace checks pass; no new tests are added.

## Validation

- Run existing input-precondition tests before and after the mechanical change; search for remaining old field accesses.
- Run the workspace validation commands in the readability brief.
