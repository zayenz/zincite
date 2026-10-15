# Documentation site

The Astro/Starlight site renders the reader guides in `../docs/`. Those Markdown
files remain authoritative and readable on GitHub. `scripts/prepare-content.mjs`
adds page metadata and rewrites repository links into site links at build time;
its output is ignored. Edit the guides in `docs/`, not generated site content.
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
program statements and MiniZinc expression syntax added. Keep shared keywords
and types aligned when updating it; highlighting does not validate syntax or
promise that Zincite accepts every Zirql construct.

## Build and preview

Website work needs Node.js 22.12 or later and npm. Rust contributors need only the
Rust toolchain; no Cargo command invokes the site build.

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
SITE_URL=https://zayenz.se SITE_BASE=/zincite npm run build
SITE_URL=https://zayenz.se SITE_BASE=/zincite npm run preview
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

## Publication

The chosen canonical URL is `https://zayenz.se/zincite/`. Follow
[the publication procedure](PUBLISHING.md) to import a reviewed build into the
existing personal-site deployment, verify it over HTTPS and retain a rollback.

## Zirql syntax highlighting

Use `zirql` for query code fences. The grammar in
`../assets/syntax/zirql.tmLanguage.json` is an unchanged copy of the
[shared Zirql grammar](https://github.com/zayenz/zirql/blob/main/assets/syntax/zirql.tmLanguage.json).
Copy updates from that file to keep the three documentation sites consistent.
`src/zirql.mjs` adds Zincite's surface syntax using the same token scopes.
Highlighting does not validate queries.
