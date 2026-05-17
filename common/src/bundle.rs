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

#[cfg(test)]
mod tests {
    use super::*;

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
}
