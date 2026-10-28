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

Use the Zincite 0.4.0 release source. Confirm the examples and support
claims describe that revision, including any unfinished query or semantic work.
The Markdown guides in `docs/` remain authoritative.

From the Zincite checkout:

```sh
cd site
npm ci
SITE_URL=https://zayenz.se SITE_BASE=/zincite npm run check
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

In a clean publication checkout of the personal site, use its project-site import
command. The project is registered as `zincite` and published at `/zincite/`:

```sh
npm run import:project -- --project zincite --source /absolute/path/to/zincite/site/dist --dry-run --replace
npm run import:project -- --project zincite --source /absolute/path/to/zincite/site/dist --replace
```

Use the absolute path to the output built above. `--dry-run` previews the import;
`--replace` explicitly allows replacing an existing imported directory. A first
import into an absent directory does not need `--replace`. The command checks
the static output and places it in `public/zincite/`. It omits `404.html` because
the personal site owns the error page, and keeps Zincite's assets, guides, search
index and sitemaps together. Do not edit the generated output by hand.

Commit the imported output so the hosted build can reproduce publication without
a second repository checkout or new credentials. Record the Zincite source commit
in the import commit message. Git retains the previous import for rollback.

Review the diff. It should contain only the intended Zincite import and any
explicitly reviewed publication instructions. Publish the reviewed commit
through the existing Git-based production route, which runs the personal site's
release checks. Follow the current repository release policy; a documentation
update does not require repeating those checks locally.

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

Retain the URL, revision, deploy IDs and any observed failures in the publication
record. A ready production deployment is sufficient release assurance. The
checks above help investigate a concrete delivery problem; they are not a second
release gate after a successful deployment.

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
