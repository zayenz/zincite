# Documentation site

The Astro/Starlight site renders the reader guides in `../docs/`. Those Markdown
files remain authoritative and readable on GitHub. `scripts/prepare-content.mjs`
adds page metadata and rewrites guide links into site links at build time.
Other repository links point to the matching `v0.2.0` source. Generated content
is ignored. Edit the guides in `docs/`, not generated site content.
The landing template lives in `src/landing.mdx`; its installation commands come
from `docs/getting-started.md`, and its example follows `docs/first-model.md`. Logos and favicon originals
remain in `../assets/`.

The design follows zayenz.se's restrained navigation, dark charcoal surfaces,
readable prose and Avenir/Georgia typography, with Zincite's orange crystal and
wordmarks. Starlight owns search, theme selection and responsive navigation.
The TextMate grammars in `src/minizinc.mjs` and `src/zirql.mjs` provide
presentation highlighting. Use `minizinc` for models and data, and `zq` (or
`zirql`) for query programs. Keep command output in `text` fences. The query
grammar follows Zirql's `assets/syntax/zirql.tmLanguage.json`, with Zincite's
program statements and MiniZinc expression syntax added. Update the shared
grammar through the synchronization procedure below. Highlighting does not
validate syntax or promise that Zincite accepts every Zirql construct.

Keep the MiniZinc source operations, output shapes and data-edit restrictions in
the Zincite guides. Link to a specific Zirql guide when readers need the shared
expression, collection or function rules. The local example should still be
usable without following that link.

Give saved-file examples a title after the language name, for example
`zirql title="shortages.zq"`. Show input files directly in named code
blocks; keep shell blocks for the commands that use them. Label a printed data
candidate with `title="Standard output"` so readers can distinguish it from a
file they need to create.

## Build and preview

Website work needs Node.js 22.12 or later, npm, the Rust toolchain and MiniZinc's
standard library. Content preparation builds the current `zincite-lint` and runs
the marked lint examples. Set `MZN_STDLIB_DIR` to the MiniZinc library root
containing `std/stdlib.mzn`, or put `minizinc` on `PATH` so preparation can read
the path from `minizinc --config-dirs`. No solver is run.

Rust contributors need only the Rust toolchain; no Cargo command invokes the
site build.

From the repository root:

```sh
cd site
npm ci
npm run check
npm run build
npm run preview -- --host 127.0.0.1
```

Open the preview URL printed by Astro. Search uses the Pagefind index generated
by `build`; use the built preview to check search. `npm run dev` starts the local
editing server. Restart it after changing a guide outside `site/` so the content
preparation step runs again.

## Site URL and base path

Set `SITE_URL` to the intended HTTP(S) origin and `SITE_BASE` to its path. The default
origin is the reserved `https://zincite.example`; it is a local build placeholder,
not a published URL. Publication must supply the real origin.

Build for a dedicated domain:

```sh
SITE_URL=https://zincite.example SITE_BASE=/ npm run build
SITE_URL=https://zincite.example SITE_BASE=/ npm run preview
```

Build for a subdirectory on zayenz.se:

```sh
SITE_URL=https://zayenz.se SITE_BASE=/zincite/ npm run build
SITE_URL=https://zayenz.se SITE_BASE=/zincite/ npm run preview
```

Use the same values for build and preview. Astro supplies canonical URLs and
Starlight prefixes navigation, search and assets with the configured base path.
Output is in `dist/`. These commands do not deploy the site or change DNS.

## Focused validation

Run `check` and `build` for website changes. Preview both base paths and check the
landing page, a guide with MiniZinc examples, the reference tables, search, theme
selection and mobile navigation. Follow internal guide links and a search result.
Use Tab and Escape to check focus and search dismissal. Code blocks and wide
tables should scroll within the content, without widening the page.

No site build is wired into Rust checks. Build the website explicitly when its
content, configuration or appearance changes.

## Generated lint responses

Put `<!-- zincite-lint: RULE-ID -->` immediately after a MiniZinc code block,
separated by a blank line, to show the linter's current response on the site.
The code block must demonstrate the selected rule without extra setup files.
For example, the `naming` example in `../docs/lint-conventions.md` uses
`<!-- zincite-lint: naming -->`.

`scripts/lint-examples.mjs` builds the workspace's linter once per preparation,
runs each marked block in a temporary directory with isolated settings and
captures its stderr verbatim. Relative filenames keep machine-specific paths
out of the response. It adds the command and diagnostic block to generated site
content; the source Markdown keeps the example and marker. The build fails on
an execution error or if the example no longer produces its named warning.

`build`, `check` and `dev` all run content preparation. After changing Rust code
or the source guides, rerun the command to regenerate the responses. Do not edit
generated diagnostic text by hand.

## Publication

The chosen canonical URL is `https://zayenz.se/zincite/`. Follow
[the publication procedure](PUBLISHING.md) to import a reviewed build into the
existing personal-site deployment, verify it over HTTPS and retain a rollback.

## Zirql syntax highlighting

Use `zirql` or `zq` for query code fences. Zirql owns the common grammar;
`../assets/syntax/zirql.tmLanguage.json` is a checked-in copy from the full
commit in `../assets/syntax/zirql-revision.txt`. Do not edit that copy directly.
From a Zirql checkout, run its revision-pinned synchronization commands:

```sh
node scripts/sync-syntax.mjs sync <full-Zirql-commit> ../zincite ../zirium
node scripts/sync-syntax.mjs check <full-Zirql-commit> ../zincite ../zirium
```

Review and commit the grammar and revision marker here, then run this site's
`check` and `build` commands. Builds read the local asset and need no Zirql
checkout. See [Zirql's contributor procedure](https://github.com/zayenz/zirql/blob/main/site/README.md#syntax-highlighting)
for the common update path.

`src/zirql.mjs` retains Zincite's statement/edit keywords and MiniZinc operators.
Escaped single-quoted names and the uppercase octal prefix remain deliberate
legacy highlighting allowances; they do not imply parser support. Common percent/hash
line comments and block comments come from Zirql. Highlighting describes
spelling; parsing and host policy decide which constructs a query may execute.
