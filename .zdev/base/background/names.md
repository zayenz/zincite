# Zincite naming decision

Selected by the user on 2026-09-29: **Zincite**, with `zincite-fmt` and
`zincite-lint` as executable names. Use the `zincite-` prefix for subcrates, starting
with the candidate `zincite-syntax`, `zincite-fmt`, and `zincite-lint` boundaries.
This supersedes the original `mznfmt`/`mznlint` naming preference. The checkout
directory is `zincite`.

## Name reservations, 2026-09-30

Published empty `0.0.0-pre` releases on crates.io for
[`zincite`](https://crates.io/crates/zincite),
[`zincite-syntax`](https://crates.io/crates/zincite-syntax),
[`zincite-fmt`](https://crates.io/crates/zincite-fmt), and
[`zincite-lint`](https://crates.io/crates/zincite-lint).
Each package contains an empty library, a placeholder README and the project's
MIT and Apache-2.0 licenses, with no dependencies or executable commands. All four
versions were confirmed in the public registry index. The packages link to
[`zayenz/zincite`](https://github.com/zayenz/zincite).

These releases reserve the names; they do not distribute the implementation.
The development workspace remains at version `0.1.0`.

The original shortlist is retained below to explain the choice.

| Name | Connection and trade-off | Example crate |
| --- | --- | --- |
| **Zincite** | Selected. A zinc oxide mineral: a direct Zinc connection, a natural word, and the desired z-prefix. Shares search results with the mineral. | `zincite-syntax` |
| **Zinque** | Short, distinctive spelling inspired by Zinc. Less obvious pronunciation and spelling when spoken. | `zinque-syntax` |
| **Zincraft** | Zinc plus craft; clearly suggests tools. More literal and less compact than Zincite. | `zincraft-syntax` |
| **Zinform** | Zinc plus form, with an echo of informing the modeller. The connection to MiniZinc is weaker. | `zinform-syntax` |
| **Zinkstone** | Zinc-inspired material name with room for a tool family. Longer and has a deliberate spelling variation. | `zinkstone-syntax` |

The mineral connection is documented by
[CSIRO's Zincite entry](https://spectroscopy.csiro.au/material/Zincite).
These are naming judgements, not claims of official MiniZinc affiliation.

## Registry checks, 2026-09-29

Direct requests to crates.io's API returned HTTP 403. The public sparse index was
accessible. Exact entries below returned HTTP 404 (`NoSuchKey` for the first
inspected response); known occupied names `serde` and `zinnia` returned HTTP 200.
This is evidence that these names were not published in the index at check time,
not a reservation or a guarantee that publication will be accepted.

| Prefix | Root | `-syntax` | `-fmt` | `-lint` |
| --- | --- | --- | --- | --- |
| `zincite` | 404 | 404 | 404 | 404 |
| `zinque` | 404 | 404 | 404 | 404 |
| `zincraft` | 404 | 404 | 404 | 404 |
| `zinform` | 404 | 404 | 404 | 404 |
| `zinkstone` | 404 | 404 | 404 | 404 |

`mznfmt` and `mznlint` also returned 404. `zinnia` is occupied and is not shortlisted.
Underscore variants of `zincite`, `zinque`, and `zincraft` with the three suffixes
above also returned 404, including `zincite_syntax`, `zincite_fmt`, and `zincite_lint`.
Representative checks: [zincite](https://index.crates.io/zi/nc/zincite),
[zincite-syntax](https://index.crates.io/zi/nc/zincite-syntax),
[mznfmt](https://index.crates.io/mz/nf/mznfmt),
[mznlint](https://index.crates.io/mz/nl/mznlint).

Recheck exact package names before publishing, including hyphen/underscore
equivalents. The suffixes above illustrate naming fit; they are not a commitment
to publish that many crates. Repository/domain availability and broader naming
conflicts have not been checked.
