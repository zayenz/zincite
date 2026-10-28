# Local workshop schedule

This page checks edits to the existing two-job model: job 1 cuts panels in three
hours, then job 2 assembles them in two. It loads one parser-free Wasm
`PreparedModel` and reuses it for each edit, including infeasible schedules and
omitted input. No frontend framework, solver, compiler service or account is used.

## Build, export and serve

Run these commands from the repository root. The prerequisites and versioned
tool installation commands are in [the Wasm example](../checker-wasm/README.md).
The checked setup uses Rust 1.98.1, wasm-bindgen-cli 0.2.129, Node.js 24.22.1 and
Python 3 for a static local server.

```sh
cargo build --locked -p zincite-check --lib --target wasm32-unknown-unknown --no-default-features --features wasm --release
mkdir -p target/checker-browser
wasm-bindgen --target web --out-dir target/checker-browser/web target/wasm32-unknown-unknown/release/zincite_check.wasm
cargo build --locked -p zincite-check --bin zincite-check
rm -f target/checker-browser/schedule.json
target/debug/zincite-check --export target/checker-browser/schedule.json examples/checker-compilation/model.fzn
cp examples/checker-browser/index.html examples/checker-browser/style.css examples/checker-browser/app.js target/checker-browser/
python3 -m http.server 8000 --bind 127.0.0.1 --directory target/checker-browser
```

Open <http://127.0.0.1:8000/>. Stop the server with Ctrl+C. All generated bindings,
Wasm, prepared JSON and served copies live under the ignored `target/` directory.
After editing page source, repeat the `cp` command and reload the browser.

The export command prepares the saved compiled model; it does not invoke
MiniZinc. For an intentional source-model change, first use the separate
[compilation recipe](../../crates/zincite-check/README.md#compile-the-application-model-offline) and inspect its output.
Ordinary schedule edits never recompile the model. Initial page loading makes
only local static HTTP requests for the page, CSS, JS, Wasm and prepared JSON.
After loading, edits call Wasm directly without HTTP requests.

## Try the edits

1. **Initial plan** supplies `[1,4]`: complete and valid, with no findings.
2. **Both start at 8** supplies `[8,8]`: complete and invalid, with four individual
   findings. The two finishes (11 and 10) exceed their domains, cutting finishes
   3 hours after assembly starts, and the start sum is 16 against a budget of 10.
   Each job shows three findings: its own finish, precedence and budget. Both late
   finishes extend into the shaded part of the timeline beyond closing at hour 9.
3. **Recover with 2 and 5** supplies `[2,5]`: complete and valid. The old findings
   and job highlights disappear. Move either labelled slider for further edits;
   the focused native control supports arrow keys, Home and End.
4. Select **Omit start times from the check**. The assignment becomes
   `{ "entries": [] }`. The ordinary report is incomplete with no established
   violations, nine unresolved value references and fourteen blocked checks.
   Bars hide because these start times have not been supplied. Clear the checkbox
   to check the current controls again, or choose an example plan.

The developer disclosure shows the exact assignment and JSON report for the
current edit, including structured completeness. These are useful for comparison
with the native CLI's `--load --format json target/checker-browser/schedule.json` path
and the native `schedule-feedback` example. The ordinary interface uses job names
and scheduling terms.

## Application mapping and measurement seams

`app.js` owns the two application jobs and their positions in `start` and `finish`.
`assignmentFor` sends the whole start array using decimal integer strings, or
omits it entirely. The page only uses JavaScript numbers for its bounded timeline
coordinates; report integers keep their exact string representation.

`affectedJobs` uses constraint application entity payloads directly. For domain
findings it uses the variable's application ID plus its array alias position to
choose the job from the entity payload. `feedbackMessage` chooses wording from
`CheckRef`, `ViolationKind` and typed values, as the native example does. It does
not parse checker-written prose, labels or paths, deduplicate shared IDs, or
evaluate a second set of scheduling constraints. The fixed durations only draw
the job bars; the checker decides whether the schedule is valid.

Preparation is the offline native `--export` step. In the browser, `start` loads
Wasm and the artifact and constructs one model. `checkSchedule` builds a fresh
assignment, calls `model.check`, parses the JSON and maps findings;
`renderSchedule` and `renderReport` update the view. These direct boundaries allow
the performance task to measure loading, repeated checks, boundary/report work
and rendering separately without changing the checker or introducing a benchmark
framework. No timing threshold is asserted here.

Reuse the existing [full native/Wasm smoke checks](../checker-wasm/README.md#execute-the-nativewasm-comparison)
after building. Browser acceptance additionally covers the four interactions
above, keyboard edits, successive report replacement, the narrow layout and the
absence of network requests during edits.


The [interactive checker measurements](../../scripts/checker-performance.md) use
this model and a five-state edit cycle, including infeasible, omitted and partial
input. The separate measurement page times the Wasm/JSON path and growing models;
it does not include this page's DOM rendering. Follow its commands to save,
resume and analyze a named run alongside the workshop server.

## Acceptance on 2026-10-28

The rebuilt release Wasm passed actual Chrome interactions with **Initial plan**,
**Both start at 8**, **Recover with 2 and 5**, and **Omit start times from the check**.
The invalid edit showed four individual findings and three findings per job;
recovery cleared the old findings and highlights. Omission hid the bars and
reported nine unresolved references and fourteen blocked checks. Choosing a plan
after omission restored a complete valid report. This graphical acceptance is
separate from the measurement page's Wasm/JSON timing, which excludes DOM work.
