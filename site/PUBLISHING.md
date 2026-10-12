# Publish the documentation site

Zincite's canonical site is `https://zayenz.se/zincite/`. The personal site's
existing Netlify project, `zayenz-web` (`36755581-737f-42cb-ac2f-a16d37fcd00f`),
publishes it alongside the rest of zayenz.se. Build Zincite separately and import
its static output into the personal site's `public/zincite/` directory.

The personal site's production branch is `main`. Its hosted build runs
`npm run release:build` and publishes `dist/`. Follow that repository's release
instructions and Git-based publication policy. Netlify blocks non-Git production
deploys (`prevent_non_git_prod_deploys`). Keep its existing build command,
functions, environment, domain and DNS settings.

## Prepare an update

Use a reviewed, committed Zincite revision. Confirm the examples and support
claims describe that revision, including any unfinished query or semantic work.
The Markdown guides in `docs/` remain authoritative.

From the Zincite checkout:

```sh
cd site
npm ci
npm run check
SITE_URL=https://zayenz.se SITE_BASE=/zincite npm run build
SITE_URL=https://zayenz.se SITE_BASE=/zincite npm run preview -- --host 127.0.0.1
```

Open the preview at `/zincite/`. Check the landing page, a guide, reference tables,
search and a search result. The generated directory contains the contents of
`/zincite/`; do not add another `zincite` level when copying it.

Before importing, record the personal site's current production deploy ID,
production source commit and immutable deploy URL. Retain that deploy as the
rollback point. Compare the chosen personal-site source commit with production;
unpublished changes need their own review before they enter the release.
Preserve existing local changes and the Git index.

In a clean publication checkout of the personal site, inspect whether
`public/zincite/` already exists. Preserve the previous imported directory before
replacing it. Copy the new `site/dist/` contents there, including `_astro/`,
`pagefind/`, icons, guides and the Zincite sitemap files. Commit the imported
output so the existing hosted build can reproduce the publication without a
second repository checkout or new credentials. Omit the generated `404.html`: the
personal site owns Netlify's error page, and Zincite's standalone error page links
to a directory route that is absent from the import. Record the Zincite source commit
in the import commit message.

Review the diff. It should contain only the intended Zincite import and any
explicitly reviewed publication instructions. Run the personal site's required
`npm run release:check`; keep browser checks manual. Verify the resulting
`dist/zincite/` contains the landing page, guide pages, search index and assets.
Publish the reviewed commit through the existing Git-based production route.

## Verify the published update

Wait for Netlify to report a ready production deploy of the intended personal-site
commit. Record its deploy ID and the imported Zincite revision. Check the actual
HTTPS site, rather than treating the local build as publication evidence:

- `/` and an existing personal-site page still work.
- `/zincite` reaches `/zincite/`; the landing page, guide navigation and a reference
  table load at their expected paths.
- Search opens, returns a known guide and follows its result. Search scripts,
  Pagefind files, styles, logo and favicon load under `/zincite/`.
- Canonical metadata uses `https://zayenz.se/zincite/` and the matching guide URLs.
  `/zincite/sitemap-index.xml` and its linked sitemap contain those URLs.
- At a narrow mobile width, navigation opens and closes, code and wide tables
  scroll within the content, and the page has no horizontal overflow.
- Tab reaches navigation and search, Escape dismisses search, and focus remains
  visible. Representative copy buttons produce the commands shown in the guides.

Retain the URL, revision, deploy IDs, checks and any limitations in the publication
record. A failed asset or search request leaves the update unverified even when
Netlify reports a successful deploy.

No alternate Zincite URL is configured by this procedure. If an alternate is
introduced later, review its redirect to the canonical URL before publication.

## Roll back

Keep the prior deploy available until the smoke checks pass. If an update fails,
revert the import commit in the personal-site repository and publish the revert
through its normal Git-based route. This restores the previous Zincite directory;
for a first publication, it removes that directory. Check the personal homepage
and affected paths after the rollback.

Use Netlify's saved deploy only if the existing production policy permits that
restore method. Do not disable deployment restrictions to make rollback work.
The Git revert remains the repeatable recovery path and preserves unrelated
personal-site content.
