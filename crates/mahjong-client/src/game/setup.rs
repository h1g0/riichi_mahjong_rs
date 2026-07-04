//! 設定画面・オンラインUIの状態

use super::*;
use mahjong_server::table::GameLength;

/// オンライン対戦UI（メニュー・ロビー）の状態
#[derive(Debug, Clone)]
pub struct OnlineUiState {
    /// 表示名の入力欄
    pub name_input: String,
    /// ルームコードの入力欄
    pub code_input: String,
    /// true ならルームコード欄、false なら名前欄にフォーカス
    pub code_focused: bool,
    /// 接続状況・エラーの表示文言
    pub status_line: Option<String>,
    /// status_line がエラーか（赤色で表示する）
    pub status_is_error: bool,
    /// 入室中のルーム表示（メインループがアダプターからコピーする）
    pub room: Option<RoomViewUi>,
    /// 手番の制限時間の残り秒数（オンラインで自分の手番のときのみ Some）
    pub turn_remaining: Option<u32>,
    /// ルーム作成時に三麻（3人打ち）ルームにするか
    pub three_player: bool,
    /// ルーム作成時に半荘戦にするか（false なら東風戦）
    pub hanchan: bool,
    /// ルーム作成時に北抜きドラありにするか（三麻のみ有効）
    pub nuki_dora: bool,
}

impl OnlineUiState {
    pub fn new() -> Self {
        OnlineUiState {
            // 既定の表示名は送信時に display_name() が言語に応じて補う
            name_input: String::new(),
            code_input: String::new(),
            code_focused: false,
            status_line: None,
            status_is_error: false,
            room: None,
            turn_remaining: None,
            three_player: false,
            hanchan: false,
            nuki_dora: true,
        }
    }

    /// ルーム作成時に送るルール設定を組み立てる
    ///
    /// UIで選択できないルールは既定値のまま。ルール選択UIを増やす場合は
    /// ここに反映すればサーバへそのまま伝わる。
    pub fn build_rules(&self) -> mahjong_core::settings::Settings {
        mahjong_core::settings::Settings {
            three_player: self.three_player,
            nuki_dora: self.nuki_dora,
            ..mahjong_core::settings::Settings::new()
        }
    }

    /// ルーム作成時に送る対局の長さ（東風戦か半荘戦か）
    pub fn length(&self) -> GameLength {
        length_from_hanchan(self.hanchan)
    }

    /// 選択中の対局モードのインデックス（[`MODE_COUNT`] 参照）
    pub fn mode_index(&self) -> usize {
        mode_index_from_flags(self.three_player, self.hanchan)
    }

    /// 対局モードのインデックスから三麻・半荘フラグを設定する
    pub fn set_mode_index(&mut self, idx: usize) {
        (self.three_player, self.hanchan) = mode_flags_from_index(idx);
    }
}

/// 対局モード（四人東風/四人半荘/三人東風/三人半荘）の個数
pub const MODE_COUNT: usize = 4;

/// 三麻・半荘フラグから対局モードのインデックスを求める
///
/// 0=四人東風, 1=四人半荘, 2=三人東風, 3=三人半荘
pub fn mode_index_from_flags(three_player: bool, hanchan: bool) -> usize {
    (three_player as usize) * 2 + (hanchan as usize)
}

/// 対局モードのインデックスから (三麻, 半荘) フラグを求める
pub fn mode_flags_from_index(idx: usize) -> (bool, bool) {
    (idx >= 2, idx % 2 == 1)
}

/// 半荘フラグから対局の長さを求める
fn length_from_hanchan(hanchan: bool) -> GameLength {
    if hanchan {
        GameLength::Hanchan
    } else {
        GameLength::EastOnly
    }
}

/// ロビー画面に表示するルーム情報
#[derive(Debug, Clone)]
pub struct RoomViewUi {
    /// ルームコード
    pub code: String,
    /// 各座席の表示文言（東南西北の順）
    pub seat_labels: [String; 4],
    /// 自分がホストか（対局開始ボタンの表示に使う）
    pub is_host: bool,
    /// 三麻（3人打ち）ルームか
    pub three_player: bool,
    /// 半荘戦ルームか（false なら東風戦）
    pub hanchan: bool,
}

/// 対局開始前の設定画面の状態
#[derive(Debug, Clone)]
pub struct SetupState {
    /// 三麻（3人打ち）モードか
    pub three_player: bool,
    /// 半荘戦か（false なら東風戦）
    pub hanchan: bool,
    /// 北抜きドラありか（三麻のみ有効）
    pub nuki_dora: bool,
    /// 各CPUの強さ設定（下家, 対面, 上家）
    pub cpu_levels: [usize; 3],
    /// 各CPUの性格設定（下家, 対面, 上家）
    pub cpu_personalities: [usize; 3],
}

impl SetupState {
    pub fn new() -> Self {
        SetupState {
            three_player: false,
            hanchan: false,
            nuki_dora: true,
            cpu_levels: [1, 1, 1],        // 全員 Normal
            cpu_personalities: [0, 1, 2], // Balanced, Speedy, HighValue
        }
    }

    /// このモードで設定するCPUの人数（四麻=3、三麻=2）
    pub fn cpu_count(&self) -> usize {
        if self.three_player { 2 } else { 3 }
    }

    /// 選択中のルール設定を組み立てる
    ///
    /// UIで選択できないルールは既定値のまま。ルール選択UIを増やす場合は
    /// ここに反映すれば、ローカル・オンラインの両方に伝わる。
    pub fn build_rules(&self) -> mahjong_core::settings::Settings {
        mahjong_core::settings::Settings {
            three_player: self.three_player,
            nuki_dora: self.nuki_dora,
            ..mahjong_core::settings::Settings::new()
        }
    }

    /// ゲーム設定を組み立てる（持ち点はルールから決まる）
    pub fn build_game_settings(&self) -> mahjong_server::table::GameSettings {
        mahjong_server::table::GameSettings::with_rules(self.length(), self.build_rules())
    }

    /// 対局の長さ（東風戦か半荘戦か）
    pub fn length(&self) -> GameLength {
        length_from_hanchan(self.hanchan)
    }

    /// 選択中の対局モードのインデックス（[`MODE_COUNT`] 参照）
    pub fn mode_index(&self) -> usize {
        mode_index_from_flags(self.three_player, self.hanchan)
    }

    /// 対局モードのインデックスから三麻・半荘フラグを設定する
    pub fn set_mode_index(&mut self, idx: usize) {
        (self.three_player, self.hanchan) = mode_flags_from_index(idx);
    }

    pub fn level_count() -> usize {
        3
    }
    pub fn personality_count() -> usize {
        4
    }

    /// 設定からCpuConfigの配列を生成する
    pub fn build_configs(&self) -> [CpuConfig; 3] {
        let to_level = |idx: usize| -> CpuLevel {
            match idx {
                0 => CpuLevel::Weak,
                2 => CpuLevel::Strong,
                _ => CpuLevel::Normal,
            }
        };
        let to_personality = |idx: usize| -> CpuPersonality {
            match idx {
                1 => CpuPersonality::Speedy,
                2 => CpuPersonality::HighValue,
                3 => CpuPersonality::Defensive,
                _ => CpuPersonality::Balanced,
            }
        };
        [
            CpuConfig::new(
                to_level(self.cpu_levels[0]),
                to_personality(self.cpu_personalities[0]),
            ),
            CpuConfig::new(
                to_level(self.cpu_levels[1]),
                to_personality(self.cpu_personalities[1]),
            ),
            CpuConfig::new(
                to_level(self.cpu_levels[2]),
                to_personality(self.cpu_personalities[2]),
            ),
        ]
    }

    /// 設定から CPU 指定（オンライン対戦でホストが送る）を生成する
    pub fn build_cpu_specs(&self) -> [CpuSpec; 3] {
        self.build_configs()
            .map(|config| CpuSpec::from_config(&config))
    }
}
