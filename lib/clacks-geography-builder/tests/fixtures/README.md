# Original coordinate-grid input

`us_noaa_pvhpgn.tif` is the unmodified published NOAA/PROJ-data grid from https://cdn.proj.org/us_noaa_pvhpgn.tif. Its original metadata states public domain and credits NOAA. The representative recipe records the same source URL, SHA-256 and attribution. It is original acquired data, not a generated fixture or output from executing native geographic utilities.

Keeping this small original input permits offline numerical regression against independently evaluated TIFF nodes/interpolation. Production builds still acquire and verify the pinned grid through the normal source path. Tests do not substitute execution of PROJ or another native geographic package for their expected-value arithmetic.

`cq-19-original.json` is the selected original CQ-19 linework extracted by the locked Rust decoder from the recipe's pinned HA8TKS source, with original vertex order retained. It tests normalization at the date line. `HA8TKS-LICENSE.txt` carries the source notice; the source URL, revision and archive digest are in the representative recipe. The real-source test reproduces these bytes independently of the normalizer. `CLACKS_REFRESH_GEOGRAPHY_FIXTURES=1` on that explicit test refreshes only fixture/reviewed geometry files, leaving recipe policies and mappings for review.
