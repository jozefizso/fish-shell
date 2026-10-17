//! Host-portable equivalent of `build_tools/git_version_gen.sh` for Cargo builds.
//! Release scripts continue using that script; keep its prefix/fallback policy in sync.

use std::{io, path::Path, process::Command};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VersionSource {
    Git,
    /// Git is missing, or the source archive has no usable Git metadata.
    Package,
}

#[derive(Debug, Eq, PartialEq)]
pub struct BuildVersion {
    pub value: String,
    pub source: VersionSource,
}

/// Apply the release script's version policy without depending on host tools.
/// A successful but empty Git response is an error, not archive fallback.
pub fn select_version(
    package_version: &str,
    git_version: Option<&str>,
) -> io::Result<BuildVersion> {
    let Some(git_version) = git_version else {
        return Ok(BuildVersion {
            value: package_version.to_owned(),
            source: VersionSource::Package,
        });
    };
    let git_version = git_version.trim_ascii_end();
    if git_version.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Git returned an empty version",
        ));
    }
    Ok(BuildVersion {
        value: if git_version.starts_with(package_version) {
            git_version.to_owned()
        } else {
            format!("{package_version}-g{git_version}")
        },
        source: VersionSource::Git,
    })
}

/// Query Git directly, never through a shell. Without Git or repository metadata,
/// the manifest's package version remains authoritative (including release archives).
pub fn acquire_version(source_dir: &Path, package_version: &str) -> io::Result<BuildVersion> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(source_dir)
        .args(["describe", "--always", "--dirty"]);
    // Do not accidentally describe a repository containing an unpacked source archive.
    if let Some(parent) = source_dir.parent() {
        command.env("GIT_CEILING_DIRECTORIES", parent);
    }
    let output = match command.output() {
        Ok(output) if output.status.success() => output,
        _ => return select_version(package_version, None),
    };
    let git_version = std::str::from_utf8(&output.stdout)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    select_version(package_version, Some(git_version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn w1_version_matching_git() {
        assert_eq!(
            select_version("4.9.3", Some("4.9.3-12-gabcdef-dirty\n")).unwrap(),
            BuildVersion {
                value: "4.9.3-12-gabcdef-dirty".into(),
                source: VersionSource::Git,
            }
        );
    }

    #[test]
    fn w1_version_revision_prefix() {
        assert_eq!(
            select_version("4.9.3", Some("abcdef\n")).unwrap(),
            BuildVersion {
                value: "4.9.3-gabcdef".into(),
                source: VersionSource::Git,
            }
        );
    }

    #[test]
    fn w1_version_archive_fallback() {
        assert_eq!(
            select_version("4.9.3", None).unwrap(),
            BuildVersion {
                value: "4.9.3".into(),
                source: VersionSource::Package,
            }
        );
        assert_eq!(
            select_version("4.9.3", Some("\n")).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
