# Signature-definition data

`signatures-11.34.json` preserves the ordered station/contact fields, required
flags, representation hints, name/version and deprecation status from the pinned
configuration's `sigspecs` section. Provenance is embedded in the data.

`cargo run --bin clacks-lotw-data -- --check` reads XML as reference data
and compares extraction without changing files. Neither this extraction nor
Rust loading compiles, links or executes TrustedQSL. Updates must retain ordered
fields and compatible model provenance; unsupported selected versions fail.
