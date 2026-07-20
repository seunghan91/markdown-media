//! HEIC/HEIF image support module.
//!
//! Registers the libheif decoder as an `image` crate plugin hook,
//! so `image::ImageReader::open("photo.heic")` works without
//! any special-casing in caller code.
//!
//! Feature-gated behind `heic` (see `core/Cargo.toml`).
//!
//! Requires `libheif` system library (brew install libheif on macOS,
//! apt install libheif-dev on Linux).

#[cfg(feature = "heic")]
pub fn register() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        libheif_rs::integration::image::register_all_decoding_hooks();
    });
}

#[cfg(not(feature = "heic"))]
pub fn register() {}

pub fn available() -> bool {
    cfg!(feature = "heic")
}
