use std::{fmt, fs};

use zed_extension_api as zed;

pub const SERVER_BINARY: &str = "atfile-lsp";
const EXTENSION_VERSION: &str = env!("CARGO_PKG_VERSION");
const REPOSITORY: &str = "inmzhang/atfile";

pub fn installed_server_binary() -> zed::Result<String> {
    let platform = Platform::current()?;
    let binary_path = platform.binary_path();

    if !fs::metadata(&binary_path).is_ok_and(|metadata| metadata.is_file()) {
        download_server_binary(platform)?;
    }

    Ok(binary_path)
}

fn download_server_binary(platform: Platform) -> zed::Result<()> {
    let tag = release_tag();
    let release = zed::github_release_by_tag_name(REPOSITORY, &tag)?;
    let asset_name = platform.asset_name();
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .ok_or_else(|| {
            format!(
                "release {} does not contain asset {asset_name}",
                release_tag()
            )
        })?;

    zed::download_file(
        &asset.download_url,
        &platform.archive_path(),
        platform.archive_kind().into(),
    )?;

    if !platform.is_windows() {
        zed::make_file_executable(&platform.binary_path())?;
    }

    Ok(())
}

fn release_tag() -> String {
    format!("v{EXTENSION_VERSION}")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Platform {
    os: SupportedOs,
    arch: SupportedArch,
}

impl Platform {
    fn current() -> zed::Result<Self> {
        let (os, arch) = zed::current_platform();
        Self::new(os.into(), arch.into())
    }

    fn new(os: HostOs, arch: HostArch) -> zed::Result<Self> {
        let os = SupportedOs::try_from(os)?;
        let arch = SupportedArch::try_from(arch)?;

        if os == SupportedOs::Windows && arch != SupportedArch::X86_64 {
            return Err(format!("unsupported platform: {os}-{arch}"));
        }

        Ok(Self { os, arch })
    }

    fn asset_name(self) -> String {
        format!(
            "{SERVER_BINARY}-{}-{}.{}",
            release_tag(),
            self,
            self.archive_kind()
        )
    }

    fn archive_path(self) -> String {
        format!("{}/{}", release_dir(), self)
    }

    fn binary_path(self) -> String {
        format!("{}/{}", self.archive_path(), binary_file_name(self.os))
    }

    fn archive_kind(self) -> ArchiveKind {
        if self.is_windows() {
            ArchiveKind::Zip
        } else {
            ArchiveKind::GzipTar
        }
    }

    fn is_windows(self) -> bool {
        self.os == SupportedOs::Windows
    }
}

fn release_dir() -> String {
    format!("{SERVER_BINARY}-{}", release_tag())
}

fn binary_file_name(os: SupportedOs) -> &'static str {
    if os == SupportedOs::Windows {
        "atfile-lsp.exe"
    } else {
        SERVER_BINARY
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SupportedOs {
    Linux,
    Macos,
    Windows,
}

impl fmt::Display for SupportedOs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Linux => f.write_str("linux"),
            Self::Macos => f.write_str("macos"),
            Self::Windows => f.write_str("windows"),
        }
    }
}

impl TryFrom<HostOs> for SupportedOs {
    type Error = String;

    fn try_from(os: HostOs) -> Result<Self, Self::Error> {
        match os {
            HostOs::Linux => Ok(Self::Linux),
            HostOs::Macos => Ok(Self::Macos),
            HostOs::Windows => Ok(Self::Windows),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SupportedArch {
    X86_64,
    Aarch64,
}

impl fmt::Display for SupportedArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86_64 => f.write_str("x86_64"),
            Self::Aarch64 => f.write_str("aarch64"),
        }
    }
}

impl TryFrom<HostArch> for SupportedArch {
    type Error = String;

    fn try_from(arch: HostArch) -> Result<Self, Self::Error> {
        match arch {
            HostArch::X86_64 => Ok(Self::X86_64),
            HostArch::Aarch64 => Ok(Self::Aarch64),
            HostArch::Other(arch) => Err(format!("unsupported architecture: {arch}")),
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.os, self.arch)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArchiveKind {
    GzipTar,
    Zip,
}

impl From<ArchiveKind> for zed::DownloadedFileType {
    fn from(kind: ArchiveKind) -> Self {
        match kind {
            ArchiveKind::GzipTar => Self::GzipTar,
            ArchiveKind::Zip => Self::Zip,
        }
    }
}

impl fmt::Display for ArchiveKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GzipTar => f.write_str("tar.gz"),
            Self::Zip => f.write_str("zip"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HostOs {
    Linux,
    Macos,
    Windows,
}

impl From<zed::Os> for HostOs {
    fn from(os: zed::Os) -> Self {
        match os {
            zed::Os::Linux => Self::Linux,
            zed::Os::Mac => Self::Macos,
            zed::Os::Windows => Self::Windows,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HostArch {
    X86_64,
    Aarch64,
    Other(String),
}

impl From<zed::Architecture> for HostArch {
    fn from(arch: zed::Architecture) -> Self {
        match arch {
            zed::Architecture::X8664 => Self::X86_64,
            zed::Architecture::Aarch64 => Self::Aarch64,
            zed::Architecture::X86 => Self::Other("x86".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_x86_64_uses_tarball_asset() {
        let platform = Platform::new(HostOs::Linux, HostArch::X86_64).unwrap();

        assert_eq!(
            platform.asset_name(),
            "atfile-lsp-v0.1.0-linux-x86_64.tar.gz"
        );
        assert_eq!(
            platform.binary_path(),
            "atfile-lsp-v0.1.0/linux-x86_64/atfile-lsp"
        );
    }

    #[test]
    fn macos_aarch64_uses_tarball_asset() {
        let platform = Platform::new(HostOs::Macos, HostArch::Aarch64).unwrap();

        assert_eq!(
            platform.asset_name(),
            "atfile-lsp-v0.1.0-macos-aarch64.tar.gz"
        );
        assert_eq!(platform.archive_path(), "atfile-lsp-v0.1.0/macos-aarch64");
        assert_eq!(
            platform.binary_path(),
            "atfile-lsp-v0.1.0/macos-aarch64/atfile-lsp"
        );
    }

    #[test]
    fn windows_x86_64_uses_zip_asset_and_exe() {
        let platform = Platform::new(HostOs::Windows, HostArch::X86_64).unwrap();

        assert_eq!(
            platform.asset_name(),
            "atfile-lsp-v0.1.0-windows-x86_64.zip"
        );
        assert_eq!(
            platform.binary_path(),
            "atfile-lsp-v0.1.0/windows-x86_64/atfile-lsp.exe"
        );
    }

    #[test]
    fn windows_aarch64_is_not_supported() {
        let error = Platform::new(HostOs::Windows, HostArch::Aarch64).unwrap_err();

        assert_eq!(error, "unsupported platform: windows-aarch64");
    }

    #[test]
    fn unsupported_arch_error_names_arch() {
        let error =
            Platform::new(HostOs::Linux, HostArch::Other("riscv64".to_string())).unwrap_err();

        assert_eq!(error, "unsupported architecture: riscv64");
    }
}
