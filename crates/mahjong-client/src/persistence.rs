//! 設定の永続化
//!
//! 表示言語の選択をプラットフォームごとの方法で保存・読み込みする。
//! - WASM: `js/storage.js` 経由で `localStorage`（wasm-bindgen 不使用）
//! - ネイティブ: ユーザのホーム配下の小さなテキストファイル
//!
//! 言語コードは JS 側と一致させる: 0 = 日本語, 1 = 英語（未設定は -1）。

use mahjong_core::settings::Lang;

/// 言語を整数コードへ変換する（保存形式・FFI 共通）。
fn lang_to_code(lang: Lang) -> i32 {
    match lang {
        Lang::Ja => 0,
        Lang::En => 1,
    }
}

/// 整数コードから言語へ変換する（不明な値は `None`）。
fn code_to_lang(code: i32) -> Option<Lang> {
    match code {
        0 => Some(Lang::Ja),
        1 => Some(Lang::En),
        _ => None,
    }
}

/// 保存された表示言語を読み込む（未保存・不正なら `None`）。
pub fn load_lang() -> Option<Lang> {
    code_to_lang(load_lang_code())
}

/// 表示言語を保存する（失敗しても致命的ではないので無視する）。
pub fn save_lang(lang: Lang) {
    save_lang_code(lang_to_code(lang));
}

#[cfg(target_arch = "wasm32")]
mod backend {
    // storage.js が miniquad のプラグイン機構で importObject.env に注入する関数群
    unsafe extern "C" {
        fn mahjong_storage_get_lang() -> i32;
        fn mahjong_storage_set_lang(code: i32);
    }

    /// storage.js プラグインのバージョン照合用
    ///
    /// mq_js_bundle.js の init_plugins が `{プラグイン名}_crate_version` を
    /// 呼び、JS 側の version と一致するか検証する。
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

    /// 設定ファイルのパス（OS のホーム/設定ディレクトリ配下）。
    fn config_path() -> Option<PathBuf> {
        // 追加依存を避け、環境変数からホーム配下を素朴に決める。
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
