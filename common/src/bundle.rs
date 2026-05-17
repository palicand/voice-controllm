//! Locate sibling binaries and resources relative to the current executable,
//! handling both macOS .app bundle layout (`Contents/MacOS/<exe>`, resources under
//! `Contents/Resources/`) and flat dev/release/cargo-install layouts.

use std::path::{Path, PathBuf};

/// Layout of a `.app` bundle's resource tree, expressed as path segments under
/// `Contents/Resources/`.
pub struct BundleLayout<'a> {
    /// Path segments under `Contents/Resources/` when running from inside a bundle.
    pub bundled: &'a [&'a str],
    /// Path segments relative to the exe's directory in non-bundle layouts.
    pub fallback: &'a [&'a str],
}

/// Resolve a sibling resource path for `current_exe`, picking `layout.bundled`
/// when the exe sits under `Contents/MacOS/`, otherwise `layout.fallback`.
pub fn resolve(current_exe: &Path, layout: BundleLayout<'_>) -> PathBuf {
    let parent = current_exe.parent().unwrap_or_else(|| Path::new(""));

    if parent.ends_with("Contents/MacOS")
        && let Some(contents) = parent.parent()
    {
        let mut path = contents.join("Resources");
        for seg in layout.bundled {
            path.push(seg);
        }
        return path;
    }

    let mut path = parent.to_path_buf();
    for seg in layout.fallback {
        path.push(seg);
    }
    path
}

/// `vcmd` sibling — bundled at `Contents/Resources/vcmd`, otherwise next to the exe.
pub const VCMD: BundleLayout<'static> = BundleLayout {
    bundled: &["vcmd"],
    fallback: &["vcmd"],
};

/// `vcmctl` sibling — bundled at `Contents/Resources/bin/vcmctl`, otherwise next to the exe.
pub const VCMCTL: BundleLayout<'static> = BundleLayout {
    bundled: &["bin", "vcmctl"],
    fallback: &["vcmctl"],
};

/// The state of the `vcmctl` installation at `~/.local/bin/vcmctl`.
#[derive(Debug, PartialEq, Eq)]
pub enum VcmctlInstallState {
    /// A symlink exists and resolves to the vcmctl shipped alongside `current_exe`.
    Installed,
    /// No file exists at the install path (or the bundle target is missing).
    NotInstalled,
    /// A file exists at the install path but it is **not** a symlink. The user
    /// may have placed their own vcmctl there deliberately.
    ConflictingFile,
}

/// Returns the install state of `vcmctl` at `~/.local/bin/vcmctl` relative to
/// the vcmctl shipped alongside `current_exe`.
pub fn vcmctl_install_state(current_exe: &Path) -> VcmctlInstallState {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return VcmctlInstallState::NotInstalled;
    };
    vcmctl_install_state_with_home(current_exe, &home)
}

/// Returns true iff `~/.local/bin/vcmctl` is a symlink that resolves to the
/// vcmctl shipped alongside `current_exe`.
pub fn is_vcmctl_installed(current_exe: &Path) -> bool {
    vcmctl_install_state(current_exe) == VcmctlInstallState::Installed
}

fn vcmctl_install_state_with_home(current_exe: &Path, home: &Path) -> VcmctlInstallState {
    let bundle_target = resolve(current_exe, VCMCTL);
    let Ok(canon_bundle) = std::fs::canonicalize(&bundle_target) else {
        return VcmctlInstallState::NotInstalled;
    };
    let install_path = home.join(".local").join("bin").join("vcmctl");
    // Check for a regular file (not a symlink) before attempting canonicalize,
    // which would silently follow symlinks and lose this distinction.
    if install_path.exists() && !install_path.is_symlink() {
        return VcmctlInstallState::ConflictingFile;
    }
    let Ok(canon_install) = std::fs::canonicalize(&install_path) else {
        return VcmctlInstallState::NotInstalled;
    };
    if canon_install == canon_bundle {
        VcmctlInstallState::Installed
    } else {
        VcmctlInstallState::NotInstalled
    }
}

fn is_vcmctl_installed_with_home(current_exe: &Path, home: &Path) -> bool {
    vcmctl_install_state_with_home(current_exe, home) == VcmctlInstallState::Installed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn dev_vcmd_sibling() {
        assert_eq!(
            resolve(Path::new("/Users/x/proj/target/debug/vcm"), VCMD),
            PathBuf::from("/Users/x/proj/target/debug/vcmd")
        );
    }

    #[test]
    fn bundled_vcmd_in_resources() {
        assert_eq!(
            resolve(Path::new("/Applications/VCM.app/Contents/MacOS/vcm"), VCMD),
            PathBuf::from("/Applications/VCM.app/Contents/Resources/vcmd")
        );
    }

    #[test]
    fn cargo_install_vcmd_sibling() {
        assert_eq!(
            resolve(Path::new("/Users/x/.cargo/bin/vcm"), VCMD),
            PathBuf::from("/Users/x/.cargo/bin/vcmd")
        );
    }

    #[test]
    fn bundled_vcmctl_under_bin() {
        assert_eq!(
            resolve(
                Path::new("/Applications/VCM.app/Contents/MacOS/vcm"),
                VCMCTL
            ),
            PathBuf::from("/Applications/VCM.app/Contents/Resources/bin/vcmctl")
        );
    }

    #[test]
    fn dev_vcmctl_sibling() {
        assert_eq!(
            resolve(Path::new("/Users/x/proj/target/debug/vcm"), VCMCTL),
            PathBuf::from("/Users/x/proj/target/debug/vcmctl")
        );
    }

    /// Set up a fake bundle (`<tmp>/bundle/vcm` + sibling `vcmctl`) and a fake
    /// home directory (`<tmp>/home`). Returns `(exe_path, home_path)`.
    fn fake_install_layout(tmp: &Path) -> (PathBuf, PathBuf) {
        let bundle = tmp.join("bundle");
        fs::create_dir_all(&bundle).unwrap();
        let exe = bundle.join("vcm");
        fs::write(&exe, b"").unwrap();
        fs::write(bundle.join("vcmctl"), b"vcmctl-bin").unwrap();
        let home = tmp.join("home");
        fs::create_dir_all(home.join(".local").join("bin")).unwrap();
        (exe, home)
    }

    #[test]
    fn installed_when_symlink_points_to_bundle_vcmctl() {
        let tmp = tempdir().unwrap();
        let (exe, home) = fake_install_layout(tmp.path());
        symlink(
            tmp.path().join("bundle").join("vcmctl"),
            home.join(".local").join("bin").join("vcmctl"),
        )
        .unwrap();

        assert!(is_vcmctl_installed_with_home(&exe, &home));
    }

    #[test]
    fn not_installed_when_symlink_points_elsewhere() {
        let tmp = tempdir().unwrap();
        let (exe, home) = fake_install_layout(tmp.path());
        let other = tmp.path().join("other");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("vcmctl"), b"different").unwrap();
        symlink(
            other.join("vcmctl"),
            home.join(".local").join("bin").join("vcmctl"),
        )
        .unwrap();

        assert!(!is_vcmctl_installed_with_home(&exe, &home));
    }

    #[test]
    fn not_installed_when_no_symlink_exists() {
        let tmp = tempdir().unwrap();
        let (exe, home) = fake_install_layout(tmp.path());

        assert!(!is_vcmctl_installed_with_home(&exe, &home));
    }

    #[test]
    fn not_installed_when_install_path_is_regular_file() {
        let tmp = tempdir().unwrap();
        let (exe, home) = fake_install_layout(tmp.path());
        fs::write(
            home.join(".local").join("bin").join("vcmctl"),
            b"vcmctl-bin",
        )
        .unwrap();

        assert!(!is_vcmctl_installed_with_home(&exe, &home));
    }

    #[test]
    fn conflicting_file_state_when_regular_file_at_install_path() {
        let tmp = tempdir().unwrap();
        let (exe, home) = fake_install_layout(tmp.path());
        fs::write(
            home.join(".local").join("bin").join("vcmctl"),
            b"vcmctl-bin",
        )
        .unwrap();

        assert_eq!(
            vcmctl_install_state_with_home(&exe, &home),
            VcmctlInstallState::ConflictingFile
        );
    }

    #[test]
    fn not_installed_state_when_no_file_at_install_path() {
        let tmp = tempdir().unwrap();
        let (exe, home) = fake_install_layout(tmp.path());

        assert_eq!(
            vcmctl_install_state_with_home(&exe, &home),
            VcmctlInstallState::NotInstalled
        );
    }

    #[test]
    fn installed_state_when_symlink_points_to_bundle() {
        let tmp = tempdir().unwrap();
        let (exe, home) = fake_install_layout(tmp.path());
        symlink(
            tmp.path().join("bundle").join("vcmctl"),
            home.join(".local").join("bin").join("vcmctl"),
        )
        .unwrap();

        assert_eq!(
            vcmctl_install_state_with_home(&exe, &home),
            VcmctlInstallState::Installed
        );
    }

    #[test]
    fn not_installed_when_symlink_is_broken() {
        let tmp = tempdir().unwrap();
        let (exe, home) = fake_install_layout(tmp.path());
        symlink(
            tmp.path().join("does-not-exist"),
            home.join(".local").join("bin").join("vcmctl"),
        )
        .unwrap();

        assert!(!is_vcmctl_installed_with_home(&exe, &home));
    }

    #[test]
    fn not_installed_when_bundle_vcmctl_missing() {
        let tmp = tempdir().unwrap();
        let (exe, home) = fake_install_layout(tmp.path());
        fs::remove_file(tmp.path().join("bundle").join("vcmctl")).unwrap();
        symlink(
            tmp.path().join("bundle").join("vcmctl"),
            home.join(".local").join("bin").join("vcmctl"),
        )
        .unwrap();

        assert!(!is_vcmctl_installed_with_home(&exe, &home));
    }
}
