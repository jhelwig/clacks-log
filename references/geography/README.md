# Geographic build inputs

`representative.json` pins the original source bytes, exact source declarations, selected identities/counts and source notices for issue #5's representative production-path validation. It is not complete U.S. or worldwide coverage. Full application integration and coverage expansion belong to issue #23.

Maintain these inputs with the production Rust `clacks-geography-build fetch` and `inspect` commands documented in the builder crate. Inspection supplies source-defined IDs and canonical original feature guards for reviewed replacements. Do not generate opaque expectations with external geographic/Python tooling or promote a successful build's answers into its expected results.

The original Census archives declare generic NAD83 (North American Datum of 1983). Natural Earth declares WGS84 (World Geodetic System 1984), with separately pinned literal coordinate declarations for subunits and minor islands. HA8TKS data uses GeoJSON longitude/latitude conventions without a separate survey lineage. Source release dates do not identify coordinate epochs or accuracy.

Census 2026 topological-face metadata, section `LWFLAG`, defines five values: G Glacier, I Intermittent Water, L Land, P Perennial Water and S Marsh/Swamp. The recipe selects L faces and rejects unknown domain codes. These are additional Census-family support inputs for source-coordinate operation applicability, not a replacement county dataset. The selected 2026 L counts are 534 (02016), 5,963 (72127), 1,352 (78030). Their original geometry and source-defined TFID keys are retained; no shoreline is inferred from a registry bounding rectangle.

Minor islands have no source-defined unique key in the selected input: `featurecla`, `min_zoom` and `scalerank` are not unique. The representative feature is guarded by its canonical original geometry/attribute digest and the whole archive's pinned digest. It has no supported DXCC correspondence and remains an explicit reviewed omission. No nearest-feature or invented island ownership rule is used.

Source rights and source precision are different questions. Census attribution, HA8TKS's full MIT notice, Natural Earth's public-domain acknowledgment and the NOAA grid credit travel with the bundle. The source comparison and coordinate investigation retain the factual provenance and accuracy limits; Clacks's application license does not replace upstream data terms.
