# Expanded lint checkpoint

Base-074 validates the ten added rules and the settings/fix workflow together.
The current production linter is built at
`1cc9aa7b8669e619840037cedc4260b7c844f5d4`; this checkpoint changes the two existing
Python drivers, their two Rust developer examples and documentation. It adds no
rule, fix, semantic interpretation or optimization.

The user-facing commands and supported limits are in
[docs/linting.md](../docs/linting.md). The matched measurements are in
[batch-performance.md](batch-performance.md). Raw input/binary hashes, commands,
streams, deadlines, findings, partitions and checks are retained under ignored
`target/base074/`; validation and ownership indexes are linked from the task
handoff. Private corpus sources are read-only and are not published as fixtures.

## Tools and routing

Both drivers now accept existing explicit selectors through the Rust catalogue.
The batch driver records ordered IDs for each selection and uses short `sN`
stream names, so long comma-separated lists do not exceed filename limits.
`--native-only` retains timing/discovery/original hashes while marking companion
coverage as not collected. The default still collects the companion. A manifest
made by the current probe is distinct from the retained binary being timed.

The corpus checker follows native selection routing. Model-dependent selections
load/analyse the include closure even when root syntax is rejected. Source-local
selections parse/analyse the source without include loading. Default and style
checks on a missing-include control both remain clean; a semantic selection
retains the dependency errors. Unreadable semantic roots retain API errors and unavailable source coverage,
with the actual recorded per-rule outcomes (all ten Limited on the invalid-UTF-8
control); source-local input failures retain
Unobserved rule results. Syntax/format mode remains available without `--rules`.

The corpus driver records every canonical inventoried root. Per-file and campaign
deadlines are finite; children have file-backed streams and are reaped after
termination. A campaign cutoff leaves every remaining rule/root Unobserved rather
than treating the unattempted rows as completed. Pre-campaign hashes remain
authoritative when the same path later appears as a dependency. Later-only
observed dependencies keep their first-observed hashes separately. Original raw
failure and cutoff rows remain available.

## Integrated public behavior

The staged synthetic model uses the installed MiniZinc 2.10.1 standard library.
Its explicit new10 selection reports 11 warnings covering every new ID: one each
except two unused-generator names. The independent library replay parses the same
TOML, resolves the same CLI replacement/options/fix restrictions, loads the same
model and calls `analyze_model`. It reports all ten rules Completed, zero errors
and limitations, and byte-identical full `write_analysis` stderr/status 1.
`integration/library-reconciliation.json` retains this comparison.

The settings outputs and `settings-reconciliation.json` establish ordered
selection and independent options: the parent personal family preset expands all
26, a nearer child config selects naming alone, explicit parent config overrides
that child, isolated/default select two, thesis selects fourteen and all selects
26. CLI naming replacement keeps the threshold 10, ignored name and fix
restrictions. The personal threshold 100 is overridden by root threshold 10;
fixable names both producers while unfixable excludes element independently of
diagnostic selection. Stdin clean/warning and broken input retain status 0/1/2.

The guarded neighbour/callable control stays quiet. Joint indexing and partial
selection emits one indexing access finding plus the distinct division hazard.
Suppressing indexing restores standalone partial advice for that same access.
Existing public producer/library/CLI checks cover unknown/user-overloaded
operations, raw totality, optional elements, enum/dimension identity, relevant
assumptions, typed thresholds, alias visibility, unsafe supplied groups, fix
restrictions and the remaining same-access dimension/dedup boundaries. These are
behavior checks, not a declaration of full semantic coverage.

## Actual fixes and compiler checks

`native/results.json` records preview, apply, second apply and final preview with
statuses 1/0/0/0. The model retains BOM, Unicode comment, CRLF, labels, surrounding
comments and exact expected slices. Two unused binders become `_`; both written
Cartesian dimensions remain, giving nine repeated terms of two and sum 18. The
reified non-one-based standard element call becomes a parenthesized equality and
array access. Compatible scalar types, scoped index membership, standard body
identity, raw totality and annotation admission establish the supported Safe
case. The edit retains operand evaluation and its Boolean attachment.

The second application changes no byte and the final diff is empty. The configured
standard element file stays unchanged. Generator-call shorthand, unknown or unsafe
evaluation, user/shadowed equality and unsupported annotations retain advice-only
behavior in the existing public checks; no quoted underscore or restructuring is
used to force a fix. Only staged synthetic/public copies are fixed.

MiniZinc 2.10.1 rejects the exact BOM-prefixed before/after files at byte 1. Those
four model-check/compile failures remain in `compiler/results.json`. Counterparts
removing only their shared three-byte BOM pass both model-check-only and explicit
Gecode compile-only with the same installed stdlib. `compiler-no-bom/results.json`
pins both copies and the unchanged originals. No solver ran; compiler acceptance
supplements the concrete eligibility/preservation argument and proves no general
semantic equivalence.

## Available corpus and observed limits

The full campaign retains the same challenge/local/supplement directories and
installed stdlib as base-049. Canonical inventory contains 6,417 roots, with no
missing tracked challenge input or inventory error. The checker uses the new10
selection, a ten-second per-file limit and a 600-second campaign limit. It
attempts 921 roots: 886 return API reports and 35 time out. The remaining 5,496
roots are explicitly unattempted. Every rule has 5,531 Unobserved rows, including
the 35 timeouts. `full-corpus/results.jsonl`, `inventory.json`, `summary.json` and
`reconciliation.json` retain all 64,170 rule/root outcomes, including failures.
“Checked” in the summary means every inventory row was accounted for; it does
not mean every root was analyzed.

The reports observe 738 data roots, 146 structurally complete models and two
fragments. Of the complete models, 55 have resolved dependencies and only three
complete all ten rules. Observed totals are 547 warnings, 1,180 errors and 2,807
limitations. Error and limitation causes overlap. The campaign's nonzero status
retains incomplete/failing observations; it is not a clean corpus result.

| Rule | Completed | Inapplicable | Limited | Unobserved | Observed findings |
| --- | ---: | ---: | ---: | ---: | ---: |
| index-set-mismatch | 42 | 738 | 106 | 5531 | 0 |
| hidden-optionality | 57 | 738 | 91 | 5531 | 0 |
| partial-expression | 55 | 738 | 93 | 5531 | 0 |
| suspicious-shadowing | 57 | 738 | 91 | 5531 | 30 |
| vacuous-constraint | 6 | 738 | 142 | 5531 | 0 |
| unused-generator-binding | 52 | 738 | 96 | 5531 | 27 |
| global-constraint-opportunity | 57 | 738 | 91 | 5531 | 0 |
| expensive-comprehension | 49 | 738 | 99 | 5531 | 340 |
| missing-input-precondition | 52 | 738 | 96 | 5531 | 150 |
| suspicious-domain | 27 | 738 | 121 | 5531 | 0 |

All 7,439 pre-campaign original hashes equal the historical full map and remain
unchanged after the campaign. The runner also rechecks 633 observed loaded
paths. The two known `.mzn.model` dependencies have explicit current before/after
hashes; their historical omission remains historical. Later-only dependency
hashes record first observation after analysis, not an invented pre-campaign
capture. Originals and standard-library files remain read-only.

The retained `representative-source-review.json` inspects the four rule IDs with
observed findings. In debruijn_binary, the formal float `base` conceals the root
integer `base`; `toNum` accepts `array[int]` and indexes it over `1..length(t)`
without declaring matching actual index membership. Length alone cannot protect
an offset array. Pentominoes' independent height/width generators match the
symbolic ordered structure message, which gives no numeric threshold or runtime
claim. Filters' `d_add=[del_add | i in add]` leaves `i` unused while sibling
`t[i]`/`r[i]` comprehensions use it; multiplicity must remain. No concrete defect
was established in these samples. Six IDs have no observed corpus findings;
their staged positive/negative public checks remain separate evidence. These
samples supply no corpus-wide false-positive rate or full semantic proof.

The staged quiet control protects `a[i+1]` with `i<3` and division with `d!=0`.
It emits no warnings. Existing public tests also retain unknown data, user
operations, optional/partial behavior, unsafe annotation effects and suppressed
or conditional contexts. No external original was copied into a published test
fixture or changed by fixes.

## Validation and remaining work

The final checker preserves initial bounded semantic/selection streams and
statuses. Valid model/data syntax/format controls retain token/tree coverage,
spelling, structure, protected bytes and idempotence; malformed input retains
parse errors and skipped formatting. The first private replay mistakenly passed
an unsupported `--data` flag; its panic/outer failure remains beside the corrected
filename-inferred replay. A Clippy `collapsible_if` correction changes only the
written routing branch. A final bounded replay links that checker build to the
retained corpus binary without rewriting the full campaign evidence.

The staged cutoff control explicitly retains timed-out/unattempted partitions.
Its temporary challenge directory is not a Git checkout, so its inventory
metadata failures are retained rather than presented as clean corpus coverage.
A private library replay initially passed an unsplit comma string to the vector
selector API; the corrected replay passes separate selectors and matches native
stderr exactly. Failed profiler attaches, native deadlines, compiler BOM failures
and all correction logs remain available.

The existing producer/library/CLI tests supply the safety negatives: callables'
`element_fix_facts_require_actual_indices_total_operands_and_core_equality`,
unused-generator's safety and Cartesian checks, fixes' independent eligibility,
atomic conflict/replacement and explicit Unsafe groups, and CLI's actual two
producer selection/suppression/include-only/second-pass check. All ten rule suites
retain their quiet, unknown, located-limit and suppression cases. No new tracked
tests or production lint changes were needed for this integration checkpoint.

The required Cargo checks and read-only checks are indexed in the task validation
handoff with raw logs and process statuses. All required checks run on final
source bytes. Current native diagnostic/full-batch deadlines and incomplete
semantic coverage remain acceptance gaps. Source-prefix cost is already assigned
to base-078; this checkpoint neither duplicates that repair nor claims acceptance
before the existing correctness/performance follow-ups and final gates finish.
