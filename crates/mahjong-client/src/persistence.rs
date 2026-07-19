//! Settings persistence.
//!
//! Saves and loads the display-language choice per platform:
//! - WASM: `localStorage` via `js/storage.js` (no wasm-bindgen)
//! - native: a small text file under the user's home directory
//!
//! Language codes must match the JS side: 0 = Japanese, 1 = English
//! (-1 = unset).

use mahjong_core::settings::Lang;

/// Language to integer code (shared by storage format and FFI).
fn lang_to_code(lang: Lang) -> i32 {
    match lang {
        Lang::Ja => 0,
        Lang::En => 1,
    }
}

/// Integer code to language; unknown values yield `None`.
fn code_to_lang(code: i32) -> Option<Lang> {
    match code {
        0 => Some(Lang::Ja),
        1 => Some(Lang::En),
        _ => None,
    }
}

/// Loads the display language selected by the platform backend.
///
/// WASM uses a saved choice first and otherwise detects the browser preference;
/// native returns `None` when the setting is unsaved or invalid.
pub fn load_lang() -> Option<Lang> {
    code_to_lang(load_lang_code())
}

/// Saves the display language; failures are non-fatal and ignored.
pub fn save_lang(lang: Lang) {
    save_lang_code(lang_to_code(lang));
}

#[cfg(target_arch = "wasm32")]
mod backend {
    // Functions storage.js injects into importObject.env via miniquad's
    // plugin mechanism.
    unsafe extern "C" {
        fn mahjong_storage_get_lang() -> i32;
        fn mahjong_storage_set_lang(code: i32);
    }

    /// Version handshake for the storage.js plugin: mq_js_bundle.js's
    /// init_plugins calls `{plugin}_crate_version` and verifies it matches
    /// the JS-side version.
    #[unsafe(no_mangle)]
    pub extern "C" fn mahjong_storage_crate_version() -> u32 {
        1
    }

    pub fn load_lang_code() -> i32 {
        unsafe { mahjong_storage_get_lang() }
    }

    pub fn save_lang_code(code: i32) {
        unsafe { mahjong_storage_set_lang(code) };
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use std::path::PathBuf;

    /// Path of the settings file under the OS home/config directory.
    fn config_path() -> Option<PathBuf> {
        // Resolved naively from environment variables to avoid a
        // directories dependency.
        let base = std::env::var_os("APPDATA")
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)?;
        Some(base.join("mahjong_rs.lang"))
    }

    pub fn load_lang_code() -> i32 {
        let Some(path) = config_path() else {
            return -1;
        };
        match std::fs::read_to_string(&path) {
            Ok(s) => match s.trim() {
                "ja" => 0,
                "en" => 1,
                _ => -1,
            },
            Err(_) => -1,
        }
    }

    pub fn save_lang_code(code: i32) {
        let Some(path) = config_path() else {
            return;
        };
        let value = match code {
            0 => "ja",
            1 => "en",
            _ => return,
        };
        let _ = std::fs::write(&path, value);
    }
}

use backend::{load_lang_code, save_lang_code};
