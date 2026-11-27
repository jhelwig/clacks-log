//! Pure desktop release identity and installer names shared by browser and packaging.

/// Prefix identifying desktop release tags.
const TAG_PREFIX: &str = "desktop-v";

/// A canonical desktop calendar version that can identify a public release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseVersion(String);

impl ReleaseVersion {
    /// Parse YY.M.N, with an unpadded month and installer-compatible build number.
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        let mut parts = value.split('.');
        let valid = match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(year), Some(month), Some(number), None) => {
                let decimal = |part: &str| {
                    !part.is_empty()
                        && part.bytes().all(|byte| byte.is_ascii_digit())
                        && (part.len() == 1 || !part.starts_with('0'))
                };
                year.len() == 2
                    && year.bytes().all(|byte| byte.is_ascii_digit())
                    && decimal(month)
                    && matches!(month.parse::<u8>(), Ok(1..=12))
                    && decimal(number)
                    && number.parse::<u16>().is_ok()
            }
            _ => false,
        };
        anyhow::ensure!(
            valid,
            "desktop release version {value} must be YY.M.N with unpadded month 1–12 and build number 0–65535"
        );
        Ok(Self(value.to_owned()))
    }

    /// Canonical numeric version used by the application and installers.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Source tag for this desktop release.
    pub fn tag(&self) -> String {
        format!("{TAG_PREFIX}{}", self.0)
    }

    /// Parse a desktop source tag through the same calendar-version rules.
    pub fn from_tag(tag: &str) -> anyhow::Result<Self> {
        Self::parse(
            tag.strip_prefix(TAG_PREFIX)
                .ok_or_else(|| anyhow::anyhow!("expected a desktop-vYY.M.N tag"))?,
        )
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
