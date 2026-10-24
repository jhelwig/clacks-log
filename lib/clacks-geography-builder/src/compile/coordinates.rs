//! Explicit datum operations and source-coordinate geometry compilation.

use std::io::Cursor;

use anyhow::{Context, Result, ensure};
use geo_types::Coord;
use proj4rs::Proj;
use sha2_rsa::{Digest, Sha256};

#[cfg(test)]
mod tests;

/// Required horizontal conversion with explicit source and target ellipsoids.
pub(super) struct Projection {
    /// Source coordinate reference system including its required datum operation.
    source: Proj,
    /// Target WGS84 longitude/latitude reference system.
    target: Proj,
}

impl Projection {
    /// Construct a three-parameter geocentric translation using zero input height.
    pub(super) fn translation([x, y, z]: [f64; 3]) -> Result<Self> {
        ensure!(
            x.is_finite() && y.is_finite() && z.is_finite(),
            "non-finite translation"
        );
        Self::new(&format!(
            "+proj=longlat +ellps=GRS80 +towgs84={x},{y},{z} +over"
        ))
    }

    /// Load verified mandatory grid bytes into the dependency's digest-keyed catalog.
    pub(super) fn grid(bytes: &[u8], digest: &str) -> Result<Self> {
        ensure!(
            hex::encode(Sha256::digest(bytes)) == digest,
            "grid digest mismatch"
        );
        let key = format!("clacks-geography-{digest}.tif");
        proj4rs::nadgrids::catalog::with_catalog(|catalog| -> Result<()> {
            if catalog.find(&key).is_none() {
                proj4rs::nadgrids::formats::read(catalog, &key, &mut Cursor::new(bytes))
                    .context("decode required coordinate grid")?;
            }
            Ok(())
        })?;
        Self::new(&format!("+proj=longlat +ellps=GRS80 +nadgrids={key} +over"))
    }

    /// Parse explicit coordinate-system definitions without environment discovery.
    fn new(source: &str) -> Result<Self> {
        Ok(Self {
            source: Proj::from_proj_string(source).context("construct source projection")?,
            target: Proj::from_proj_string("+proj=longlat +datum=WGS84 +over")
                .context("construct target projection")?,
        })
    }

    /// Convert a finite longitude/latitude pair in degrees, discarding output height.
    pub(super) fn apply(&self, point: Coord<f64>) -> Result<Coord<f64>> {
        ensure!(
            point.x.is_finite() && point.y.is_finite() && point.y.abs() <= 90.,
            "invalid source coordinate"
        );
        let mut position = (point.x.to_radians(), point.y.to_radians(), 0.);
        proj4rs::transform::transform(&self.source, &self.target, &mut position)
            .context("apply required coordinate operation")?;
        let result = Coord {
            x: position.0.to_degrees(),
            y: position.1.to_degrees(),
        };
        ensure!(
            result.x.is_finite() && result.y.is_finite() && result.y.abs() <= 90.,
            "invalid transformed coordinate"
        );
        Ok(result)
    }
}

impl Projection {
    /// Approximate a transformed source segment with a shared deterministic vertex sequence.
    pub(super) fn segment(
        &self,
        from: Coord<f64>,
        to: Coord<f64>,
        budget: usize,
    ) -> Result<Vec<Coord<f64>>> {
        let reverse = from
            .x
            .total_cmp(&to.x)
            .then(from.y.total_cmp(&to.y))
            .is_gt();
        let (a, b) = if reverse { (to, from) } else { (from, to) };
        let (start, end) = (self.apply(a)?, self.apply(b)?);
        let mut output = vec![start];
        self.subdivide(a, b, start, end, 0, budget, &mut output)?;
        if reverse {
            output.reverse();
        }
        Ok(output)
    }

    /// Enforce quarter/midpoint deviation and fail when computational limits are exhausted.
    #[allow(clippy::too_many_arguments)]
    fn subdivide(
        &self,
        a: Coord<f64>,
        b: Coord<f64>,
        start: Coord<f64>,
        end: Coord<f64>,
        depth: u8,
        budget: usize,
        output: &mut Vec<Coord<f64>>,
    ) -> Result<()> {
        ensure!(output.len() < budget, "transformed vertex budget exhausted");
        let mut acceptable = true;
        for fraction in [0.25, 0.5, 0.75] {
            let point = Coord {
                x: a.x + (b.x - a.x) * fraction,
                y: a.y + (b.y - a.y) * fraction,
            };
            let actual = self.apply(point)?;
            let expected = Coord {
                x: start.x + (end.x - start.x) * fraction,
                y: start.y + (end.y - start.y) * fraction,
            };
            let cosine = if actual.y * expected.y <= 0. {
                1.
            } else {
                actual.y.abs().min(expected.y.abs()).to_radians().cos()
            };
            let error = ((actual.x - expected.x) * cosine).hypot(actual.y - expected.y)
                * MAX_METRES_PER_DEGREE;
            acceptable &= error <= 0.01;
        }
        if acceptable {
            output.push(end);
            return Ok(());
        }
        ensure!(depth < 24, "transformation subdivision depth exhausted");
        let midpoint = Coord {
            x: (a.x + b.x) / 2.,
            y: (a.y + b.y) / 2.,
        };
        let transformed = self.apply(midpoint)?;
        self.subdivide(a, midpoint, start, transformed, depth + 1, budget, output)?;
        self.subdivide(midpoint, b, transformed, end, depth + 1, budget, output)
    }
}

/// Conservative WGS84 metres-per-degree ceiling for the subdivision error bound.
const MAX_METRES_PER_DEGREE: f64 = 111_700.;
