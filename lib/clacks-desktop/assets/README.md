# Application artwork

The user selected image-generation option 2 on November 4, 2026: pale blue CL
letters on the existing desktop's dark workspace color. The original is
`docs/superpowers/specs/assets/2026-11-04-clacks-log-icon.png`. No SVG source or
additional iconography was selected. These renditions preserve its appearance.

Generated with the built-in image generation tool. Prompt: a single standalone
rounded-square app icon with ordinary bold sans-serif uppercase "CL", separate
letters centered with generous margins, flat background `#1D2228` and letters
`#ADD6E7`; transparent outside the tile; no gradients, shading, shadows, texture,
decoration, radio imagery, books, towers, pictograms, symbolism or other text.
The original remains authoritative; generated color/shading are not asserted to
be pixel-exact palette values. This project-created artwork accompanies the
project under its root LICENSE; no external artwork or font file is bundled.

Renditions were generated with macOS ImageIO (`sips`), without redrawing:

```sh
sips -z 256 256 docs/superpowers/specs/assets/2026-11-04-clacks-log-icon.png \
  --out lib/clacks-desktop/assets/clacks-log.png
rustc --edition 2024 lib/clacks-desktop/assets/convert-icon.rs -o /tmp/clacks-convert-icon
/tmp/clacks-convert-icon \
  docs/superpowers/specs/assets/2026-11-04-clacks-log-icon.png \
  lib/clacks-desktop/assets/clacks-log.ico
```

Inspect small-size legibility and native alpha edges on Windows before claiming
visual acceptance. Conversion tools are not part of the release build.

Original SHA-256:
`ffeed442e16325147955e8186428c3221e7b50ab73d2e54d083506974ef3797f`.
The ICO contains 16, 24, 32, 48, 64, 128 and 256 pixel PNG frames.
