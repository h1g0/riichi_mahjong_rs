//! 局の管理
//!
//! 1局分のゲーム進行を管理する。
//! ツモ → 打牌 → 鳴き判定 → 次の手番 のターンフローを制御する。

mod calls;
#[cfg(debug_assertions)]
mod diagnostics;
mod draws;
#[cfg(test)]
mod test_helpers;
mod turn;
mod win;

use mahjong_core::scoring::score::ScoreItem;
use mahjong_core::settings::Settings;
use mahjong_core::tile::{Tile, TileType, Wind};
use mahjong_core::winning_hand::name::Kind;

use crate::player::Player;
use crate::protocol::{AvailableCall, CallType, MeldTiles, PlayerHandInfo, ServerEvent};
use crate::wall::Wall;

/// リーチ棒1本の点数
const RIICHI_STICK_VALUE: i32 = 1000;
/// リーチ宣言に必要な最低持ち点
const RIICHI_MIN_SCORE: i32 = 1000;

/// ターンのフェーズ
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnPhase {
    /// ツモフェーズ: 現在のプレイヤーがツモる
    Draw,
    /// 打牌待ち: 現在のプレイヤーの打牌を待つ
    WaitForDiscard,
    /// 鳴き待ち: 打牌後、他プレイヤーの鳴き応答を待つ
    WaitForCalls,
    /// 九種九牌待ち: プレイヤーが流局を宣言するか選択するのを待つ
    WaitForNineTerminals,
    /// 局終了
    RoundOver,
}

/// 局の結果
#[derive(Debug, Clone)]
pub enum RoundResult {
    /// ツモ和了
    Tsumo { winner: usize, winning_tile: Tile },
    /// ロン和了（1人・ダブロン・トリロン共通）
    Ron {
        /// 和了プレイヤーのインデックス（打順優先順: 下家→対面→上家）
        winners: Vec<usize>,
        loser: usize,
        winning_tile: Tile,
    },
    /// 荒牌流局（牌山切れ）
    ExhaustiveDraw {
        /// 親がテンパイしているか
        dealer_tenpai: bool,
    },
    /// 途中流局（四風連打、四家立直、九種九牌）
    SpecialDraw,
}

/// 鳴き解決後の進行先
#[derive(Debug, Clone)]
enum CallResolution {
    /// 通常の打牌後処理
    AfterDiscard,
    /// 加カンに対する搶槓判定後の処理
    AfterKakan { caller: usize, tile_type: TileType },
}

/// 鳴き待ち中の状態
#[derive(Debug, Clone)]
pub struct CallState {
    /// 捨てられた牌
    pub discarded_tile: Tile,
    /// 捨てたプレイヤー
    pub discarder: usize,
    /// 各プレイヤーが可能な鳴きのリスト（空=鳴き不可）
    pub available_calls: [Vec<AvailableCall>; 4],
    /// 各プレイヤーが応答済みか（true=応答済みまたは対象外）
    pub responded: [bool; 4],
    /// ロンを宣言したプレイヤー（複数ロン対応用）
    pub ron_declared: Vec<usize>,
    /// ポンを宣言したプレイヤーと使う手牌2枚
    pub pon_declared: Option<(usize, [Tile; 2])>,
    /// 大明カンを宣言したプレイヤー
    pub daiminkan_declared: Option<usize>,
    /// チーを宣言したプレイヤーと使う手牌2枚
    pub chi_declared: Option<(usize, [Tile; 2])>,
    /// 全員応答後の進行先
    resolution: CallResolution,
}

/// 1局分の状態
pub struct Round {
    /// 牌山
    pub wall: Wall,
    /// 4人のプレイヤー
    pub players: [Player; 4],
    /// 場風
    pub round_wind: Wind,
    /// 親のプレイヤーインデックス（0-3）
    pub dealer: usize,
    /// 現在の手番プレイヤー（0-3）
    pub current_player: usize,
    /// 本場数
    pub honba: usize,
    /// 場に出ている供託リーチ棒の本数
    pub riichi_sticks: usize,
    /// ターンフェーズ
    pub phase: TurnPhase,
    /// 局の結果（終了時にセット）
    pub result: Option<RoundResult>,
    /// 溜まったイベントキュー
    events: Vec<(usize, ServerEvent)>,
    /// 鳴き待ち中の状態
    pub call_state: Option<CallState>,
    /// 直前のツモが嶺上牌か
    pub last_draw_was_dead_wall: bool,
    /// 包（責任払い）の記録。プレイヤーごとの (確定した役満, 責任を負うプレイヤー) のリスト。
    /// 大三元・大四喜・四槓子を確定させる鳴きが発生した時点で記録される。
    pub pao: [Vec<(Kind, usize)>; 4],
    /// プレイヤー人数（四麻=4、三麻=3。三麻ではシート3はダミー）
    pub player_count: usize,
    /// ゲーム設定
    pub settings: Settings,
}

impl Round {
    /// 新しい局を開始する
    ///
    /// - `round_wind`: 場風（東場なら East）
    /// - `dealer`: 親のプレイヤーインデックス（0-3）
    /// - `initial_scores`: 各プレイヤーの初期点数
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        round_wind: Wind,
        dealer: usize,
        initial_scores: [i32; 4],
        honba: usize,
        riichi_sticks: usize,
        round_number: usize,
        total_rounds: usize,
        settings: Settings,
    ) -> Self {
        Self::with_wall(
            Wall::new(settings.three_player),
            round_wind,
            dealer,
            initial_scores,
            honba,
            riichi_sticks,
            round_number,
            total_rounds,
            settings,
        )
    }

    /// 固定シードの牌山でラウンドを生成する
    ///
    /// 牌山が決定的になるため、シミュレーション・再現性のあるテストに使用する。
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_seed(
        seed: u64,
        round_wind: Wind,
        dealer: usize,
        initial_scores: [i32; 4],
        honba: usize,
        riichi_sticks: usize,
        round_number: usize,
        total_rounds: usize,
        settings: Settings,
    ) -> Self {
        Self::with_wall(
            Wall::new_with_seed(seed, settings.three_player),
            round_wind,
            dealer,
            initial_scores,
            honba,
            riichi_sticks,
            round_number,
            total_rounds,
            settings,
        )
    }

    /// 指定した牌山から局を開始する共通処理
    #[allow(clippy::too_many_arguments)]
    fn with_wall(
        mut wall: Wall,
        round_wind: Wind,
        dealer: usize,
        initial_scores: [i32; 4],
        honba: usize,
        riichi_sticks: usize,
        round_number: usize,
        total_rounds: usize,
        settings: Settings,
    ) -> Self {
        let player_count = settings.player_count();
        let dealt = wall.deal(player_count);

        // 座席の風を割り当て: dealer=東, 反時計回りに南西（北）
        // 三麻では北家は存在せず、シート3はダミー（空手牌・点数0）となる
        let winds: [Wind; 4] = std::array::from_fn(|i| {
            if i < player_count {
                Wind::from_index((i + player_count - dealer) % player_count)
            } else {
                Wind::North
            }
        });

        let players: [Player; 4] = std::array::from_fn(|i| {
            if i < player_count {
                Player::new(winds[i], dealt[i].clone(), initial_scores[i])
            } else {
                // ダミー席: 空手牌・点数0。テンパイ・フリテン・リーチには決してならない
                Player::new(winds[i], Vec::new(), 0)
            }
        });

        let dora_indicators = wall.dora_indicators();

        // 各プレイヤーにゲーム開始イベントを送信
        let mut events = Vec::new();
        for (i, player) in players.iter().enumerate().take(player_count) {
            events.push((
                i,
                ServerEvent::GameStarted {
                    seat_wind: player.seat_wind,
                    hand: player.hand.tiles().to_vec(),
                    scores: initial_scores,
                    round_wind,
                    dora_indicators: dora_indicators.clone(),
                    round_number,
                    total_rounds,
                    honba,
                    riichi_sticks,
                    three_player: settings.three_player,
                    nuki_dora: settings.three_player && settings.nuki_dora,
                },
            ));
        }

        Round {
            wall,
            players,
            round_wind,
            dealer,
            current_player: dealer,
            honba,
            riichi_sticks,
            phase: TurnPhase::Draw,
            result: None,
            events,
            call_state: None,
            last_draw_was_dead_wall: false,
            pao: std::array::from_fn(|_| Vec::new()),
            player_count,
            settings,
        }
    }

    /// 和了役に包（責任払い）の対象役満が含まれる場合、責任を負うプレイヤーを返す
    ///
    /// 複数の包が記録されている場合は、後から成立した包を優先する。
    pub(super) fn pao_player_for_win(
        &self,
        winner: usize,
        yaku_list: &[(ScoreItem, u32)],
    ) -> Option<usize> {
        self.pao[winner]
            .iter()
            .rev()
            .find_map(|(pao_kind, liable)| {
                yaku_list
                    .iter()
                    .any(|(item, _)| matches!(item, ScoreItem::Yaku(kind) if kind == pao_kind))
                    .then_some(*liable)
            })
    }

    /// 指定プレイヤーの次の手番プレイヤーを返す（プレイヤー人数で循環）
    fn next_seat(&self, seat: usize) -> usize {
        (seat + 1) % self.player_count
    }

    /// 各プレイヤーの点数を返す
    /// 全プレイヤーの手牌情報を構築する（三麻ではダミー席を含めない）
    fn build_player_hands(&self) -> Vec<PlayerHandInfo> {
        self.players
            .iter()
            .take(self.player_count)
            .map(|p| {
                let melds: Vec<MeldTiles> = p
                    .hand
                    .melds()
                    .iter()
                    .map(|open| {
                        let tiles: Vec<Tile> = open.expanded_tiles();
                        let call_type = match open.category {
                            mahjong_core::hand_info::meld::MeldType::Chi => CallType::Chi,
                            mahjong_core::hand_info::meld::MeldType::Pon => CallType::Pon,
                            mahjong_core::hand_info::meld::MeldType::Kan => {
                                if open.from == mahjong_core::hand_info::meld::MeldFrom::Myself {
                                    CallType::Ankan
                                } else {
                                    CallType::Daiminkan
                                }
                            }
                            mahjong_core::hand_info::meld::MeldType::Kakan => CallType::Kakan,
                        };
                        MeldTiles { call_type, tiles }
                    })
                    .collect();

                PlayerHandInfo {
                    wind: p.seat_wind,
                    hand: p.hand.tiles().to_vec(),
                    melds,
                    pei: p.pei_tiles.clone(),
                }
            })
            .collect()
    }

    pub fn get_scores(&self) -> [i32; 4] {
        [
            self.players[0].score,
            self.players[1].score,
            self.players[2].score,
            self.players[3].score,
        ]
    }

    /// 溜まったイベントを取り出す
    /// 戻り値: (対象プレイヤーインデックス, イベント) のリスト
    pub fn drain_events(&mut self) -> Vec<(usize, ServerEvent)> {
        std::mem::take(&mut self.events)
    }

    /// 自動プレイヤー（CPU）のターンを進める（ツモ切り）
    /// 現在のプレイヤーがツモ → ツモ切りを1ターン分行う
    pub fn advance_auto_player(&mut self) -> bool {
        if self.phase == TurnPhase::RoundOver {
            return false;
        }

        // ツモ
        if !self.do_draw() {
            return false;
        }

        // 流局チェック
        if self.phase == TurnPhase::RoundOver {
            return true;
        }

        // ツモ切り
        self.do_discard(None)
    }

    /// 局が終了したかどうか
    pub fn is_over(&self) -> bool {
        self.phase == TurnPhase::RoundOver
    }
}

/// 鳴き応答の種類
#[derive(Debug, Clone)]
pub enum CallResponse {
    /// ロン
    Ron,
    /// ポン（手牌から使う牌2枚。赤ドラも区別する）
    Pon { hand_tile_types: [Tile; 2] },
    /// 大明カン
    Daiminkan,
    /// チー（手牌から使う牌2枚。赤ドラも区別する）
    Chi { hand_tile_types: [Tile; 2] },
    /// パス
    Pass,
}

#[cfg(test)]
mod tests;
