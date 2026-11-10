# Representative geographic recipe

`representative.json` is the maintained build input. It pins every original archive/grid, its coordinate declaration, complete archive inventory, selected identities/counts, source notices, explicit operations and classified values. It is a representative release, not complete U.S. or worldwide application coverage.

Census 2026 states/counties provide selected complete administrative geometry. Connecticut uses the eight actual 2021 counties; 2026 planning regions are excluded. An original-source union check independently establishes that Washington/Oregon administrative land **and waters** cover the whole CN85 cell. CN75 extends offshore and has only partial Oregon coverage. Source `INTPTLAT`/`INTPTLON` attributes provide independent county-interior cases. Administrative water boundaries are not coastlines.

Census land faces support onshore datum assignments in the selected Aleutian, San Juan and St. Thomas areas. The remaining Caribbean onshore masks are outside this recipe's scope, so primary suggestions there use only the selected complete county footprints. Full Puerto Rico/Virgin Islands state geometry is not classified using an unsupported transformation. Pacific/outlying/offshore components use D259's disclosed no-shift approximation. The [coordinate investigation](../geographic-coordinate-operations.md) owns the parameter/source rationale. Exact-operation seams are retained and reported under D269.

Natural Earth subunit keys are explicitly mapped to DX Century Club (DXCC) values. The selected IDs also exist in the pinned native LoTW catalog; names/identities are checked against the [ARRL entity list](https://www.arrl.org/country-lists-prefixes). The mapping is Clacks-authored factual correspondence, not a claim that ordinary national boundaries always equal radio entities. A selected minor island remains explicitly unmapped. Wider correspondence and coverage are issue #23 work.

The HA8TKS collections supply all 40 CQ zones and eleven selected International Telecommunication Union (ITU) zones. Other ITU geography, including known defective records, is explicitly omitted. `itu-2.json` retains all original ITU-2 vertices except the final near-start overshoot and closes directly to the original first vertex. Original archive/semantic feature and replacement-file hashes guard this complete replacement. No generic repair algorithm is used. The original source notices remain bundled.

`athos-review-area.json` is an authored uncertainty rectangle at longitude 23.9–24.5°, latitude 40.1–40.6°. It deliberately removes more than Mount Athos from Greece's classification. It is **not** an entity boundary and assigns no replacement entity. The [ARRL statement](https://www.arrl.org/news/arrl-dxcc-statement-on-mount-athos-operation-sv1ga-a) establishes separate-entity treatment; the [UNESCO location/map page](https://whc.unesco.org/en/list/454/maps/) supplies geographic context. No map artwork or traced boundary is included.

Data source notices and redistribution evidence are literal recipe inputs, independent of Clacks's Elastic License 2.0. An upstream URL/version alone is insufficient: acquired bytes must match the pinned digest. To update inputs, use the Rust CLI's `fetch` and `inspect`, review changed selections/guards/mappings, and run the real-source acceptance suite. Geometry fixtures are reproducible with the locked Rust test support; no external geographic executable or Python environment is required.

## Shipping recipe

[`shipping.json`](shipping.json) broadens the source selection to all 56 Census
state/territory features and 3,234 county equivalents, preserving the eight actual
Connecticut counties. Its [maintained input notes](shipping/README.md) describe
full regional operation masks, separate U.S. island entities, worldwide
correspondence/omissions, guarded ITU replacements and original-source acceptance.
The representative recipe remains independently usable with its original scope.
