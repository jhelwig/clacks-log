//! Pure desktop release identity and installer names shared by browser and packaging.

use anyhow::Context;
use std::{fmt, num::NonZeroU16};

/// Prefix identifying desktop release tags.
const TAG_PREFIX: &str = "desktop-v";

/// A canonical desktop calendar version that can identify a public release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleaseVersion {
    /// Two-digit UTC calendar year.
    year: u8,
    /// UTC calendar month, from January through December.
    month: u8,
    /// Release allocation within the month, starting at one.
    number: NonZeroU16,
}

impl ReleaseVersion {
    /// Parse canonical YY.M.N with month 1–12 and sequence 1–65535.
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        let mut parts = value.split('.');
        let (Some(year), Some(month), Some(number), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            anyhow::bail!("desktop release version {value} must be YY.M.N");
        };
        let decimal = |part: &str| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part.len() == 1 || !part.starts_with('0'))
        };
        anyhow::ensure!(
            year.len() == 2
                && year.bytes().all(|byte| byte.is_ascii_digit())
                && decimal(month)
                && decimal(number),
            "desktop release version {value} must use YY.M.N without month/sequence padding"
        );
        Self::new(
            year.parse()?,
            month.parse()?,
            number
                .parse()
                .context("desktop release sequence must be 1–65535")?,
        )
    }

    /// Allocate after the highest used sequence in this UTC month, or start at one.
    /// Reject invalid calendar components or an exhausted sequence.
    pub fn next_for_month(year: u8, month: u8, allocated: &[Self]) -> anyhow::Result<Self> {
        let last = allocated
            .iter()
            .filter(|version| version.year == year && version.month == month)
            .map(|version| version.number)
            .max();
        let number = match last {
            Some(last) => last
                .checked_add(1)
                .context("desktop release month exhausted sequence 65535")?,
            None => NonZeroU16::MIN,
        };
        Self::new(year, month, number)
    }

    /// Construct a version with a valid two-digit year and month.
    fn new(year: u8, month: u8, number: NonZeroU16) -> anyhow::Result<Self> {
        anyhow::ensure!(
            year <= 99 && (1..=12).contains(&month),
            "invalid desktop release year/month"
        );
        Ok(Self {
            year,
            month,
            number,
        })
    }

    /// Source tag for this desktop release.
    pub fn tag(&self) -> String {
        format!("{TAG_PREFIX}{self}")
    }

    /// Parse a desktop source tag through the same calendar-version rules.
    pub fn from_tag(tag: &str) -> anyhow::Result<Self> {
        Self::parse(
            tag.strip_prefix(TAG_PREFIX)
                .ok_or_else(|| anyhow::anyhow!("expected a {TAG_PREFIX}YY.M.N tag"))?,
        )
    }
}

impl fmt::Display for ReleaseVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:02}.{}.{}", self.year, self.month, self.number)
    }
}

impl serde::Serialize for ReleaseVersion {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Native installer targets currently distributed by Clacks.
#[derive(Clone, Copy, Debug, strum::EnumIter)]
pub enum PackagePlatform {
    /// Windows x64 installer.
    WindowsX64,
    /// Apple Silicon disk image.
    MacosArm64,
}

impl PackagePlatform {
    /// Filename using the source package version and selected distribution mode.
    pub fn installer_name(self, version: &str, development: bool) -> String {
        let channel = if development { "-Development" } else { "" };
        let suffix = match self {
            Self::WindowsX64 => "Windows-x64.msi",
            Self::MacosArm64 => "macOS-Apple-Silicon.dmg",
        };
        format!("Clacks-Log-{version}{channel}-{suffix}")
    }
}

impl std::str::FromStr for PackagePlatform {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "windows-x64" => Ok(Self::WindowsX64),
            "macos-arm64" => Ok(Self::MacosArm64),
            _ => anyhow::bail!("expected windows-x64 or macos-arm64"),
        }
    }
}

#[cfg(test)]
mod tests;
