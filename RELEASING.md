# Release Zincite 0.1.0

The installation guides describe the 0.1.0 release destination. Before
publication, the crates.io installation commands, `v0.1.0` source links and
versioned docs.rs pages are pending external checks. A source build or local
archive check does not establish that those public destinations are available.

## Check the source and contents

Use current stable Rust for the supported workspace checks:

```sh
cargo +stable fmt --all -- --check
cargo +stable clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo +stable test --workspace --locked
RUSTDOCFLAGS="-D warnings" cargo +stable doc --workspace --no-deps --locked
cargo +stable package --workspace --list --locked
```

The last command inspects package contents; it does not verify a packaged build.
All five archives need their sources, README and both license texts. Keep site
output, `node_modules`, planning records and exploration assets out. Workspace
integration tests that depend on fixtures outside their crate stay in the
checkout. Query examples ship under `tests/fixtures/` in `zincite-query`.

The source dependency on Zirql is the crates.io release `0.1.0`. Packaged
manifests must replace sibling paths with registry version requirements.
Inspect every normalized `Cargo.toml`, package file list
and lockfile. Build and run the root and standalone commands from an isolated
local installation, including formatting writes, lint warnings, a complete
query and a later-error request with empty stdout.

Before packaging, both consumers must build against the verified Zirql
`0.1.0` registry release. Their accepted relevant suites and the CSV example
must cover the same reviewed shared-language source. Rebuild artifacts after
changes to the shared reader or adapters; retain the existing package, clean-install, CI and site checks.

## Verify archives and publication order

Cargo 1.99 supports native multi-package archive verification and publish dry
runs. Run these commands from a clean candidate checkout:

```sh
cargo +1.99.0 package --workspace --locked
cargo +1.99.0 publish --workspace --dry-run --locked
```

These commands may stage unpublished workspace siblings together. They still
need the actual Zirql 0.1.0 registry dependency. Its absence is a release gate;
do not bypass verification or call a local dependency override registry proof.
Final archives must resolve Zirql from crates.io without source overrides.
Retain the clean candidate commits, archive hashes and command results.

Publish dependencies before their consumers:

1. `zirql` 0.1.0.
2. `zincite-syntax` 0.1.0.
3. `zincite-lint` 0.1.0.
4. `zincite-fmt` and `zincite-query` 0.1.0.
5. `zincite` 0.1.0.

After each dependency is available, verify its real registry resolution before
publishing the next consumer. Registry upload and release-tag pushes require
separate authorization. After publication, install exact versions into a new
prefix with `cargo install --locked --version '=0.1.0'` for the root and each
standalone package. Run their help and the installation-guide examples outside
the source tree. Verify registry checksums, docs.rs pages and `v0.1.0` source
links against the accepted artifacts.

## Check the documentation site

From `site/`, with MiniZinc's standard library configured:

```sh
npm ci
npm run check
SITE_URL=https://zayenz.se SITE_BASE=/zincite/ npm run build
SITE_URL=https://zayenz.se SITE_BASE=/zincite/ npm run preview -- --host 127.0.0.1
```

Check installation and guide pages at desktop and mobile widths, navigation,
search results, keyboard focus and Escape, themes, code/table scrolling,
canonical URLs and assets under `/zincite/`. Match fixture links to the release
checkout. Follow the [site publication procedure](https://github.com/zayenz/zincite/blob/v0.1.0/site/PUBLISHING.md)
only after separate authorization for the personal-site import. A source push
does not publish the site.
