use shapefile::{Point, Polygon, PolygonRing, dbase};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    path::Path,
};

pub fn archive(directory: &Path, mutation: &str) -> Vec<u8> {
    let shape = directory.join("source.shp");
    let table =
        dbase::TableWriterBuilder::new().add_character_field("GEOID".try_into().unwrap(), 8);
    let mut writer = shapefile::Writer::from_path(&shape, table).unwrap();
    let mut row = dbase::Record::default();
    row.insert(
        "GEOID".into(),
        dbase::FieldValue::Character(Some("53".into())),
    );
    let points = |w: f64, s: f64, e: f64, n: f64| {
        vec![
            Point::new(w, s),
            Point::new(w, n),
            Point::new(e, n),
            Point::new(e, s),
            Point::new(w, s),
        ]
    };
    if mutation == "shape_kind" {
        writer
            .write_shape_and_record(&shapefile::Polyline::new(points(0., 0., 3., 3.)), &row)
            .unwrap();
    } else {
        let polygon = Polygon::with_rings(vec![
            PolygonRing::Outer(points(0., 0., 3., 3.)),
            PolygonRing::Inner(points(1., 1., 2., 2.)),
            PolygonRing::Outer(points(4., 0., 5., 1.)),
        ]);
        writer.write_shape_and_record(&polygon, &row).unwrap();
    }
    drop(writer);
    let dbf = shape.with_extension("dbf");
    let mut file = fs::OpenOptions::new().write(true).open(&dbf).unwrap();
    file.seek(SeekFrom::Start(1)).unwrap();
    file.write_all(&[126, 1, 1]).unwrap();
    drop(file);
    if mutation == "counts" {
        let mut bytes = fs::read(&dbf).unwrap();
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
        fs::write(&dbf, bytes).unwrap();
    }
    if mutation == "deleted" {
        let mut bytes = fs::read(&dbf).unwrap();
        let start = u16::from_le_bytes(bytes[8..10].try_into().unwrap()) as usize;
        bytes[start] = b'*';
        fs::write(&dbf, bytes).unwrap();
    }
    if mutation == "truncated" {
        let bytes = fs::read(&shape).unwrap();
        fs::write(&shape, &bytes[..110]).unwrap();
    }
    fs::write(
        shape.with_extension("prj"),
        if mutation == "prj" {
            "changed datum"
        } else {
            "test source declaration"
        },
    )
    .unwrap();
    fs::write(
        shape.with_extension("cpg"),
        if mutation == "encoding" {
            "1252"
        } else {
            "UTF-8"
        },
    )
    .unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut bytes);
        for name in [
            "source.shp",
            "source.shx",
            "source.dbf",
            "source.prj",
            "source.cpg",
        ] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(&fs::read(directory.join(name)).unwrap())
                .unwrap();
        }
        zip.finish().unwrap();
    }
    bytes.into_inner()
}
