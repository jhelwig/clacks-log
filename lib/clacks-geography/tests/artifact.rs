mod support;

use clacks_geography::artifact::{validate::open_bundle, write::write_bundle};
use clacks_geography::error::GeographyError;
use geo_types::polygon;
use std::{error::Error, fs};

#[test]
fn deterministic_bundle_round_trip_and_empty_layers() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let ma = write_bundle(a.path(), &support::contents()).unwrap();
    let mb = write_bundle(b.path(), &support::contents()).unwrap();
    assert_eq!(ma, mb);
    let opened = open_bundle(a.path()).unwrap();
    assert_eq!(opened.manifest(), &ma);
    assert_eq!(ma.layers.len(), 5);
    for entry in &ma.files {
        assert_eq!(
            fs::read(a.path().join(&entry.path)).unwrap(),
            fs::read(b.path().join(&entry.path)).unwrap()
        );
    }
    assert!(write_bundle(a.path(), &support::contents()).is_err());
}

#[test]
fn selection_checks_expected_manifest_before_decode_and_binds_payload_bytes() {
    use sha2_rsa::{Digest, Sha256};
    let directory = tempfile::tempdir().unwrap();
    let manifest = write_bundle(directory.path(), &support::contents()).unwrap();
    let path = directory.path().join("manifest.json");
    let bytes = fs::read(&path).unwrap();
    let expected = hex::encode(Sha256::digest(&bytes));
    let load = || clacks_geography::artifact::read::open_bundle(directory.path(), &expected);
    assert_eq!(load().unwrap().manifest(), &manifest);

    fs::write(&path, b"not a manifest").unwrap();
    let error = load().unwrap_err();
    assert!(matches!(error, GeographyError::InvalidArtifact { .. }));
    assert!(
        error
            .to_string()
            .contains("selected manifest digest mismatch")
    );

    let mut replaced = manifest.clone();
    replaced.release_id = "another-self-consistent-release".into();
    fs::write(&path, serde_json::to_vec(&replaced).unwrap()).unwrap();
    assert!(
        load()
            .unwrap_err()
            .to_string()
            .contains("selected manifest digest mismatch")
    );

    fs::write(&path, &bytes).unwrap();
    let payload_path = directory.path().join("layers/primary_subdivision.fgb");
    let original_payload = fs::read(&payload_path).unwrap();
    let mut damaged_payload = original_payload.clone();
    damaged_payload[0] ^= 0xff;
    fs::write(&payload_path, &damaged_payload).unwrap();
    assert!(
        load()
            .unwrap_err()
            .to_string()
            .contains("payload size or digest mismatch")
    );

    let mut bound_payload = manifest.clone();
    bound_payload
        .files
        .iter_mut()
        .find(|entry| entry.path == "layers/primary_subdivision.fgb")
        .unwrap()
        .sha256 = hex::encode(Sha256::digest(&damaged_payload));
    let bound_bytes = serde_json::to_vec(&bound_payload).unwrap();
    fs::write(&path, &bound_bytes).unwrap();
    assert!(
        clacks_geography::artifact::read::open_bundle(
            directory.path(),
            &hex::encode(Sha256::digest(&bound_bytes))
        )
        .is_ok(),
        "runtime loading decoded the digest-valid malformed layer"
    );

    fs::write(&payload_path, original_payload).unwrap();
    let mut invalid_metadata = manifest;
    invalid_metadata.release_id = "..".into();
    let invalid_bytes = serde_json::to_vec(&invalid_metadata).unwrap();
    fs::write(&path, &invalid_bytes).unwrap();
    assert!(
        clacks_geography::artifact::read::open_bundle(
            directory.path(),
            &hex::encode(Sha256::digest(&invalid_bytes))
        )
        .is_ok(),
        "runtime loading repeated producer metadata validation"
    );
}

#[test]
fn rejects_inconsistent_manifest_even_when_payload_hashes_match() {
    for change in [
        "schema",
        "role",
        "values",
        "derivation",
        "source",
        "count",
        "extent",
        "traversal",
        "absolute",
        "notice",
        "release",
    ] {
        let dir = tempfile::tempdir().unwrap();
        write_bundle(dir.path(), &support::contents()).unwrap();
        let path = dir.path().join("manifest.json");
        let mut json: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        match change {
            "schema" => json["schema_version"] = 2.into(),
            "role" => {
                let layers = json["layers"].as_array_mut().unwrap();
                layers.pop();
            }
            "values" => {
                let v = json["values"][0].clone();
                json["values"].as_array_mut().unwrap().push(v);
            }
            "derivation" => {
                json["derivations"][0]["operation_ids"] = serde_json::json!(["missing"])
            }
            "source" => json["sources"] = serde_json::json!([]),
            "count" => json["layers"][0]["feature_count"] = 99.into(),
            "extent" => json["layers"][0]["extent_or_empty"] = serde_json::json!([0, 0, 100, 100]),
            "traversal" => json["files"][0]["path"] = "../outside".into(),
            "absolute" => json["files"][0]["path"] = "/etc/passwd".into(),
            "notice" => {
                json["sources"][0]["notice_paths"] = serde_json::json!(["notices/missing.txt"])
            }
            "release" => json["release_id"] = "..".into(),
            _ => unreachable!(),
        }
        fs::write(path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert!(open_bundle(dir.path()).is_err(), "accepted {change}");
    }
}

#[test]
fn rejects_invalid_writer_inputs() {
    for change in [
        "duplicate_part",
        "reference",
        "nan",
        "infinite",
        "self_intersection",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut input = support::contents();
        let layer = input
            .layers
            .iter_mut()
            .find(|l| !l.features.is_empty())
            .unwrap();
        match change {
            "duplicate_part" => layer.features.push(layer.features[0].clone()),
            "reference" => layer.features[0].value_key = "missing".into(),
            "nan" => layer.features[0].geometry.0[0].exterior_mut(|r| r.0[0].x = f64::NAN),
            "infinite" => {
                layer.features[0].geometry.0[0].exterior_mut(|r| r.0[0].x = f64::INFINITY)
            }
            "self_intersection" => {
                layer.features[0].geometry = geo_types::MultiPolygon(vec![
                    geo_types::polygon![(x:0.,y:0.),(x:2.,y:1.),(x:0.,y:1.),(x:2.,y:0.),(x:0.,y:0.)],
                ])
            }
            _ => unreachable!(),
        }
        assert!(
            write_bundle(dir.path(), &input).is_err(),
            "accepted {change}"
        );
    }
}

#[test]
fn rejects_missing_and_damaged_payloads_and_preserves_io_source() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = write_bundle(dir.path(), &support::contents()).unwrap();
    let entry = &manifest.files[0];
    fs::write(dir.path().join(&entry.path), b"corrupt").unwrap();
    assert!(matches!(
        open_bundle(dir.path()),
        Err(GeographyError::InvalidArtifact { .. })
    ));
    fs::remove_file(dir.path().join(&entry.path)).unwrap();
    let err = open_bundle(dir.path()).err().unwrap();
    assert!(err.source().is_some());
    assert!(
        open_bundle(&dir.path().join("missing"))
            .err()
            .unwrap()
            .source()
            .is_some()
    );
}

#[cfg(unix)]
#[test]
fn rejects_symlink_payloads() {
    let dir = tempfile::tempdir().unwrap();
    write_bundle(dir.path(), &support::contents()).unwrap();
    let path = dir.path().join("notices/census.txt");
    let copy = dir.path().join("copy.txt");
    fs::rename(&path, &copy).unwrap();
    std::os::unix::fs::symlink(copy, path).unwrap();
    assert!(open_bundle(dir.path()).is_err());
}

#[test]
fn checksum_valid_index_and_property_corruption_is_rejected_without_panicking() {
    use sha2_rsa::{Digest, Sha256};
    for mutation in ["index", "properties", "truncated"] {
        let dir = tempfile::tempdir().unwrap();
        write_bundle(dir.path(), &support::contents()).unwrap();
        let path = dir.path().join("layers/primary_subdivision.fgb");
        let mut bytes = fs::read(&path).unwrap();
        match mutation {
            "index" => {
                let header_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
                let start = 12 + header_len;
                bytes[start..start + 8].copy_from_slice(&100f64.to_le_bytes());
            }
            "properties" => {
                let position = bytes.windows(4).position(|w| w == b"wa-1").unwrap();
                bytes[position - 4..position].copy_from_slice(&u32::MAX.to_le_bytes());
            }
            "truncated" => {
                bytes.truncate(bytes.len() - 5);
            }
            _ => unreachable!(),
        }
        fs::write(path, &bytes).unwrap();
        let manifest_path = dir.path().join("manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        let entry = manifest["files"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|v| v["path"] == "layers/primary_subdivision.fgb")
            .unwrap();
        entry["bytes"] = (bytes.len() as u64).into();
        entry["sha256"] = hex::encode(Sha256::digest(&bytes)).into();
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let result = std::panic::catch_unwind(|| open_bundle(dir.path()));
        assert!(result.is_ok(), "decoder panicked for {mutation}");
        assert!(result.unwrap().is_err(), "accepted {mutation}");
    }
}

#[test]
fn rejects_unsupported_layer_headers_with_valid_payload_digests() {
    use flatgeobuf::{ColumnType, FgbCrs, FgbWriter, FgbWriterOptions, GeometryType};
    use geozero::{ColumnValue, FeatureProcessor, GeozeroGeometry, PropertyProcessor};
    use sha2_rsa::{Digest, Sha256};
    for mutation in [
        "dimensions",
        "geometry_type",
        "column",
        "index",
        "coordinate",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let input = support::contents();
        write_bundle(dir.path(), &input).unwrap();
        let row = &input
            .layers
            .iter()
            .find(|l| !l.features.is_empty())
            .unwrap()
            .features[0];
        let path = dir.path().join("layers/primary_subdivision.fgb");
        let mut writer = FgbWriter::create_with_options(
            "invalid",
            if mutation == "geometry_type" {
                GeometryType::Unknown
            } else {
                GeometryType::MultiPolygon
            },
            FgbWriterOptions {
                write_index: mutation != "index",
                has_z: mutation == "dimensions",
                detect_type: false,
                promote_to_multi: false,
                crs: FgbCrs {
                    org: Some("OGC"),
                    code_string: Some(if mutation == "coordinate" {
                        "WRONG"
                    } else {
                        "CRS84"
                    }),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        for name in [
            if mutation == "column" {
                "wrong"
            } else {
                "part_id"
            },
            "value_key",
            "derivation_key",
        ] {
            writer.add_column(name, ColumnType::String, |_, _| {});
        }
        writer.feature_begin(0).unwrap();
        writer.properties_begin().unwrap();
        for (i, (name, value)) in [
            ("part_id", &row.part_id),
            ("value_key", &row.value_key),
            ("derivation_key", &row.derivation_key),
        ]
        .into_iter()
        .enumerate()
        {
            writer
                .property(i, name, &ColumnValue::String(value))
                .unwrap();
        }
        writer.properties_end().unwrap();
        writer.geometry_begin().unwrap();
        geo_types::Geometry::MultiPolygon(row.geometry.clone())
            .process_geom(&mut writer)
            .unwrap();
        writer.geometry_end().unwrap();
        writer.feature_end(0).unwrap();
        let mut bytes = Vec::new();
        writer.write(&mut bytes).unwrap();
        fs::write(path, &bytes).unwrap();
        let manifest_path = dir.path().join("manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        let entry = manifest["files"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|v| v["path"] == "layers/primary_subdivision.fgb")
            .unwrap();
        entry["bytes"] = (bytes.len() as u64).into();
        entry["sha256"] = hex::encode(Sha256::digest(&bytes)).into();
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(
            open_bundle(dir.path()).is_err(),
            "accepted unsupported {mutation}"
        );
    }
}

#[test]
fn indexed_topology_validation_checks_crossings_holes_and_parts() {
    use clacks_geography::geometry::validate_topology;
    use geo_types::{MultiPolygon, Polygon, line_string};
    let square = |w, s, e, n| line_string![(x:w,y:s),(x:e,y:s),(x:e,y:n),(x:w,y:n),(x:w,y:s)];
    let valid = MultiPolygon(vec![Polygon::new(
        square(0., 0., 10., 10.),
        vec![square(2., 2., 4., 4.)],
    )]);
    assert!(validate_topology(&valid).is_ok());
    for invalid in [
        MultiPolygon(vec![Polygon::new(
            square(0., 0., 1., 1.),
            vec![square(2., 2., 3., 3.)],
        )]),
        MultiPolygon(vec![Polygon::new(
            square(0., 0., 10., 10.),
            vec![square(2., 2., 5., 5.), square(3., 3., 6., 6.)],
        )]),
        MultiPolygon(vec![
            Polygon::new(square(0., 0., 2., 2.), vec![]),
            Polygon::new(square(1., 1., 3., 3.), vec![]),
        ]),
        MultiPolygon(vec![Polygon::new(
            line_string![(x:0.,y:0.),(x:2.,y:2.),(x:0.,y:2.),(x:2.,y:0.),(x:0.,y:0.)],
            vec![],
        )]),
    ] {
        assert!(validate_topology(&invalid).is_err());
    }
}

#[test]
fn hole_contacts_must_not_disconnect_the_polygon_interior() {
    use clacks_geography::geometry::validate_topology;
    use geo_types::{MultiPolygon, Polygon, line_string};
    let shell = line_string![(x:0.,y:0.),(x:10.,y:0.),(x:10.,y:10.),(x:0.,y:10.),(x:0.,y:0.)];
    let single_contact = line_string![(x:0.,y:5.),(x:3.,y:3.),(x:3.,y:7.),(x:0.,y:5.)];
    assert!(
        validate_topology(&MultiPolygon(vec![Polygon::new(
            shell.clone(),
            vec![single_contact]
        )]))
        .is_ok()
    );
    let disconnected = line_string![(x:0.,y:5.),(x:5.,y:2.),(x:10.,y:5.),(x:5.,y:8.),(x:0.,y:5.)];
    assert!(
        validate_topology(&MultiPolygon(vec![Polygon::new(shell, vec![disconnected])])).is_err()
    );
}

#[test]
fn unsupported_schema_is_identified_before_decoding_its_payload() {
    use sha2_rsa::{Digest, Sha256};
    let directory = tempfile::tempdir().unwrap();
    let bytes = br#"{"schema_version":2,"different_future_contract":true}"#;
    fs::write(directory.path().join("manifest.json"), bytes).unwrap();
    assert!(matches!(
        clacks_geography::artifact::read::open_bundle(
            directory.path(),
            &hex::encode(Sha256::digest(bytes))
        ),
        Err(GeographyError::UnsupportedSchema(2))
    ));
}

#[test]
fn feature_frames_are_bounded_before_dependency_buffer_allocation() {
    use sha2_rsa::{Digest, Sha256};
    let directory = tempfile::tempdir().unwrap();
    let mut manifest = write_bundle(directory.path(), &support::contents()).unwrap();
    let path = "layers/primary_subdivision.fgb";
    let mut bytes = fs::read(directory.path().join(path)).unwrap();
    let header = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let feature = 12 + header + flatgeobuf::packed_r_tree::PackedRTree::index_size(1, 16);
    bytes[feature..feature + 4].copy_from_slice(&4_194_304u32.to_le_bytes());
    fs::write(directory.path().join(path), &bytes).unwrap();
    manifest
        .files
        .iter_mut()
        .find(|entry| entry.path == path)
        .unwrap()
        .sha256 = hex::encode(Sha256::digest(&bytes));
    fs::write(
        directory.path().join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let error = open_bundle(directory.path()).err().unwrap();
    assert!(
        error.to_string().contains("feature exceeds file length"),
        "{error:#}"
    );
}

#[test]
fn typed_zone_values_cannot_have_competing_textual_aliases() {
    use clacks_geography::artifact::{Field, ParentScope};
    for field in [Field::CqZone, Field::ItuZone] {
        let mut input = support::contents();
        let features = std::mem::take(
            &mut input
                .layers
                .iter_mut()
                .find(|layer| layer.field == Field::PrimarySubdivision)
                .unwrap()
                .features,
        );
        input
            .layers
            .iter_mut()
            .find(|layer| layer.field == field)
            .unwrap()
            .features = features;
        input.manifest.values[0].field = field;
        input.manifest.values[0].parent_scope = ParentScope::Global;
        input.manifest.values[0].recordable_value = "03".into();
        let directory = tempfile::tempdir().unwrap();
        assert!(write_bundle(directory.path(), &input).is_err());
    }
}
