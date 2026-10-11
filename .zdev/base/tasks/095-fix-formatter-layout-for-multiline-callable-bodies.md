+++
schema_version = 1
id = "base-095"
key = "format-callable-bodies"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Fix formatter layout for multiline callable bodies

## Outcome

Multiline predicate and function bodies have a clear boundary from the signature and consistent body indentation.

## Context

Observed while formatting the Gecode MiniZinc registry PR with the local Zincite source checkout at 6edaf0a. Reproduce with zincite-fmt --indent-size 2 --max-line-length 120. Read the area brief Formatting direction and start in crates/zincite-fmt/src/layout.rs and tests/formatting.rs. The output reparses and is idempotent; this is a readability problem, not evidence of changed model behavior.

Input:

```minizinc
function var int: product(array[$$X] of var int: x) :: promise_commutative =
  if mzn_in_root_context() then
    let {
      var int: result :: is_defined_var;
      constraint native_product(array1d(x), result) :: defines_var(result);
    } in result
  else
    product_rec(array1d(x))
  endif;

predicate gecode_all_different(array[int] of int: offset, array[int] of var int: x) =
  gecode_all_different_offset(offset, x);
```

Current output puts the first if after the long signature and prints else/endif at the declaration indentation. The long gecode_all_different signature also makes the short gecode_all_different_offset call expand. The same behavior affects = let bodies and bodies with a leading comment in gecode.mzn and the redefinitions files.

## Boundaries

- Keep this a formatting change: preserve names, comments, literals, annotations, expression grouping, and declaration order. Do not change the parser or linter unless a demonstrated formatter requirement needs it.

## Done when

- [x] A multiline if/let body starts on a new line after = and is indented relative to the callable header; else/endif and closing let braces follow the same body indentation.
- [x] A short wrapper call can remain compact on the indented body line when its signature would otherwise force unnecessary argument expansion.
- [x] Leading body comments retain their text and indentation with the expression they explain.
- [x] A multiline expression after `let { ... } in` stays at the let indentation instead of adding another level.
- [x] A direct `if`/`elseif` condition using `forall` with a single short body may remain inline when the whole condition fits; comments and genuinely multiline bodies retain their layout.

## Validation

- Add a few focused formatter regressions for the examples, including a comment-led body; check formatting at two- and four-space indentation.
- Check reparsing, token/comment preservation, width behavior, and second-pass idempotence; run the existing area Cargo checks.

## Result

Fixed callable body layout and comment attachment, aligned let-in bodies with let, and allowed fitting short direct if/elseif forall conditions while preserving multiline-body stability.

Validation:

- Independent whole-task review passed; snapshot W56562b7921da3f79 comparison equal true.
- cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace and zdev check passed.
- Focused old-failing/new-passing regressions and independent CLI checks verify two/four-space layouts, widths 50/120, token/comment/structure preservation and second-pass idempotence.
