# Zincite artwork

The logo is an angular crystal Z in orange (`#f86708`) and rust (`#c84b08`).
SVG files are the editable originals. Mark, wordmark and favicon PNGs have
transparent backgrounds.

| Asset | Use |
| --- | --- |
| `zincite-mark.svg`, `zincite-mark.png` | Standalone mark; PNG is 512×512 |
| `zincite-logo-dark.svg`, `zincite-logo-dark.png` | Dark lettering for light backgrounds |
| `zincite-logo-light.svg`, `zincite-logo-light.png` | Light lettering for dark backgrounds |
| `favicon.svg` | Scalable favicon |
| `favicon.ico` | Favicon containing 16, 32 and 48px images |
| `favicon-16.png`, `favicon-32.png` | Small favicon exports |
| `apple-touch-icon.png` | 180×180 mark |
| `github-social-preview.svg`, `github-social-preview.png` | 1280×640 repository social preview |

The wordmarks use Arial with Helvetica and sans-serif fallbacks. Their PNG
exports are 1120×360. Render SVG exports with `rsvg-convert`, for example:

```sh
rsvg-convert -w 512 -h 512 assets/zincite-mark.svg -o assets/zincite-mark.png
rsvg-convert assets/github-social-preview.svg -o assets/github-social-preview.png
```
