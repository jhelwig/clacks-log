# Application artwork

The user selected image-generation option 2 on November 4, 2026: pale blue CL
letters on the existing desktop's dark workspace color. The original is
`lib/clacks-desktop/assets/clacks-log-original.png`. No SVG source or
additional iconography was selected. These renditions preserve its appearance.

Generated with the built-in image generation tool. Prompt: a single standalone
rounded-square app icon with ordinary bold sans-serif uppercase "CL", separate
letters centered with generous margins, flat background `#1D2228` and letters
`#ADD6E7`; transparent outside the tile; no gradients, shading, shadows, texture,
decoration, radio imagery, books, towers, pictograms, symbolism or other text.
The original remains authoritative; generated color/shading are not asserted to
be pixel-exact palette values. This project-created artwork belongs to
Technosorcery, LLC and accompanies the project under its root LICENSE; no external artwork or font file is bundled.

Renditions were generated with macOS ImageIO (`sips`), without redrawing:

```sh
sips -z 256 256 lib/clacks-desktop/assets/clacks-log-original.png \
  --out lib/clacks-desktop/assets/clacks-log.png
rustc --edition 2024 lib/clacks-desktop/assets/convert-icon.rs -o /tmp/clacks-convert-icon
/tmp/clacks-convert-icon \
  lib/clacks-desktop/assets/clacks-log-original.png \
  lib/clacks-desktop/assets/clacks-log.ico
```

Inspect small-size legibility and native alpha edges on Windows before claiming
visual acceptance. Conversion tools are not part of the release build.

Original SHA-256:
`ffeed442e16325147955e8186428c3221e7b50ab73d2e54d083506974ef3797f`.
The ICO contains 16, 24, 32, 48, 64, 128 and 256 pixel PNG frames.

The macOS `clacks-log.icns` uses the same original without redrawing. Generate
it on macOS with the native tools below. Conversion is an authoring operation,
not a dependency of release builds. Inspect the installed application's icon
in Finder, the Dock and application switching before claiming native acceptance.

```sh
icon_dir=$(mktemp -d "$TMPDIR/clacks-icon.XXXXXX")
mkdir "$icon_dir/Clacks.iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" \
    lib/clacks-desktop/assets/clacks-log-original.png \
    --out "$icon_dir/Clacks.iconset/icon_${size}x${size}.png"
  double=$((size * 2))
  sips -z "$double" "$double" \
    lib/clacks-desktop/assets/clacks-log-original.png \
    --out "$icon_dir/Clacks.iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$icon_dir/Clacks.iconset" \
  -o lib/clacks-desktop/assets/clacks-log.icns
rm -r "$icon_dir"
```
