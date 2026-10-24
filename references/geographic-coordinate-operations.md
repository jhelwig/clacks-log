# Geographic coordinate operations for issue #5

Research checkpoint: **23 October 2026**. D239 selects the source families;
D257 selects Rust ingestion, `geo` and indexed FlatGeobuf artifacts. D258 selects
`proj4rs` for the identified transformations within their applicability. D259
selects a documented no-shift approximation for the remaining areas. This document
distinguishes those choices from accuracy limitations and does not certify a
shipping dataset. The Rust producer and its independent numerical tests implement
the identified operations; the representative recipe defines their included scope.

The [geographic-pipeline specification](../docs/geographic-pipeline.md) owns the
consolidated production contract. This investigation retains
source declarations, operation parameters, applicability and accuracy evidence;
it does not replace that specification or establish accuracy outside the recorded scope.

## Coordinate convention and inspected sources

Use World Geodetic System 1984 (WGS84) longitude/latitude in decimal degrees as
the common artifact convention. Explicitly identify longitude-first
axis order. Source geometry need not be rounded to native input resolution.
A datum defines the reference frame for positions; a realization is a particular
implementation of that frame, and an epoch identifies the time to which
coordinates refer. A release year does not establish a coordinate epoch.
Changing a datum label does not transform coordinates.

| Input | Direct evidence | Required handling |
| --- | --- | --- |
| Census 2026 states/counties and 2021 Connecticut counties | All three downloaded archives' `.prj` files declare North American Datum of 1983 (NAD83), Greenwich meridian, degree units and the GRS 1980 ellipsoid. Archive hashes match the existing audit. | Select an applicable regional operation from the declared generic NAD83 system; do not infer a newer realization. |
| Natural Earth subunits 5.1.1 and minor islands 4.1.0 | Both pinned `.prj` files declare WGS84 and degree units, consistent with publisher documentation. | No datum shift or projection conversion; retain precision and normalize geometry/date-line representation. |
| Pinned HA8TKS CQ/ITU linework | Embedded GeoJSON (geographic JavaScript Object Notation) feature collections with longitude/latitude arrays, passed into Leaflet's vector-grid GeoJSON path. No separate survey datum/realization or accuracy statement was found. | Interpret using GeoJSON's WGS84 convention. This is format-based interpretation, not verified survey lineage. Do not inverse-project from the map's display projection. |

Census's [2026 technical documentation, sections 3.3.3–3.3.6](https://www2.census.gov/geo/pdfs/maps-data/data/tiger/tgrshp2026/TGRSHP2026_TechDoc.pdf)
and inspected state metadata supply generic NAD83 without a coordinate epoch or
regional realization, and caution that boundary accuracy varies. Check source
declarations during production builds and reject unexpected changes. Source
resolution, numerical precision, transformation accuracy and boundary accuracy
are different quantities.

Natural Earth evidence: [publisher convention](https://www.naturalearthdata.com/features/),
[subunits declaration](https://github.com/nvkelso/natural-earth-vector/blob/v5.1.1/10m_cultural/ne_10m_admin_0_map_subunits.prj),
[minor-islands declaration](https://github.com/nvkelso/natural-earth-vector/blob/v4.1.0/10m_physical/ne_10m_minor_islands.prj).
HA8TKS pins remain in [the source comparison](location-lookup.md).
[GeoJSON section 4](https://www.rfc-editor.org/rfc/rfc7946.html#section-4)
defines its coordinate convention; [Leaflet's conversion API](https://leafletjs.com/reference.html#geojson-coordstolatlng)
documents the input order. Out-of-range longitudes and invalid polygon
interpretations remain source defects; this interpretation does not establish
full format conformance or valid shipping polygons.

## Identified Census regional operations (D258)

The EPSG geodetic parameter registry, originally named for the European Petroleum
Survey Group, defines the following operations. Parameters and areas were checked
in PROJ revision `5fa31c29f0a806568ed7bc76c1a557604db4a804`:
[three-parameter transformations](https://github.com/OSGeo/PROJ/blob/5fa31c29f0a806568ed7bc76c1a557604db4a804/data/sql/helmert_transformation.sql),
[grid transformations](https://github.com/OSGeo/PROJ/blob/5fa31c29f0a806568ed7bc76c1a557604db4a804/data/sql/grid_transformation.sql),
[areas of use](https://github.com/OSGeo/PROJ/blob/5fa31c29f0a806568ed7bc76c1a557604db4a804/data/sql/extent.sql).
Registry accuracy describes the operation, not a guaranteed maximum error or the
accuracy of Census boundaries.

| Applicability | Operation | Parameters/data | Registry accuracy |
| --- | --- | --- | --- |
| Contiguous states and Alaska mainland, including Connecticut's older counties | EPSG:1188, NAD83 to WGS 84 (1) | Geocentric translations X=0, Y=0, Z=0 metres; extent 1325. | 4 metres |
| Aleutian Islands | EPSG:1251, NAD83 to WGS 84 (2) | X=−2, Y=0, Z=+4 metres; extent 2157, crossing the date line. | 8 metres |
| Hawaii main islands | EPSG:1252, NAD83 to WGS 84 (3) | X=+1, Y=+1, Z=−1 metres; extent 3883. | 4 metres |
| Puerto Rico and U.S. Virgin Islands, onshore | EPSG:1731, NAD83 to WGS 84 (35) | Horizontal correction grid from NAD83 to NAD83 High Accuracy Reference Network (HARN), with that target approximated as WGS84; extent 3634. | 2 metres |
| Guam, Northern Mariana Islands and American Samoa | Explicit no-shift approximation (D259) | Preserve supplied angular coordinates; retain generic NAD83 source declaration and approximation provenance. | Not established |

The first three are geographic two-dimensional operations implemented through
Earth-centred coordinate translations. The metre offsets are not degrees added
to latitude/longitude. Follow the geographic two-dimensional operation's height
convention without inventing recorded station elevation. EPSG:1188's zero shift
is an explicit regional approximation, not permission to equate all NAD83 and
WGS84 coordinates worldwide.

Respect published applicability, not simply state codes or the first overlapping
bounding rectangle. Alaska has mainland and island components; Hawaii includes
geography outside the main-islands operation. Onshore operations do not validate
offshore administrative polygons. These candidates are not a complete regional
partition already authored. Check component extents and metadata before claiming
coverage; explicitly assign D259's approximation to remaining components rather
than silently extending these operations. Transform shared borders consistently within each operation. Across different operations, preserve the exact operation and report gaps or overlaps rather than adjusting boundaries (D269). Choosing
different local grids for adjacent mainland states can introduce avoidable seams.

For EPSG:1731, the published [`us_noaa_pvhpgn.tif`](https://cdn.proj.org/us_noaa_pvhpgn.tif)
contains latitude/longitude offsets in arc-seconds, positive east, and identifies
source/target systems 4269/4152. It is the grid step, not proof that HARN is exactly
WGS84. Its metadata credits NOAA and states public domain. Inspected SHA-256:
`a6db010f956526a011ba719c45fe32e5aff987eeb1cb03f2b8338208a77c619d`.
Retain attribution and original metadata; pin the grid instead of fetching a
replacement implicitly. Consume published data without running upstream
conversion tooling. PROJ-data revision inspected for related source notices:
`cdab44864a36a9f3c3e90a36637c5d8a86e624c6`.

The National Geodetic Survey's [2011 adjustment explanation](https://www.ngs.noaa.gov/web/surveys/NA2011/NA2011_FAQ.shtml)
documents separate Pacific and Mariana realizations. It does not establish which
one Census used. Do not assign NAD83(PA11), NAD83(MA11), an old island datum or
an epoch solely from geography. Further source metadata or Census clarification
would be needed to establish a stronger transformation claim for these components.
D259 permits proceeding with the documented no-shift approximation instead.
Preserve required U.S. coverage and disclose the accuracy limitation.

## Selected Rust implementation (D258)

[`proj4rs` 0.2.1](https://docs.rs/proj4rs/latest/proj4rs/) implements explicit
three-parameter shifts and grid shifts. Published source includes GeoTIFF grid
reading behind `tiff`, including sample order, angular units and shift direction.
Its README mentions only NTv2 grids, while the crate-level documentation and
source support GeoTIFF too. Native NADCON `.las/.los` files are not recognized;
the published GeoTIFF representation avoids a new Clacks grid decoder.

Use `proj4rs` for producer-side execution of the identified operations,
subject to numerical and real-grid validation. Supply explicit ellipsoid/shift
or grid parameters. Generic `+datum=NAD83` selects a zero-shift approximation,
not the Aleutian, Hawaiian or Caribbean operation. Its angular interface uses
radians. Constrain grid lookup to pinned data; reject missing/out-of-area grids
and failed/non-finite results. Do not add optional-grid or zero-shift fallbacks.
Grid support is documented upstream as experimental. The pinned grid is covered by independent Rust TIFF node/bilinear arithmetic and explicit out-of-grid/checksum regressions; this does not establish compatibility with other grid conventions or dynamic datum operations.

There is no demonstrated need for native PROJ for these identified operations.
Its broader database cannot supply missing Census realization/epoch evidence.
A dependency change needs a concrete unsupported operation or validation failure.
Artifact readers need normalized geometry, indexes and `geo` predicates rather
than datum transformation code once the producer establishes a common convention.

## Pacific, outlying-island and offshore findings

### Historical datum statements do not identify the current input

The [2007 Census technical documentation, datum section](https://www2.census.gov/geo/pdfs/maps-data/data/tiger/tgrshp2007/TGRSHP07.pdf)
explicitly excludes much of Hawaii and the Pacific Island Areas from its NAD83
transformation claim. It identifies local astronomical datums, rather than a
single regional NAD83 realization. In contrast, the
[2010 documentation, section 3.4](https://www2.census.gov/geo/pdfs/maps-data/data/tiger/tgrshp2010/TGRSHP10.pdf)
and [2011 documentation, section 3.3](https://www2.census.gov/geo/pdfs/maps-data/data/tiger/tgrshp2011/TGRSHP2011_TechDoc.pdf)
declare all generated shapefiles NAD83. The downloaded 2010 state archive's
metadata explicitly includes Hawaii and the Pacific Island Areas in that
declaration. That 2010 archive has 52 features; its broad metadata wording is not
an inspection of Pacific Island Area geometry. The inspected 2026 state and county metadata also declare NAD83;
their lineage does not identify regional realizations, epochs or island-specific
conversion parameters.

Therefore the old exception list is evidence about historical inputs, not a
recipe to apply to the selected 2026 files. The documentation change establishes
a changed declaration, not independently verified conversion dates or parameters.
Do not apply Old Hawaiian, Guam 1963 or American Samoa 1962 corrections to modern
NAD83-labelled files based on old metadata or island names. That could introduce
a second, erroneous shift.

Published operations exist for those legacy inputs, but are conditional evidence:

| If the source actually declares… | Available registry operation | Why it does not close the current gap |
| --- | --- | --- |
| Guam 1963 | EPSG:1070, three-parameter shift (6-metre stated accuracy), or EPSG:1069, correction grid (5 metres). | Published area is Guam onshore. Neither identifies current Census coordinates nor justifies application across the Northern Marianas. |
| American Samoa 1962 | EPSG:1577, three-parameter shift (44 metres), or EPSG:1694/1695, western/eastern correction grids (5 metres). | Requires that source datum and the particular covered islands; it does not establish the datum of Rose or Swains Island. |
| Old Hawaiian | EPSG:15840, correction grid (2 metres), plus island-specific three-parameter alternatives. | The main-island operations do not establish a conversion for northwestern islands with historical local astronomical datums. |

These records are in the same pinned PROJ transformation tables cited above.
They are not additional selected operations. Actual legacy inputs would require
their own source declaration, area checks, pinned grids and validation.

Modern Pacific/Mariana realizations are also not interchangeable with generic
NAD83. The registry's operations EPSG:7808/7809 relate International Terrestrial
Reference Frame 2008 (ITRF2008) to NAD83(PA11)/NAD83(MA11) using time-dependent
parameters. Choosing those paths would require evidence of the source frame and
coordinate epoch, and would exceed `proj4rs`'s static three-/seven-parameter
interface. There is no justification for introducing that machinery merely to
fill missing source metadata.

### Outlying islands are partly a source-coverage question

The [Census geography glossary](https://www.census.gov/programs-surveys/geography/about/glossary.html)
distinguishes the four Island Areas from the U.S. Minor Outlying Islands: Baker,
Howland, Jarvis, Johnston, Kingman, Midway, Navassa, Palmyra and Wake. It notes
that the latter are usually absent from standard products. The selected 56-feature
state layer covers the 50 states, District of Columbia, Puerto Rico and four
Island Areas, not a separate Minor Outlying Islands state-equivalent feature.
Missing geometry cannot be supplied by transforming the Census coordinates.

Where the selected Natural Earth layers actually supply an island, its declared
WGS84 coordinates need no Census datum conversion; DXCC correspondence and
coverage validation remain necessary. Do not assign a missing island to Hawaii
or another nearby subdivision. Northwestern islands within Hawaii are a separate
case: EPSG:1252 does not cover the entire chain; D259 assigns the documented
no-shift approximation to the remaining Census components.

### Offshore is not one unresolved transformation

EPSG:1188's area explicitly includes onshore **and offshore** mainland North
America. EPSG:1252 includes onshore **and offshore** main Hawaiian islands.
Those portions are not a general offshore gap. EPSG:1251 (Aleutians) and
EPSG:1731 (Puerto Rico/U.S. Virgin Islands) have onshore applicability; no such
offshore accuracy claim follows from those records. A grid having numerical
values offshore does not extend its published accuracy or intended use.

The [National Geodetic Survey's NADCON guidance](https://www.ngs.noaa.gov/PC_PROD/NADCON/Readme.htm)
warns that offshore grid results can be extrapolations with unestablished
accuracy. This warning is not a quantified error estimate for every Clacks
operation. Preserve existing Census administrative geometry; do not replace it
with shipping limits, mineral-management boundaries or newly invented buffers.
Those products describe different things. Likewise, a coordinate transform must
not turn offshore administrative water into land for DXCC classification.

### Selected no-shift approximation (D259)

For remaining Pacific-territory, outlying-island and offshore Census components
without an established applicable D258 operation, preserve supplied angular
coordinates numerically in the common artifact convention. Retain the original
datum declaration alongside an explicit no-shift operation and unestablished
accuracy. This permits proceeding without source-producer clarification; no
producer was contacted during the investigation. Do not infer a legacy or modern
island realization from geography.

Modern-frame differences plausibly have metre-scale effects, but that is not a
measured error bound for these sources. Unknown legacy/local datum errors could
be much larger, and boundary-source error remains independent of datum error.
Near a boundary, a displacement can change point classification or a grid area's
coverage/candidates, including an incorrectly confident result. Large grids are
not immune: their edges can lie close to boundaries. Small supported grids can
be comparable in size to metre-scale differences. Keep this limitation explicit;
do not claim a four-metre guarantee or survey accuracy.

Keep normal point/whole-grid lookup, manual corrections and recorded positions
unchanged. No uncertainty buffer or extra runtime ambiguity rule is selected.
Missing source geometry remains missing. Assign the approximation deliberately
to identified components; it is not a fallback for failed transformations,
missing required grids, malformed geometry or changed source declarations.
Validation must cover seams between transformed and approximated components.

## Acceptance evidence still needed in #5

- Record source declarations, selected operations, applicability, parameters/grid
  hashes, accuracy statements and source limits. Identify components assigned
  the no-shift approximation and preserve its accuracy limitation in the manifest.
- Check shifts against independently calculated geocentric examples and the
  Caribbean grid against independently evaluated node/interpolation examples,
  using reproducible Rust tooling. Round trips alone cannot detect a consistently
  wrong parameter, sign, axis order or grid choice.
- Cover Connecticut's two source vintages, mainland/Aleutian components, Hawaii,
  Caribbean grid edges, missing grids, date-line equivalents and non-finite input.
  Shared borders must remain compatible across state/county layers.
- Validate transformed segments and any necessary densification: nonlinear
  operations must not introduce unmeasured straight-segment approximation error.
  Distinguish numerical error from the operation's published accuracy.
- Exercise the emitted/read-back artifact with points and complete grid areas,
  including the smallest supported locators, holes, partial coverage and seams.
  File decoding and points far from borders are insufficient validation.

These are engineering/data obligations, not new survey-grade accuracy
requirements or changes to settled point/area semantics. Application implementation
still requires the agreed specification and implementation plan.
