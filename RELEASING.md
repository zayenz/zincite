# Release Zincite 0.1.1

The five workspace crates share version 0.1.1. Their shared-language dependency
remains the published Zirql 0.1.0 release.

## Prepare the source

Update the workspace version, sibling dependency requirements, Cargo.lock,
short changelog entry and current installation/API/source links together.
Keep historical measurement and planning records at their recorded revisions.
The query archive includes both `tests/fixtures/` and `examples/`.

Commit and push the candidate to `main`. Use the existing CI workflow for Rust
formatting, Clippy, tests, API documentation and package-content checks. Trust
successful CI; version and documentation changes do not reopen accepted code
reviews or require fresh installation environments or archive audits.

## Publish the workspace

After successful CI and authorized publication, use Cargo 1.99's normal
workspace publication command:

```sh
cargo +1.99.0 publish --workspace --locked
```

Cargo publishes dependencies before their consumers: `zincite-syntax`,
`zincite-lint`, then `zincite-fmt` and `zincite-query`, and finally `zincite`.
Zirql 0.1.0 must already be available from crates.io. Do not use local registry
or path overrides for publication. If publication stops after some uploads,
retain those versions and continue only the remaining packages; do not replace
published artifacts. Successful CI and normal publication are sufficient
release assurance.

Publish only the intended tag and short release notes after the uploads:

```sh
git tag -a v0.1.1 -m "Zincite 0.1.1"
git push origin v0.1.1
gh release create v0.1.1 --title "Zincite 0.1.1" --notes-file /path/to/release-notes.md
```

Do not use `git push --tags`. Report the version and public package/release URLs.

## Publish the guides

Build the committed documentation with the production path:

```sh
cd site
SITE_URL=https://zayenz.se SITE_BASE=/zincite/ npm run build
```

Follow [site/PUBLISHING.md](site/PUBLISHING.md) for the reviewed static import
and Git-based deployment at zayenz.se. Its hosted build runs the existing
release checks; do not repeat them locally solely for the import. A successful
deployment completes the docs release. Further validation needs a concrete
failure, known defect or explicit request.
