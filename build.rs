fn main() {
    #[cfg(target_os = "macos")]
    {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        let plist = std::path::Path::new(&manifest_dir)
            .join("assets/macos/vcmd-Info.plist")
            .canonicalize()
            .expect("vcmd-Info.plist not found");
        println!("cargo:rerun-if-changed={}", plist.display());
        println!(
            "cargo:rustc-link-arg-bin=vcmd=-Wl,-sectcreate,__TEXT,__info_plist,{}",
            plist.display()
        );
    }
}
