# Corpus planning evidence

Planning baseline: 2026-09-30, after base-021, with a clean source checkout and
`cargo build --workspace` passing. This is a baseline for task selection, not an
acceptance report. External model sources were read only.

## Local corpus

A recursive filesystem inventory of `~/minizinc/` found 4,287 `.mzn`/`.dzn` files.
Running the current lint command individually on every file yielded 4,048 runs
with exit 0 or 1 and 239 with exit 2; no timeout occurred with an eight-second
per-file limit. Warnings count as completed lint runs. This exercises only the
two existing syntax rules, not semantic lint coverage.

| Subtree | Models | Data | Input/parse failures |
| --- | ---: | ---: | ---: |
| challenge-models | 38 | 763 | 5 |
| competitions | 14 | 0 | 0 |
| library | 2 | 0 | 0 |
| models | 26 | 69 | 3 |
| projects | 94 | 1337 | 23 |
| scratch | 3 | 0 | 0 |
| archive | 1 | 1 | 0 |
| software | 1907 | 32 | 208 |

Software contains compiler tests, libraries and generated copies, including
intentional negative tests. Those 208 errors are not 208 established Zincite bugs.
Personal trees also contain malformed experiments and model declarations in files
named `.dzn`. Keep those classifications separate from missing syntax support.

Demonstrated compatibility cases: MiniZinc 2.10.1 accepts `var 0..infinity: x`,
`var int: x :: output`, and singleton record values `(x: 1)` without the trailing
comma. Zincite currently rejects these forms. `output` annotations recur in the
city-strides project and recent Challenge samples. The 2021 connect models also
exercise infinity bounds and local annotations. MiniZinc 2.10.1 also accepts unnamed annotation parameters such as
`annotation demo(string)` and annotated callable parameters such as
`var int: x :: promise_ctx_monotone`; these occur in standard-library files and
fail in Zincite. Other software errors include reserved-token expressions and
unsupported syntax; inspect compiler expectations before changing their treatment.

For 2,315 parseable files outside software and archive, formatting via stdin with
the original filepath for configuration, then formatting the result again, gave
2,280 stable results and 35 differences. There were no formatter errors/timeouts
in that run. This checks idempotence only, not semantic or comment preservation.
Examples include ATSP, carpet-cutting, mapping data and seating models. The durable
runner must reproduce and group failures before selecting minimal regressions.

## Published Challenge corpus

Use [MiniZinc/mzn-challenge](https://github.com/MiniZinc/mzn-challenge), not only
`minizinc-benchmarks` or local submissions. The archive documents historical
compiler versions and per-problem licensing. At revision
`a8448864fc56162583f24aaf9c25653d93f83765`, its complete recursive tree contained
436 `.mzn` and 1,604 `.dzn` files across 2008–2026. The 2026 directory contained
20 models but no `.dzn` files at that revision; acceptance must report that data
availability gap and check the official year page for published instances.

A planning sample fetched the two smallest models in each year (38 total). The
current linter completed on 35 and rejected three from 2024/2025 using `output`
annotations. This small-file sample is deliberately limited and cannot establish
full archive compatibility. The next corpus task must acquire and run all files.

Read the [2.10.1 specification](https://docs.minizinc.dev/en/2.10.1/spec.html)
for language contracts and use the installed 2.10.1 compiler for targeted
acceptance probes. Do not make historical compiler acceptance the sole measure
of lossless parsing or format preservation. Pair data files with their actual
model invocations when checking complete models; model fragments and negative
tests need different expectations.
