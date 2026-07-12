//! ゲーム状態管理
//!
//! サーバから受信したイベントに基づいてクライアント側の状態を管理する。

use std::collections::VecDeque;

use macroquad::prelude::*;
use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::HandAnalyzer;
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::settings::Lang;
use mahjong_core::tile::{Tile, TileType, Wind};

use crate::i18n::{Key, Translator};
use mahjong_server::cpu::client::{CpuConfig, CpuLevel, CpuPersonality};
use mahjong_server::protocol::net::CpuSpec;
use mahjong_server::protocol::{
    AvailableCall, CallType, ClientAction, PlayerHandInfo, ServerEvent,
};

mod events;
mod input;
mod labels;
mod setup;
#[cfg(test)]
mod tests;

pub use labels::PlayerLabel;
pub use setup::{GameMode, OnlineUiState, RoomViewUi, SetupState};

use labels::*;

/// 鳴き・リーチ宣言バナー表示後、後続イベントの適用を保留する時間（秒）
pub const CALL_HOLD_SECS: f64 = 0.9;
/// ロン・ツモ・九種九牌の宣言バナー表示から結果画面表示までの保留時間（秒）
pub const WIN_HOLD_SECS: f64 = 1.2;
/// 宣言バナーの表示時間（秒）
pub const CALL_BANNER_SECS: f64 = 1.5;
/// リーチ中の自動ツモ切りまでの待ち時間（秒）。ツモ牌を見せてから捨てる。
pub const RIICHI_AUTO_DISCARD_SECS: f64 = 1.0;

/// 鳴き・リーチなどの宣言バナー（発声の代わりに画面へ表示する吹き出し）
#[derive(Debug, Clone, Copy)]
pub struct CallBanner {
    /// 表示する文言（ポン・チー・カン・リーチ・ロン・ツモなど）
    pub label: Key,
    /// 表示開始時刻（[`GameState::process_events`] に渡された now と同じ時計）
    pub shown_at: f64,
}

/// 宣言イベントの表示内容（`declaration_for` が返す）
struct Declaration {
    /// バナーを出すプレイヤーと文言（ダブロン時は複数）
    banners: Vec<(Wind, Key)>,
    /// 後続イベントの適用を保留する時間（秒）
    hold_secs: f64,
    /// true ならイベント自体の適用も保留する（ロン・ツモ・九種九牌）
    before_apply: bool,
}

/// 1人分の和了結果（結果画面の1ページ分）
#[derive(Debug, Clone)]
pub struct WinResult {
    pub win_hand: Vec<Tile>,
    pub win_melds: Vec<Meld>,
    pub win_tile: Option<Tile>,
    pub win_is_tsumo: bool,
    pub uradora_indicators: Vec<Tile>,
    pub result_message: String,
    /// 和了者の表示名（例: 「東家」「あなた」）
    pub winner_name: String,
    /// 放銃者の表示名（ツモの場合は None）
    pub loser_name: Option<String>,
    /// 成立した役の一覧（役名, 翻数）
    pub yaku: Vec<(String, u32)>,
    /// 翻数
    pub han: u32,
    /// 符
    pub fu: u32,
    /// 和了点
    pub score_points: i32,
    /// 点数等級名（満貫・跳満など。通常は空）
    pub rank_name: String,
    /// この和了で受け取った供託リーチ棒の本数
    pub riichi_sticks: usize,
}

/// 捨て牌の表示情報
#[derive(Debug, Clone)]
pub struct DiscardInfo {
    pub tile: Tile,
    pub is_tsumogiri: bool,
    /// リーチ宣言牌かどうか（横向きに表示）
    pub is_riichi: bool,
    /// 他家に鳴かれた牌かどうか（薄く表示する）
    pub is_called: bool,
}

/// 他家の手出し打牌の詰めアニメーション状態
///
/// 実卓で「手牌のどこから切ったか」が見えるのと同様に、抜かれた位置へ
/// 一瞬空白を見せてから右側の牌を左へ詰める演出に使う。
#[derive(Debug, Clone, Copy)]
pub struct TedashiAnim {
    /// 打牌前のソート済み手牌（ツモ牌を除く）内で牌が抜かれた位置（0始まり）
    pub gap_index: usize,
    /// 打牌時にツモ牌が張り出していたか（ツモ牌が手牌右端へ滑り込む演出用）
    pub had_drawn: bool,
    /// アニメーション開始時刻（[`GameState::process_events`] に渡された now と同じ時計）
    pub started_at: f64,
}

/// 他プレイヤーの手牌表示情報（相対インデックスで管理）
#[derive(Debug, Clone)]
pub struct OtherPlayerHand {
    /// 手牌（公開時のみ設定。非公開時は空）
    pub hand: Vec<Tile>,
    /// 副露（鳴き）一覧
    pub melds: Vec<Meld>,
    /// 手牌が公開されているか（和了時・テンパイ時）
    pub revealed: bool,
    /// 非公開時の手牌枚数（ツモ牌を除く。裏向き表示用）
    pub concealed_count: usize,
    /// ツモ牌が手牌の右に張り出しているか（ツモ後〜打牌・北抜きまで）
    pub has_drawn: bool,
    /// 直近の手出し打牌の詰めアニメーション状態
    pub tedashi_anim: Option<TedashiAnim>,
}

impl OtherPlayerHand {
    fn new() -> Self {
        OtherPlayerHand {
            hand: Vec::new(),
            melds: Vec::new(),
            revealed: false,
            concealed_count: 13,
            has_drawn: false,
            tedashi_anim: None,
        }
    }

    /// 手牌＋ツモ牌から n 枚が消費された（打牌・副露・北抜き）ときの表示枚数更新。
    ///
    /// ツモ牌が張り出していた場合、残りは手牌へ組み入れられたとみなす
    /// （サーバの `Player::try_discard` などと同じ挙動）。
    fn consume_tiles(&mut self, n: usize) {
        let total = self.concealed_count + usize::from(self.has_drawn);
        self.concealed_count = total.saturating_sub(n);
        self.has_drawn = false;
    }
}

/// クライアント側のゲーム状態
pub struct GameState {
    /// 自分の座席の風
    pub seat_wind: Option<Wind>,
    /// 自分の手牌
    pub hand: Vec<Tile>,
    /// ツモ牌（直近にツモった牌）
    pub drawn: Option<Tile>,
    /// 各プレイヤーの捨て牌（自分=0, 下家=1, 対面=2, 上家=3）
    pub discards: [Vec<DiscardInfo>; 4],
    /// 各プレイヤーの点数
    pub scores: [i32; 4],
    /// 場風
    pub round_wind: Option<Wind>,
    /// ドラ表示牌
    pub dora_indicators: Vec<Tile>,
    /// 裏ドラ表示牌（リーチ和了時のみ公開）
    pub uradora_indicators: Vec<Tile>,
    /// 和了時の手牌情報（結果画面表示用）
    pub win_hand: Vec<Tile>,
    /// 和了時の副露
    pub win_melds: Vec<Meld>,
    /// 和了牌
    pub win_tile: Option<Tile>,
    /// ツモ和了かロン和了か（true=ツモ）
    pub win_is_tsumo: bool,
    /// 山の残り枚数
    pub remaining_tiles: usize,
    /// 選択中の牌のインデックス
    pub selected_tile: Option<usize>,
    /// ツモ牌が選択中か
    pub selected_drawn: bool,
    /// ツモ和了可能か
    pub can_tsumo: bool,
    /// リーチ宣言可能か
    pub can_riichi: bool,
    /// 自分の手番で暗カン可能な牌
    pub self_kan_options: Vec<Tile>,
    /// 自分がリーチ中か
    pub is_riichi: bool,
    /// リーチ中の自動ツモ切りを実行する時刻（ツモ牌を見せる待機中のみ Some）
    riichi_auto_discard_at: Option<f64>,
    /// リーチ宣言のための打牌選択中か
    pub riichi_selection_mode: bool,
    /// リーチ可能な手牌インデックス
    pub riichi_selectable_tiles: Vec<usize>,
    /// ツモ牌切りでリーチ可能か
    pub riichi_selectable_drawn: bool,
    /// 喰い替え禁止により、鳴き直後の打牌で捨てられない牌種
    /// （チー・ポン直後にのみ設定され、打牌・ツモで解除される）
    pub forbidden_discards: Vec<TileType>,
    /// 喰い替え禁止牌を選択しようとしたか（「喰い替えです！」警告の表示用）
    pub selected_forbidden_swap: bool,
    /// 局の結果メッセージ
    pub result_message: Option<String>,
    /// 和了結果一覧（ダブロン・トリロン時は複数）
    pub win_results: Vec<WinResult>,
    /// 現在表示中の和了結果インデックス
    pub win_result_index: usize,
    /// 自分の手番か
    pub is_my_turn: bool,
    /// 現在手番のプレイヤーの席風（ツモ・鳴きイベントで更新。局の開始・終了で None）
    pub turn_player: Option<Wind>,
    /// ゲームフェーズ
    pub phase: GamePhase,
    /// 鳴き可能な選択肢
    pub available_calls: Vec<AvailableCall>,
    /// 鳴き対象の牌
    pub call_target_tile: Option<Tile>,
    /// 鳴き対象の捨てたプレイヤー
    pub call_discarder: Option<Wind>,
    /// 自分の副露（鳴き）一覧
    pub melds: Vec<Meld>,
    /// 局番号（0=東1局, 1=東2局, ...）
    pub round_number: usize,
    /// 本場数
    pub honba: usize,
    /// 場に出ている供託リーチ棒の本数
    pub riichi_sticks: usize,
    /// フリテン状態か
    pub is_furiten: bool,
    /// 選択中の牌を捨てるとフリテンになるか
    pub selected_would_cause_furiten: bool,
    /// 他プレイヤーの手牌情報（下家=0, 対面=1, 上家=2）
    pub other_players: [OtherPlayerHand; 3],
    /// リーチ宣言済みで次の打牌がリーチ宣言牌となるプレイヤーの風（一時フラグ）
    pending_riichi_player: Option<Wind>,
    /// 直前に捨て牌したプレイヤーの風（鳴き元の判定に使用）
    last_discarder: Option<Wind>,
    /// チーの組み合わせ選択UI表示中か（複数の選択肢がある場合）
    pub chi_option_selecting: bool,
    /// チー選択UIに表示する選択肢（手牌から使う2枚の牌）
    pub chi_pending_options: Vec<[Tile; 2]>,
    /// ポンの組み合わせ選択UI表示中か（赤ドラの有無で選択肢が分かれる場合）
    pub pon_option_selecting: bool,
    /// ポン選択UIに表示する選択肢（手牌から使う2枚の牌）
    pub pon_pending_options: Vec<[Tile; 2]>,
    /// 九種九牌の宣言選択中か
    pub nine_terminals_pending: bool,
    /// 対局開始前設定
    pub setup_state: SetupState,
    /// オンライン対戦UIの状態
    pub online_state: OnlineUiState,
    /// 各座席のプレイヤー種別（座席インデックス順 = scores と同じ並び）
    pub player_labels: [PlayerLabel; 4],
    /// 自分の座席インデックス（ローカルは常に0、オンラインは your_seat）
    pub my_seat: usize,
    /// 起家の座席インデックス（GameStarted から逆算して更新される）
    ///
    /// 起家はランダムで決まるため座席0とは限らない。最終順位の
    /// 同点判定（起家に近い席が上位）に使う。
    pub initial_dealer_seat: usize,
    /// プレイヤー人数（四麻=4、三麻=3。GameStarted で設定される）
    pub player_count: usize,
    /// 北抜きドラが有効か（三麻のみ true になり得る）
    pub nuki_dora: bool,
    /// 各プレイヤーの北抜き枚数（風のインデックス順: 東=0, 南=1, 西=2）
    pub pei_counts: [u8; 4],
    /// 北抜き可能か（自分の手番で手牌・ツモ牌に北がある場合）
    pub can_pei: bool,
    /// 各プレイヤーの宣言バナー（相対位置順: 自分=0, 下家=1, 対面=2, 上家=3）
    pub call_banners: [Option<CallBanner>; 4],
    /// 未適用のサーバイベント（宣言演出中は適用を保留する）
    pending_events: VecDeque<ServerEvent>,
    /// この時刻まで後続イベントの適用を保留する
    event_hold_until: f64,
    /// キュー先頭イベントの宣言バナーを表示済みか（適用前保留の管理用）
    head_announced: bool,
    /// 直近に [`process_events`](Self::process_events) へ渡された時刻。
    /// イベント適用時のアニメーション開始時刻として使う。
    clock: f64,
    /// 表示言語
    pub lang: Lang,
}

/// モード選択・CPU設定画面の遷移元（戻り先と確定時の動作を決める）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuOrigin {
    /// CPU対戦（トップ画面 → モード選択 → CPU設定 → 対局開始）
    Local,
    /// オンライン対戦（ルーム作成前のモード選択／ロビーからのCPU設定）
    Online,
}

/// ゲームフェーズ
#[derive(Debug, Clone, PartialEq)]
pub enum GamePhase {
    /// トップ画面（CPU対戦・オンライン対戦・設定・言語設定）
    TopMenu,
    /// 対局モード選択（四人東風〜三人半荘・北抜きドラ）
    ModeSelect(MenuOrigin),
    /// CPU設定画面（強さ・性格。ローカルは対局開始、オンラインは決定）
    CpuSetup(MenuOrigin),
    /// オンライン対戦メニュー（名前・ルームコード入力）
    OnlineMenu,
    /// オンラインロビー（メンバー待ち）
    OnlineLobby,
    /// ゲーム開始前
    WaitingForStart,
    /// 対局中
    Playing,
    /// 局終了（結果表示中）
    RoundResult,
    /// ゲーム終了
    GameOver,
}

impl GameState {
    pub fn new() -> Self {
        GameState {
            seat_wind: None,
            hand: Vec::new(),
            drawn: None,
            discards: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            scores: [25000; 4],
            round_wind: None,
            dora_indicators: Vec::new(),
            uradora_indicators: Vec::new(),
            win_hand: Vec::new(),
            win_melds: Vec::new(),
            win_tile: None,
            win_is_tsumo: false,
            remaining_tiles: 70,
            selected_tile: None,
            selected_drawn: false,
            can_tsumo: false,
            can_riichi: false,
            self_kan_options: Vec::new(),
            is_riichi: false,
            riichi_selection_mode: false,
            riichi_selectable_tiles: Vec::new(),
            riichi_selectable_drawn: false,
            forbidden_discards: Vec::new(),
            selected_forbidden_swap: false,
            result_message: None,
            win_results: Vec::new(),
            win_result_index: 0,
            is_my_turn: false,
            turn_player: None,
            phase: GamePhase::TopMenu,
            available_calls: Vec::new(),
            call_target_tile: None,
            call_discarder: None,
            melds: Vec::new(),
            round_number: 0,
            honba: 0,
            riichi_sticks: 0,
            is_furiten: false,
            selected_would_cause_furiten: false,
            other_players: [
                OtherPlayerHand::new(),
                OtherPlayerHand::new(),
                OtherPlayerHand::new(),
            ],
            pending_riichi_player: None,
            last_discarder: None,
            chi_option_selecting: false,
            chi_pending_options: Vec::new(),
            pon_option_selecting: false,
            pon_pending_options: Vec::new(),
            nine_terminals_pending: false,
            setup_state: SetupState::new(),
            online_state: OnlineUiState::new(),
            player_labels: [
                PlayerLabel::Me,
                PlayerLabel::Cpu {
                    level: "Normal".to_string(),
                    personality: "Balanced".to_string(),
                },
                PlayerLabel::Cpu {
                    level: "Normal".to_string(),
                    personality: "Speedy".to_string(),
                },
                PlayerLabel::Cpu {
                    level: "Normal".to_string(),
                    personality: "HighValue".to_string(),
                },
            ],
            my_seat: 0,
            initial_dealer_seat: 0,
            player_count: 4,
            nuki_dora: false,
            pei_counts: [0; 4],
            can_pei: false,
            riichi_auto_discard_at: None,
            call_banners: [None; 4],
            pending_events: VecDeque::new(),
            event_hold_until: 0.0,
            head_announced: false,
            clock: 0.0,
            // 保存された表示言語を読み込む（未保存なら日本語）。
            // 「もう一度」などで new() が再生成されても選択を保つ。
            lang: crate::persistence::load_lang().unwrap_or(Lang::Ja),
        }
    }

    /// 三麻かどうか
    pub fn is_three_player(&self) -> bool {
        self.player_count == 3
    }

    /// 現在の表示言語の [`Translator`](crate::i18n::Translator) を返す。
    pub fn tr(&self) -> crate::i18n::Translator {
        crate::i18n::Translator::new(self.lang)
    }

    /// ローカル対局のプレイヤー種別を設定する（自分=座席0, CPU=座席1〜3）
    pub fn set_local_players(&mut self, cpu_configs: &[CpuConfig; 3]) {
        self.my_seat = 0;
        self.player_labels = [
            PlayerLabel::Me,
            cpu_label(&cpu_configs[0]),
            cpu_label(&cpu_configs[1]),
            cpu_label(&cpu_configs[2]),
        ];
    }

    /// オンライン対局のプレイヤー種別を設定する
    ///
    /// `seats` は座席インデックス順、`your_seat` は自分の座席。
    pub fn set_online_players(&mut self, seats: &[PlayerLabel; 4], your_seat: usize) {
        self.my_seat = your_seat;
        self.player_labels = seats.clone();
    }

    /// 最終順位（座席インデックス, 点数）を上位から並べて返す
    ///
    /// 同点の場合は起家に近い席が上位になる。三麻ではダミー席（シート3）を
    /// 除外する。
    pub fn final_rankings(&self) -> Vec<(usize, i32)> {
        let n = self.player_count;
        let mut rankings: Vec<(usize, i32)> = self
            .scores
            .iter()
            .enumerate()
            .take(n)
            .map(|(i, &s)| (i, s))
            .collect();
        rankings.sort_by_key(|&(seat, score)| {
            (
                std::cmp::Reverse(score),
                (seat + n - self.initial_dealer_seat) % n,
            )
        });
        rankings
    }

    /// 自分の座席の風インデックスを返す（未設定時は0）。
    pub fn my_wind_index(&self) -> usize {
        self.seat_wind.map(|w| w.to_index()).unwrap_or(0)
    }

    /// 東1局開始時の自分の風インデックスを返す。
    ///
    /// 三麻の描画スロットはこの値で固定するため、局が進んで風が
    /// 回っても各家の表示位置は動かない。
    pub fn my_initial_wind_index(&self) -> usize {
        (self.my_seat + self.player_count - self.initial_dealer_seat) % self.player_count
    }

    fn relative_player_index(&self, wind: Wind) -> usize {
        let my_idx = self.my_wind_index();
        let their_idx = wind.to_index();
        // 三麻では風インデックスは0〜2で循環する
        (their_idx + self.player_count - my_idx) % self.player_count
    }

    /// 現在の局の風から、そのプレイヤーの東1局開始時の風インデックスを返す。
    fn initial_wind_index(&self, wind: Wind) -> usize {
        (self.my_initial_wind_index() + self.relative_player_index(wind)) % self.player_count
    }

    /// CallType → MeldType 変換
    fn call_type_to_meld_type(call_type: &CallType) -> MeldType {
        match call_type {
            CallType::Chi => MeldType::Chi,
            CallType::Pon => MeldType::Pon,
            CallType::Ankan | CallType::Daiminkan => MeldType::Kan,
            CallType::Kakan => MeldType::Kakan,
            CallType::Ron => MeldType::Pon, // フォールバック（使われない）
        }
    }

    /// 鳴いたプレイヤー(caller)から見て、鳴き元(discarder)がどの位置かを返す
    ///
    /// 三麻では席が東1局開始時の位置で固定表示されるため（#309）、現在の局の
    /// 風ではなく開始時の風の差分で判定する（#311）。現在の風のままだと風が
    /// 0〜2で回る三麻では mod 4 の差分が局ごとに変わり、倒す牌の位置が画面上の
    /// 鳴き元の席と一致しない局が生じる。四麻は風の差分 mod 4 が局によらず
    /// 不変なので、現在の風をそのまま使う（挙動は従来どおり）。
    fn compute_meld_direction(&self, caller: Wind, discarder: Wind) -> MeldFrom {
        let (caller_idx, discarder_idx) = if self.player_count == 3 {
            (
                self.initial_wind_index(caller),
                self.initial_wind_index(discarder),
            )
        } else {
            (caller.to_index(), discarder.to_index())
        };
        let rel = (discarder_idx + 4 - caller_idx) % 4;
        match rel {
            3 => MeldFrom::Previous,  // 上家
            2 => MeldFrom::Opposite,  // 対面
            1 => MeldFrom::Following, // 下家
            _ => MeldFrom::Myself,    // 自家（通常ここには来ない）
        }
    }

    /// サーバイベントを受信キューへ積む。適用は [`process_events`](Self::process_events) が行う。
    pub fn queue_event(&mut self, event: ServerEvent) {
        self.pending_events.push_back(event);
    }

    /// キュー内のイベントを順に適用する。毎フレーム呼ぶこと。
    ///
    /// 宣言（鳴き・リーチ・北抜き・和了・九種九牌）を伴うイベントでは
    /// 宣言バナーを表示し、[`CALL_HOLD_SECS`]（和了系は [`WIN_HOLD_SECS`]）の間
    /// 後続イベントの適用を保留する。これにより「発声 → 実際の挙動」の順に
    /// 見え、プレイヤーが宣言に気付きやすくなる。和了・九種九牌はイベント
    /// 自体の適用（結果画面への遷移）も保留する。
    pub fn process_events(&mut self, now: f64) {
        self.clock = now;

        // 表示時間を過ぎたバナーを片付ける
        for slot in &mut self.call_banners {
            if slot.is_some_and(|b| now - b.shown_at >= CALL_BANNER_SECS) {
                *slot = None;
            }
        }

        loop {
            if now < self.event_hold_until || self.pending_events.is_empty() {
                return;
            }

            if !self.head_announced
                && let Some(decl) = self.declaration_for(&self.pending_events[0])
            {
                for &(wind, label) in &decl.banners {
                    let rel = self.relative_player_index(wind);
                    self.call_banners[rel] = Some(CallBanner {
                        label,
                        shown_at: now,
                    });
                }
                self.event_hold_until = now + decl.hold_secs;
                if decl.before_apply {
                    // 宣言だけ見せて、イベントの適用は保留が明けてから行う。
                    // 保留中に古い操作UI（ロン・ツモボタン等）が残らないよう畳む。
                    self.head_announced = true;
                    self.available_calls.clear();
                    self.can_tsumo = false;
                    self.can_riichi = false;
                } else {
                    let event = self.pending_events.pop_front().expect("front checked");
                    self.handle_event(event);
                    // 自分の鳴きでは PlayerCalled の直後に HandUpdated が届く。
                    // これを保留すると、保留中の打牌が保留明けの HandUpdated で
                    // 巻き戻されて手牌がサーバと食い違うため、同時に適用する。
                    if matches!(
                        self.pending_events.front(),
                        Some(ServerEvent::HandUpdated { .. })
                    ) {
                        let event = self.pending_events.pop_front().expect("front checked");
                        self.handle_event(event);
                    }
                }
                continue;
            }

            let event = self.pending_events.pop_front().expect("front checked");
            self.head_announced = false;
            self.handle_event(event);
        }
    }

    /// イベントが宣言（発声）を伴う場合、その表示内容を返す。
    fn declaration_for(&self, event: &ServerEvent) -> Option<Declaration> {
        match event {
            ServerEvent::PlayerCalled {
                player, call_type, ..
            } => {
                let label = match call_type {
                    CallType::Pon => Key::Pon,
                    CallType::Chi => Key::Chi,
                    CallType::Ankan | CallType::Daiminkan | CallType::Kakan => Key::Kan,
                    // ロンは RoundWon 側で宣言する（サーバは通常送らない）
                    CallType::Ron => return None,
                };
                Some(Declaration {
                    banners: vec![(*player, label)],
                    hold_secs: CALL_HOLD_SECS,
                    before_apply: false,
                })
            }
            ServerEvent::PlayerRiichi { player, .. } => Some(Declaration {
                banners: vec![(*player, Key::Riichi)],
                hold_secs: CALL_HOLD_SECS,
                before_apply: false,
            }),
            ServerEvent::PeiDeclared { player, .. } => Some(Declaration {
                banners: vec![(*player, Key::Pei)],
                hold_secs: CALL_HOLD_SECS,
                before_apply: false,
            }),
            ServerEvent::RoundWon { .. } if self.phase == GamePhase::Playing => {
                // ダブロン・トリロンでは RoundWon が連続で届くため、
                // キュー内の和了者すべてのバナーを同時に表示する。
                let banners: Vec<(Wind, Key)> = self
                    .pending_events
                    .iter()
                    .filter_map(|ev| match ev {
                        ServerEvent::RoundWon { winner, loser, .. } => {
                            let label = if loser.is_some() {
                                Key::Ron
                            } else {
                                Key::Tsumo
                            };
                            Some((*winner, label))
                        }
                        _ => None,
                    })
                    .collect();
                Some(Declaration {
                    banners,
                    hold_secs: WIN_HOLD_SECS,
                    before_apply: true,
                })
            }
            ServerEvent::RoundDraw {
                declarer: Some(declarer),
                ..
            } if self.phase == GamePhase::Playing => Some(Declaration {
                banners: vec![(*declarer, Key::NineTerminals)],
                hold_secs: WIN_HOLD_SECS,
                before_apply: true,
            }),
            _ => None,
        }
    }

    /// 和了者・放銃者などに使う席名（日本語は「東家」、英語は「East」）。
    fn wind_to_name(&self, wind: Wind) -> String {
        match self.lang {
            Lang::Ja => format!("{}家", wind.name(Lang::Ja)),
            Lang::En => wind.name(Lang::En).to_string(),
        }
    }

    /// 和了結果などに使うプレイヤー名（例:「CPU2」）。席名ではなくプレイヤー名を表示する。
    fn player_display_name(&self, wind: Wind) -> String {
        let rel = self.relative_player_index(wind);
        let seat = (self.my_seat + rel) % self.player_count;
        self.player_labels[seat].short_name(rel, self.lang)
    }
}
