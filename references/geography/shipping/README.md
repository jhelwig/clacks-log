# Shipping geographic inputs

[`../shipping.json`](../shipping.json) is the resolved shipping recipe. It
selects all 56 Census 2026 state/territory features, 3,226 county equivalents
outside Connecticut, and the eight actual Connecticut counties from 2021.
Connecticut's nine 2026 planning regions are excluded. Selected source identities
are distinct from the number of polygons after longitude normalization.

The recipe retains all 40 CQ zones and all 90 International Telecommunication
Union (ITU) zones from the pinned HA8TKS originals. Six guarded ITU replacement
files preserve every original coordinate except these reviewed source
overshoots:

- Zones 2, 4, 14 and 15: remove the final near-start overshoot, then close directly
  to the unchanged original first vertex. Original final vertices are respectively
  `[-141.00403,79.99907]`, `[-90.00001,48.05995]`, `[-58.3374,-16.5]` and
  `[-58.3438,-16.51773]`.
- Zones 31 and 42: remove their identical spike `[80.60215,42.89507]`. Both original
  paths run from `[80.54704,42.88449]` through that spike, double back through
  `[80.59896,42.89399]`, then cross the preceding edge before
  `[80.59714,42.89744]`. Both records receive the same correction.

These are specific changes to the pinned original sequences, not a generic
polygon repair. Original archive and semantic-feature digests and replacement
file digests guard each correction. The separately declared source closures
remain recorded by the producer. Source linework has unestablished survey
lineage and accuracy; topological validity does not establish survey accuracy.

## Administrative and radio correspondence

Census supplies administrative names and identities. County recorded values use
exact case-insensitive correspondence with the pinned native Logbook of The
World county labels, preferring the full county/type label over the shorter
name. This preserves distinct independent cities and recordable aliases such
as `Saint Marys`. A current Census county equivalent absent from that older
catalog retains its current Census name. No fuzzy name or nearest-feature rule
is used. Parent entity and state context accompany every county value.

The ARRL DX Century Club (DXCC) [entity list](https://www.arrl.org/country-lists-prefixes)
and [special documentation list](https://lotw.arrl.org/lotw-help/specialdocumentationrequirements/)
establish the separately classified Kure, Swains and Desecheo islands. Their
exact original Census components are selected by interior anchors, transformed
through the prescribed regional operations, and subtracted from the parent
classifications. Matching state/county scopes follow the separate entity.
Swains already has its own county-equivalent identity, `60040`.

[`dxcc-correspondence.tsv`](dxcc-correspondence.tsv) explicitly maps supported
Natural Earth subunit keys to radio entities. Every selected subunit is either
mapped or identified in a reviewed omission. Detailed Census U.S. geography
replaces the corresponding Natural Earth subunits. Mixed-entity geography
without a reviewed split remains unclassified, including Greece, Canada,
Brazil, Australia, Chile, Colombia, Fiji, Kiribati, Malaysia, Mauritius,
New Caledonia, Cook Islands, French Polynesia, Russia and Taiwan. This is
best-effort worldwide assistance, not complete worldwide DXCC coverage or an
award-eligibility determination.

All 2,795 selected Natural Earth minor-island features lack names or
administrative keys: their only attributes are `featurecla`, `min_zoom` and
`scalerank`. Their original semantic digests identify them in the omission
inventory. They receive no inferred correspondence from proximity. Supported
named small-island subunits retain their own explicit mappings.

## Coordinate partition

The existing prescribed operations retain their published applicability and
accuracy limits. Complete contiguous mainland state/DC footprints and the
Alaska mainland component define the mainland assignment. The Aleutian mask
uses selected Aleutians West and East land faces within the published extent.
The eight main Hawaiian island components retain their existing assignment.

[`caribbean-pins.json`](caribbean-pins.json) records original 2026 Census FACES
archives for all 78 Puerto Rico municipalities and all three U.S. Virgin Islands
county equivalents. Only original `LWFLAG=L` land faces enter the onshore grid
mask, intersected with the published operation extent. Inland/offshore waters
and other residual areas retain the explicitly disclosed no-shift approximation;
a failed transform never chooses that approximation. Exact operation seams
are retained, not snapped away. Administrative waters are not coastlines.

## Maintenance and verification

The supported `clacks-geography-build author-shipping` command reads verified
cached originals and the reviewed correspondence, pin and case files. It writes
`geography/shipping.json` and guarded correction files beneath the selected
references tree. It starts from `geography/representative.json`, expands the
shipping selections. Review the resulting changes before building data; build
identity comes from the packaging pipeline rather than authored recipe fields.

From the repository root:

```sh
cargo +1.99.1 run -p clacks-geography-build --release --locked --offline -- author-shipping \
  --references references --cache "$CLACKS_GEOGRAPHY_CACHE"
```

Absolute `--references` and `--cache`
paths allow invoking the built executable from another directory. This command does not fetch sources, compile
geographic layers or create a release. Acquire reviewed checksum-pinned originals
with the existing `fetch` command first. Source-pin changes are reviewed edits to
`caribbean-pins.json` and `aleutian-pins.json`; authoring verifies that their
identities match the required original counties.

The original-source acceptance target validates identity inventories and
independently authored locations in [`cases.json`](cases.json), together with
the retained representative cases. It distinguishes county-border point and
whole-cell requests, actual Connecticut counties, mainland radio fields,
Caribbean/Pacific geography, separate island entities and offshore coverage.
To reuse an already built immutable bundle without recompiling unchanged data,
set `CLACKS_GEOGRAPHY_BUNDLE` to that exact bundle directory:

```sh
CLACKS_GEOGRAPHY_CACHE="$CLACKS_GEOGRAPHY_CACHE" \
CLACKS_GEOGRAPHY_BUNDLE="$CLACKS_GEOGRAPHY_BUNDLE" \
cargo +1.99.1 test -p clacks-geography-builder --test real_sources --release --locked --offline \
  shipping_inventory_and_required_us_fields -- --ignored --exact
```

Absent or invalid inputs fail acceptance. Full-source acceptance is explicit
release preparation, not automatic application CI. Use the existing
[build procedure](../BUILDING.md) for cached production and verification. Actual
hashes, measured resources and release observations belong in delivery evidence.

[`dxcc-labels.tsv`](dxcc-labels.tsv) supplies shared factual entity names where
several subunits or states have the same recorded entity, and for a supported
subunit whose own name differs from its entity (Tasmania/Australia and
Hainan/China). CQ and ITU value labels are canonical zone labels. The authoring
step rejects inconsistent labels for any scoped value before writing the recipe.

[`omissions.json`](omissions.json) records specific mixed-entity findings.
Natural Earth's Pitcairn feature includes separately classified Ducie Island;
its New Zealand subantarctic feature includes the Snares, which are excluded
from entity 16. The [ARRL clarification](https://www.arrl.org/news/arrl-dxcc-department-announces-zl9-name-change)
includes Antipodes and Bounty in entity 16 alongside Auckland and Campbell.
Both complete Natural Earth features remain unclassified until a reviewed
component correspondence is supplied.

The same original-component inspection finds Cocos Island inside the Costa Rica
feature and African Spanish islands (including Chafarinas) inside the Spain
feature. Cocos is a separate radio entity; the
[official Islands On The Air directory](https://www.iota-world.org/islands-on-the-air/iota-groups-islands/35.html)
places Chafarinas in Ceuta and Melilla. These full features also remain
unclassified. A separate country/subunit name is insufficient evidence that
all of its remote components share one radio entity.

## Named subunit partitions

The pinned source already separates these supported radio correspondences; the
listed ordinary and remote subunit footprints have no positive-area interior
overlap. The table describes source coverage, not award eligibility.

| Ordinary subunit | Separately represented subunits |
| --- | --- |
| Cuba `CUB` → 70 | Guantanamo Bay `USG` → 105 |
| Mainland Portugal `PRX` → 272 | Azores `PAZ` → 149; Madeira `PMD` → 256 |
| Mainland France `FXX` → 227 | Corsica `FXC` → 214 |
| Mainland Norway `NOW` → 266 | Jan Mayen `NJM` → 118; Svalbard `NSV` → 259 |
| Mainland India `INX` → 324 | Andamans `INA` and Nicobars `INN` → 11; Lakshadweep `INL` → 142 |
| Mainland Ecuador `ECD` → 120 | Galápagos `ECG` → 71 |
| Mainland South Africa `ZAX` → 462 | Prince Edward/Marion `ZAI` → 201 |
| Tasmania `AUA` → Australia 150 | Macquarie `AUM` → 153 |
| New Zealand `NZN`/`NZS` → 170 | `NZA` is separately represented but omitted for its mixed Snares coverage |


## Explicit full-longitude latitude bands

Two additional guarded replacements interpret the original ITU 74 and 75
rectangles as full-longitude bands. Zone 74 spans 80–85° south; zone 75 spans
80–85° north. The original horizontal edges connect +180° and −180° directly.
The replacements retain all original vertices and insert collinear vertices at
+60° and −60° along those two edges. This preserves each original Cartesian
footprint and latitude extent while making longitude continuity explicit.
[GeoJSON's line definition](https://www.rfc-editor.org/rfc/rfc7946.html#section-3.1.1)
provides the coordinate-plane interpretation; these files do not reinterpret a
band as a geodesic or infer a polar cap. The original, semantic-feature and
replacement digests guard the interpretation as they do the six ring repairs.

The ITU field remains uncovered beyond 85° north/south. The independently
authored [polar cases](polar-cases.json) check band interiors, the closed 85°
boundaries, uncovered 87° points, and partial whole-field coverage beside both
sides of the date line. Their expectations follow the exact original extents;
they are not generated from production query results.

## Egypt source-ring representation

[`egypt.json`](egypt.json) is a ninth guarded replacement, for the pinned Natural
Earth `EGY` feature. Its original clockwise exterior repeats the exact node
`35.62108710600006,23.139292914000066` around a six-vertex counterclockwise loop.
That contained loop touches the shell at one point. The replacement represents
it as a third explicit hole, keeping the existing two holes and seven other
polygon members unchanged.

The authoring step requires the reviewed original semantic digest, proves that
all 1,924 directed boundary edges are bitwise identical, verifies exactly equal
signed and unsigned area, and validates the complete replacement topology. No
coordinate location or boundary edge changes. The independently authored Cairo
case checks the recorded Egypt value and its catalog-supported radio zones.
This source-specific ring interpretation uses the same guarded replacement
facility as the ITU inputs.

## Native county-catalog attribution

County recorded-value aliases use the committed
[`lotw-11.34.json`](../../../lib/clacks-model/data/lotw-11.34.json), configuration
release 11.34, extracted from TrustedQSL 2.8.6 at upstream commit
`6ce824363e27da455c6cf6fd440b9c401945bfcb`. The
[pinned upstream source](https://sourceforge.net/p/trustedqsl/tqsl/ci/6ce824363e27da455c6cf6fd440b9c401945bfcb/tree/)
and the [native data provenance](../../../lib/clacks-model/data/README.md)
own that catalog's provenance and license terms.

The standalone geographic bundle includes the literal upstream license as
`notices/trustedqsl-catalog.txt`; the app's separate TrustedQSL data notice does
not substitute for this standalone distribution notice. No TrustedQSL code is
executed during authoring or compilation.
