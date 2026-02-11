/// Validate all embedded icons decode correctly. Call once at startup.
pub fn validate() {
    let icons: &[(&str, &[u8])] = &[
        ("listening", include_bytes!("../icons/mic-listening@2x.png")),
        ("paused", include_bytes!("../icons/mic-paused@2x.png")),
        ("init", include_bytes!("../icons/mic-init@2x.png")),
        ("error", include_bytes!("../icons/mic-error@2x.png")),
    ];

    for (name, bytes) in icons {
        image::load_from_memory(bytes)
            .unwrap_or_else(|e| panic!("failed to decode embedded icon '{name}': {e}"));
    }
}

/// Load an icon from embedded PNG bytes.
pub fn load_icon(png_bytes: &[u8]) -> tray_icon::Icon {
    let image = image::load_from_memory(png_bytes)
        .expect("embedded icon is valid PNG")
        .into_rgba8();
    let (width, height) = image.dimensions();
    tray_icon::Icon::from_rgba(image.into_raw(), width, height).expect("valid RGBA icon data")
}
