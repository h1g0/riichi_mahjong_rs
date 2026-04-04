//! 局の管理
//!
//! 1局分のゲーム進行を管理する。
//! ツモ → 打牌 → 鳴き判定 → 次の手番 のターンフローを制御する。

use mahjong_core::hand_info::hand_analyzer::{self, HandAnalyzer};
use mahjong_core::settings::Settings;
use mahjong_core::tile::{Tile, TileType, Wind};

use crate::player::Player;
use crate::protocol::{
    AvailableCall, CallType, DrawReason, MeldTiles, PlayerHandInfo, ServerEvent,
};
use crate::scoring;
use crate::wall::Wall;

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
    pub prevailing_wind: Wind,
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
    /// ゲーム設定
    pub settings: Settings,
}

impl Round {
    /// 新しい局を開始する
    ///
    /// - `prevailing_wind`: 場風（東場なら East）
    /// - `dealer`: 親のプレイヤーインデックス（0-3）
    /// - `initial_scores`: 各プレイヤーの初期点数
    pub fn new(
        prevailing_wind: Wind,
        dealer: usize,
        initial_scores: [i32; 4],
        honba: usize,
        riichi_sticks: usize,
        round_number: usize,
        settings: Settings,
    ) -> Self {
        let mut wall = Wall::new();
        let dealt = wall.deal();

        // 座席の風を割り当て: dealer=東, 反時計回りに南西北
        let winds = [
            Wind::from_index((0 + 4 - dealer) % 4),
            Wind::from_index((1 + 4 - dealer) % 4),
            Wind::from_index((2 + 4 - dealer) % 4),
            Wind::from_index((3 + 4 - dealer) % 4),
        ];

        let players = [
            Player::new(winds[0], dealt[0].clone(), initial_scores[0]),
            Player::new(winds[1], dealt[1].clone(), initial_scores[1]),
            Player::new(winds[2], dealt[2].clone(), initial_scores[2]),
            Player::new(winds[3], dealt[3].clone(), initial_scores[3]),
        ];

        let dora_indicators = wall.dora_indicators();

        // 各プレイヤーにゲーム開始イベントを送信
        let mut events = Vec::new();
        for i in 0..4 {
            events.push((
                i,
                ServerEvent::GameStarted {
                    seat_wind: players[i].seat_wind,
                    hand: players[i].hand.tiles().to_vec(),
                    scores: initial_scores,
                    prevailing_wind,
                    dora_indicators: dora_indicators.clone(),
                    round_number,
                    honba,
                    riichi_sticks,
                },
            ));
        }

        Round {
            wall,
            players,
            prevailing_wind,
            dealer,
            current_player: dealer,
            honba,
            riichi_sticks,
            phase: TurnPhase::Draw,
            result: None,
            events,
            call_state: None,
            last_draw_was_dead_wall: false,
            settings,
        }
    }

    /// 各プレイヤーの点数を返す
    /// 全プレイヤーの手牌情報を構築する
    fn build_player_hands(&self) -> Vec<PlayerHandInfo> {
        self.players
            .iter()
            .map(|p| {
                let melds: Vec<MeldTiles> = p
                    .hand
                    .melds()
                    .iter()
                    .map(|open| {
                        let mut tiles: Vec<Tile> = open.tiles.clone();
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
                        // カンの場合は4枚にする
                        if open.category.is_kan() && tiles.len() == 3 {
                            tiles.push(tiles[0]);
                        }
                        MeldTiles { call_type, tiles }
                    })
                    .collect();

                PlayerHandInfo {
                    wind: p.seat_wind,
                    hand: p.hand.tiles().to_vec(),
                    melds,
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

    /// デバッグ用に自分のツモ時の判定状態を出力する
    #[cfg(debug_assertions)]
    fn log_draw_diagnostics(
        &self,
        player_idx: usize,
        source: &str,
        can_tsumo: bool,
        can_riichi: bool,
    ) {
        if player_idx != 0 {
            return;
        }

        let player = &self.players[player_idx];
        let analyzer = HandAnalyzer::new(&player.hand);
        let win_result = scoring::check_win(
            player,
            self.prevailing_wind,
            true,
            self.wall.is_empty(),
            self.last_draw_was_dead_wall,
        );
        let riichi_discards: Vec<String> = player
            .hand
            .tiles()
            .iter()
            .copied()
            .filter(|&tile| self.can_player_riichi_with_discard(player_idx, Some(tile)))
            .map(|tile| tile.to_string())
            .collect();
        let can_riichi_with_drawn = self
            .can_player_riichi_with_discard(player_idx, None)
            .then(|| String::from("tsumo"));

        match analyzer {
            Ok(analyzer) => {
                let yaku_summary = win_result
                    .score_result
                    .as_ref()
                    .map(|score| {
                        score
                            .yaku_list
                            .iter()
                            .map(|(name, han)| format!("{}:{}", name, han))
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .unwrap_or_default();
                let drawn = player
                    .hand
                    .drawn()
                    .map(|tile| tile.to_string())
                    .unwrap_or_else(|| String::from("none"));
                let mut riichi_options = riichi_discards;
                if let Some(drawn_label) = can_riichi_with_drawn {
                    riichi_options.push(drawn_label);
                }

                eprintln!(
                    "[draw-diag] source={} hand={} drawn={} shanten={} can_tsumo={} is_win={} can_riichi={} riichi_discards=[{}] yaku=[{}] remaining={} score={}",
                    source,
                    player.hand.to_string(),
                    drawn,
                    analyzer.shanten,
                    can_tsumo,
                    win_result.is_win,
                    can_riichi,
                    riichi_options.join(","),
                    yaku_summary,
                    self.wall.remaining(),
                    player.score,
                );
            }
            Err(err) => {
                eprintln!(
                    "[draw-diag] source={} hand={} analyzer_error={} can_tsumo={} can_riichi={}",
                    source,
                    player.hand.to_string(),
                    err,
                    can_tsumo,
                    can_riichi,
                );
            }
        }
    }

    /// ツモフェーズを実行する
    /// 山から1枚引いて現在のプレイヤーに配る
    pub fn do_draw(&mut self) -> bool {
        if self.phase != TurnPhase::Draw {
            return false;
        }

        // 同巡フリテンを解除（自分のツモ番が来たので）
        self.players[self.current_player].is_temporary_furiten = false;

        // 牌山が空なら流局
        if self.wall.is_empty() {
            self.do_exhaustive_draw();
            return true;
        }

        let tile = self.wall.draw().unwrap();
        let remaining = self.wall.remaining();
        self.players[self.current_player].draw(tile);
        self.last_draw_was_dead_wall = false;
        self.phase = TurnPhase::WaitForDiscard;

        // ツモ和了チェック
        let can_tsumo = self.can_tsumo();

        // リーチ可能チェック
        let can_riichi = self.can_player_riichi(self.current_player);
        #[cfg(debug_assertions)]
        self.log_draw_diagnostics(self.current_player, "draw", can_tsumo, can_riichi);

        // フリテン判定
        let is_furiten = self.players[self.current_player].is_furiten();

        // 自分にはツモ牌を公開
        self.events.push((
            self.current_player,
            ServerEvent::TileDrawn {
                tile,
                remaining_tiles: remaining,
                can_tsumo,
                can_riichi,
                is_furiten,
            },
        ));

        // 他プレイヤーには誰がツモったかだけ通知
        let current_wind = self.players[self.current_player].seat_wind;
        for i in 0..4 {
            if i != self.current_player {
                self.events.push((
                    i,
                    ServerEvent::OtherPlayerDrew {
                        player: current_wind,
                        remaining_tiles: remaining,
                    },
                ));
            }
        }

        // 九種九牌チェック: 初回ツモかつ条件を満たす場合に選択を促す
        if self.settings.nine_terminals_draw && self.check_nine_terminals() {
            self.phase = TurnPhase::WaitForNineTerminals;
            self.events
                .push((self.current_player, ServerEvent::NineTerminalsAvailable));
        }

        true
    }

    /// 打牌を実行する
    ///
    /// - `tile`: 捨てる牌（Noneならツモ切り）
    ///
    /// 打牌後、他プレイヤーの鳴き候補をチェックし、
    /// 鳴き候補があれば WaitForCalls フェーズに移行する。
    pub fn do_discard(&mut self, tile: Option<Tile>) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }

        let discarded = self.players[self.current_player].discard(tile);
        let is_tsumogiri = tile.is_none();
        let current_wind = self.players[self.current_player].seat_wind;
        let discarder = self.current_player;

        // 全プレイヤーに打牌を通知
        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::TileDiscarded {
                    player: current_wind,
                    tile: discarded,
                    is_tsumogiri,
                },
            ));
        }

        // 打牌したプレイヤーの一発フラグを無効にする
        // （リーチ宣言直後の打牌ではippatsuは維持される。
        //  ippatsuは宣言後の次の打牌で無効になる。
        //  ただし宣言打牌自体でippatsuがセットされるので、
        //  実質的にはここでは常にfalseに設定する。
        //  宣言打牌時はdeclare_riichi()でippatsu=trueがセットされた直後なので、
        //  ここでfalseにしてしまうのを防ぐため、is_riichi宣言直後のフラグで制御）
        // ※ 一発フラグはリーチ宣言後1巡以内（自分の次のdrawまで）有効
        //   → 自分のdiscard時に解除するのではなく、自分の次のdraw後のdiscardで解除
        //   → ここでは何もしない（player.discard() 内で既に解除済み）

        // 鳴き候補をチェック
        let call_state = self.check_available_calls(discarded, discarder);
        let has_any_calls = call_state.available_calls.iter().any(|c| !c.is_empty());

        if has_any_calls {
            // 鳴き候補がある場合、WaitForCalls フェーズへ
            self.phase = TurnPhase::WaitForCalls;

            // 各プレイヤーに鳴き可能通知を送信
            let discarder_wind = self.players[discarder].seat_wind;
            for i in 0..4 {
                if !call_state.available_calls[i].is_empty() {
                    self.events.push((
                        i,
                        ServerEvent::CallAvailable {
                            tile: discarded,
                            discarder: discarder_wind,
                            calls: call_state.available_calls[i].clone(),
                        },
                    ));
                }
            }

            self.call_state = Some(call_state);
        } else {
            // 鳴き候補がなければ次のプレイヤーへ
            self.current_player = (discarder + 1) % 4;
            self.phase = TurnPhase::Draw;

            // 特殊流局チェック
            self.check_special_draws();
        }

        true
    }

    /// 打牌後の鳴き候補を全てチェックする
    fn check_available_calls(&self, discarded_tile: Tile, discarder: usize) -> CallState {
        let is_last_tile = self.wall.is_empty();
        let mut available_calls: [Vec<AvailableCall>; 4] =
            [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let mut responded = [true; 4]; // デフォルトは応答済み（対象外）

        for i in 0..4 {
            if i == discarder {
                continue;
            }

            let player = &self.players[i];

            // リーチ中は鳴き不可（ロンのみ可）
            // ロン判定: フリテンでなく、和了形であること
            if !player.is_furiten() {
                let win_result =
                    scoring::check_ron(player, discarded_tile, self.prevailing_wind, is_last_tile);
                if win_result.is_win {
                    available_calls[i].push(AvailableCall::Ron);
                }
            }

            // リーチ中は鳴き不可
            if player.is_riichi {
                if !available_calls[i].is_empty() {
                    responded[i] = false;
                }
                continue;
            }

            // ポン判定
            let pon_opts = player.pon_options(discarded_tile);
            if !pon_opts.is_empty() {
                available_calls[i].push(AvailableCall::Pon { options: pon_opts });
            }

            // 大明カン判定（場全体で4回カン済みなら不可）
            if self.total_kan_count() < 4 && player.can_daiminkan(discarded_tile) {
                available_calls[i].push(AvailableCall::Daiminkan);
            }

            // チー判定（上家からのみ＝次のプレイヤー）
            let next_player = (discarder + 1) % 4;
            if i == next_player {
                let chi_opts = player.chi_options(discarded_tile);
                if !chi_opts.is_empty() {
                    available_calls[i].push(AvailableCall::Chi { options: chi_opts });
                }
            }

            if !available_calls[i].is_empty() {
                responded[i] = false;
            }
        }

        CallState {
            discarded_tile,
            discarder,
            available_calls,
            responded,
            ron_declared: Vec::new(),
            pon_declared: None,
            daiminkan_declared: None,
            chi_declared: None,
            resolution: CallResolution::AfterDiscard,
        }
    }

    /// 鳴き応答を処理する
    ///
    /// プレイヤーからの鳴き応答（Ron/Pon/Chi/Pass）を受け付ける。
    /// 全員の応答が揃ったら、優先度に基づいて鳴きを解決する。
    pub fn respond_to_call(&mut self, player_idx: usize, response: CallResponse) -> bool {
        if self.phase != TurnPhase::WaitForCalls {
            return false;
        }

        let call_state = match self.call_state.as_mut() {
            Some(cs) => cs,
            None => return false,
        };

        // 既に応答済みなら無視
        if call_state.responded[player_idx] {
            return false;
        }

        // 応答を記録
        match response {
            CallResponse::Ron => {
                // ロン可能か確認
                if call_state.available_calls[player_idx]
                    .iter()
                    .any(|c| matches!(c, AvailableCall::Ron))
                {
                    call_state.ron_declared.push(player_idx);
                } else {
                    return false;
                }
            }
            CallResponse::Pon { hand_tile_types } => {
                // ポンの組み合わせが有効か確認
                let valid = call_state.available_calls[player_idx].iter().any(|c| {
                    if let AvailableCall::Pon { options } = c {
                        options.contains(&hand_tile_types)
                    } else {
                        false
                    }
                });
                if valid {
                    call_state.pon_declared = Some((player_idx, hand_tile_types));
                } else {
                    return false;
                }
            }
            CallResponse::Daiminkan => {
                if call_state.available_calls[player_idx]
                    .iter()
                    .any(|c| matches!(c, AvailableCall::Daiminkan))
                {
                    call_state.daiminkan_declared = Some(player_idx);
                } else {
                    return false;
                }
            }
            CallResponse::Chi { hand_tile_types } => {
                // チーの組み合わせが有効か確認
                let valid = call_state.available_calls[player_idx].iter().any(|c| {
                    if let AvailableCall::Chi { options } = c {
                        options.contains(&hand_tile_types)
                    } else {
                        false
                    }
                });
                if valid {
                    call_state.chi_declared = Some((player_idx, hand_tile_types));
                } else {
                    return false;
                }
            }
            CallResponse::Pass => {
                // パスは何もしない
            }
        }

        call_state.responded[player_idx] = true;

        // 全員応答済みなら解決
        if call_state.responded.iter().all(|&r| r) {
            self.resolve_calls();
        }

        true
    }

    /// 鳴きを解決する（優先度: ロン > 大明カン > ポン > チー > パス）
    fn resolve_calls(&mut self) {
        let call_state = self.call_state.take().unwrap();

        // ロン見逃しによるフリテン判定
        // AvailableCall::Ron があったのにロン宣言しなかったプレイヤーにフリテンを設定
        for i in 0..4 {
            let had_ron = call_state.available_calls[i]
                .iter()
                .any(|c| matches!(c, AvailableCall::Ron));
            let declared_ron = call_state.ron_declared.contains(&i);

            if had_ron && !declared_ron {
                if self.players[i].is_riichi {
                    // リーチ中 → リーチ後フリテン（局終了まで永続）
                    self.players[i].is_riichi_furiten = true;
                } else {
                    // 非リーチ → 同巡フリテン（自分のツモ番で解除）
                    self.players[i].is_temporary_furiten = true;
                }
            }
        }

        // 1. ロン（最優先）
        if !call_state.ron_declared.is_empty() {
            let is_robbing_a_quad =
                matches!(call_state.resolution, CallResolution::AfterKakan { .. });
            let discarder = call_state.discarder;
            let winning_tile = call_state.discarded_tile;
            let ron_count = call_state.ron_declared.len();

            // 打順優先順（下家→対面→上家）でソート
            let mut sorted_winners = call_state.ron_declared.clone();
            sorted_winners.sort_by_key(|&p| (p + 4 - discarder) % 4);

            if ron_count >= 3 && self.settings.triple_ron_draw {
                // 三家和流局（最優先）
                self.declare_special_draw(DrawReason::TripleRon);
                return;
            }

            // 複数同時ロンが有効かつ2人以上: 全員和了
            let winners = if ron_count >= 2 && self.settings.multiple_ron {
                sorted_winners
            } else {
                // 上家取り: 最優先の1人のみ和了
                vec![sorted_winners[0]]
            };

            self.execute_ron(winners, discarder, winning_tile, is_robbing_a_quad);
            return;
        }

        if let CallResolution::AfterKakan { caller, tile_type } = call_state.resolution {
            self.execute_kakan(caller, tile_type);
            return;
        }

        // 2. 大明カン
        if let Some(caller) = call_state.daiminkan_declared {
            self.execute_daiminkan(caller, call_state.discarder, call_state.discarded_tile);
            return;
        }

        // 3. ポン
        if let Some((caller, hand_tile_types)) = call_state.pon_declared {
            self.execute_pon(
                caller,
                call_state.discarder,
                call_state.discarded_tile,
                hand_tile_types,
            );
            return;
        }

        // 4. チー
        if let Some((caller, hand_tile_types)) = call_state.chi_declared {
            self.execute_chi(
                caller,
                call_state.discarder,
                call_state.discarded_tile,
                hand_tile_types,
            );
            return;
        }

        // 5. 全員パス → 次のプレイヤーへ
        self.current_player = (call_state.discarder + 1) % 4;
        self.phase = TurnPhase::Draw;

        // 特殊流局チェック
        self.check_special_draws();
    }

    /// ロン和了を実行する（通常・ダブロン・トリロン共通）
    ///
    /// - winners: ロン和了者の打順優先順（下家→対面→上家）でソート済みのインデックスリスト
    /// - 本場ボーナスと供託棒は最初の和了者（打順最優先）のみが取得する
    fn execute_ron(
        &mut self,
        winners: Vec<usize>,
        loser: usize,
        winning_tile: Tile,
        is_robbing_a_quad: bool,
    ) {
        let is_last_tile = self.wall.is_empty();
        let dora_indicators = self.wall.dora_indicators();
        let riichi_sticks = self.riichi_sticks;
        let player_hands = self.build_player_hands();

        struct WinnerData {
            winner: usize,
            score_result: mahjong_core::scoring::score::ScoreResult,
            deltas: [i32; 4],
            uradora_indicators: Vec<Tile>,
            score_points: i32,
        }

        // 打順が最も早い和了者を rank=0 として本場・供託ボーナスの基準にする
        let mut winner_data: Vec<WinnerData> = Vec::new();

        for (rank, &winner) in winners.iter().enumerate() {
            let honba_for_this = if rank == 0 { self.honba } else { 0 };

            let win_result = scoring::check_ron_with_flags(
                &self.players[winner],
                winning_tile,
                self.prevailing_wind,
                is_last_tile,
                is_robbing_a_quad,
            );

            if !win_result.is_win {
                continue;
            }

            let mut score_result = win_result.score_result.unwrap();

            let uradora_indicators = if self.players[winner].is_riichi {
                self.wall.uradora_indicators()
            } else {
                vec![]
            };

            scoring::add_dora_to_score(
                &mut score_result,
                &self.players[winner].hand,
                Some(winning_tile),
                &dora_indicators,
                &uradora_indicators,
            );

            let winner_is_dealer = self.players[winner].is_dealer();
            let deltas = scoring::calculate_ron_score_deltas(
                winner,
                loser,
                &score_result,
                winner_is_dealer,
                honba_for_this,
            );

            // 供託棒は打順最優先の和了者（winner_data の先頭）のみ取得
            let riichi_bonus = if winner_data.is_empty() {
                (riichi_sticks as i32) * 1000
            } else {
                0
            };
            let score_points = deltas[winner] + riichi_bonus;

            winner_data.push(WinnerData {
                winner,
                score_result,
                deltas,
                uradora_indicators,
                score_points,
            });
        }

        // 安全のため: 和了成立者が0人ならフェーズを進めて返す
        if winner_data.is_empty() {
            self.current_player = (loser + 1) % 4;
            self.phase = TurnPhase::Draw;
            return;
        }

        // 全スコアデルタを合算して適用
        for wd in &winner_data {
            for i in 0..4 {
                self.players[i].score += wd.deltas[i];
            }
        }
        // 供託棒は打順最優先の和了者に付与
        if riichi_sticks > 0 {
            self.players[winner_data[0].winner].score += (riichi_sticks as i32) * 1000;
            self.riichi_sticks = 0;
        }

        if !is_robbing_a_quad {
            if let Some(last_discard) = self.players[loser].discards.last_mut() {
                last_discard.is_called = true;
            }
        }

        let scores = self.get_scores();
        let loser_wind = self.players[loser].seat_wind;

        // 各和了者にRoundWonイベントを送信
        for (idx, wd) in winner_data.iter().enumerate() {
            let winner_wind = self.players[wd.winner].seat_wind;
            let yaku_list: Vec<(String, u32)> = wd
                .score_result
                .yaku_list
                .iter()
                .map(|(name, han)| (name.to_string(), *han))
                .collect();
            let rank_name = scoring::rank_to_string(&wd.score_result.rank).to_string();
            let event_riichi_sticks = if idx == 0 { riichi_sticks } else { 0 };

            for i in 0..4 {
                self.events.push((
                    i,
                    ServerEvent::RoundWon {
                        winner: winner_wind,
                        loser: Some(loser_wind),
                        winning_tile,
                        scores,
                        yaku_list: yaku_list.clone(),
                        han: wd.score_result.han,
                        fu: wd.score_result.fu,
                        score_points: wd.score_points,
                        rank_name: rank_name.clone(),
                        uradora_indicators: wd.uradora_indicators.clone(),
                        riichi_sticks: event_riichi_sticks,
                        player_hands: player_hands.clone(),
                    },
                ));
            }
        }

        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::Ron {
            winners,
            loser,
            winning_tile,
        });
    }

    /// ポンを実行する
    fn execute_pon(
        &mut self,
        caller: usize,
        discarder: usize,
        called_tile: Tile,
        hand_tile_types: [Tile; 2],
    ) {
        let from = Player::meld_from_relative(caller, discarder);
        self.players[caller].do_pon(called_tile, hand_tile_types, from);

        // 捨て牌を「鳴かれた」としてマーク
        if let Some(last_discard) = self.players[discarder].discards.last_mut() {
            last_discard.is_called = true;
        }

        // 鳴きにより全プレイヤーの一発フラグを無効化
        for i in 0..4 {
            self.players[i].is_ippatsu = false;
            self.players[i].first_turn_interrupted = true;
        }

        // 全プレイヤーにポン通知
        let caller_wind = self.players[caller].seat_wind;
        let tiles: Vec<Tile> = self.players[caller]
            .hand
            .melds()
            .last()
            .unwrap()
            .tiles
            .to_vec();

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::PlayerCalled {
                    player: caller_wind,
                    call_type: CallType::Pon,
                    called_tile,
                    tiles: tiles.clone(),
                },
            ));
        }

        // 鳴いたプレイヤーに手牌更新を通知
        self.events.push((
            caller,
            ServerEvent::HandUpdated {
                hand: self.players[caller].hand.tiles().to_vec(),
            },
        ));

        // ポンしたプレイヤーの打牌待ちへ
        self.current_player = caller;
        self.phase = TurnPhase::WaitForDiscard;
    }

    /// 大明カンを実行する
    fn execute_daiminkan(&mut self, caller: usize, discarder: usize, called_tile: Tile) {
        let from = Player::meld_from_relative(caller, discarder);
        self.players[caller].do_daiminkan(called_tile, from);

        if let Some(last_discard) = self.players[discarder].discards.last_mut() {
            last_discard.is_called = true;
        }

        for i in 0..4 {
            self.players[i].is_ippatsu = false;
            self.players[i].first_turn_interrupted = true;
        }

        let caller_wind = self.players[caller].seat_wind;
        let open = self.players[caller].hand.melds().last().unwrap();
        let mut tiles = open.tiles.to_vec();
        tiles.push(called_tile);

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::PlayerCalled {
                    player: caller_wind,
                    call_type: CallType::Daiminkan,
                    called_tile,
                    tiles: tiles.clone(),
                },
            ));
        }

        self.events.push((
            caller,
            ServerEvent::HandUpdated {
                hand: self.players[caller].hand.tiles().to_vec(),
            },
        ));

        self.reveal_new_dora_indicator();
        self.current_player = caller;
        self.draw_after_kan(caller);
    }

    /// チーを実行する
    fn execute_chi(
        &mut self,
        caller: usize,
        discarder: usize,
        called_tile: Tile,
        hand_tile_types: [Tile; 2],
    ) {
        self.players[caller].do_chi(called_tile, hand_tile_types);

        // 捨て牌を「鳴かれた」としてマーク
        if let Some(last_discard) = self.players[discarder].discards.last_mut() {
            last_discard.is_called = true;
        }

        // 鳴きにより全プレイヤーの一発フラグを無効化
        for i in 0..4 {
            self.players[i].is_ippatsu = false;
            self.players[i].first_turn_interrupted = true;
        }

        // 全プレイヤーにチー通知
        let caller_wind = self.players[caller].seat_wind;
        let tiles: Vec<Tile> = self.players[caller]
            .hand
            .melds()
            .last()
            .unwrap()
            .tiles
            .to_vec();

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::PlayerCalled {
                    player: caller_wind,
                    call_type: CallType::Chi,
                    called_tile,
                    tiles: tiles.clone(),
                },
            ));
        }

        // 鳴いたプレイヤーに手牌更新を通知
        self.events.push((
            caller,
            ServerEvent::HandUpdated {
                hand: self.players[caller].hand.tiles().to_vec(),
            },
        ));

        // チーしたプレイヤーの打牌待ちへ
        self.current_player = caller;
        self.phase = TurnPhase::WaitForDiscard;
    }

    fn execute_kakan(&mut self, caller: usize, tile_type: TileType) {
        self.players[caller].do_kakan(tile_type);
        for i in 0..4 {
            self.players[i].is_ippatsu = false;
            self.players[i].first_turn_interrupted = true;
        }

        let caller_wind = self.players[caller].seat_wind;
        let open = self.players[caller]
            .hand
            .melds()
            .iter()
            .rev()
            .find(|open| {
                open.category == mahjong_core::hand_info::meld::MeldType::Kakan
                    && open.tiles[0].get() == tile_type
            })
            .unwrap();
        let mut tiles = open.tiles.clone();
        if tiles.len() == 3 {
            tiles.push(Tile::new(tile_type));
        }

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::PlayerCalled {
                    player: caller_wind,
                    call_type: CallType::Kakan,
                    called_tile: Tile::new(tile_type),
                    tiles: tiles.clone(),
                },
            ));
        }

        self.events.push((
            caller,
            ServerEvent::HandUpdated {
                hand: self.players[caller].hand.tiles().to_vec(),
            },
        ));

        self.reveal_new_dora_indicator();
        self.draw_after_kan(caller);
    }

    fn check_kakan_ron_and_resolve(&mut self, caller: usize, tile_type: TileType) {
        let called_tile = Tile::new(tile_type);
        let is_last_tile = self.wall.is_empty();
        let mut available_calls: [Vec<AvailableCall>; 4] =
            [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let mut responded = [true; 4];

        for i in 0..4 {
            if i == caller {
                continue;
            }

            let player = &self.players[i];
            if !player.is_furiten() {
                let win_result = scoring::check_ron_with_flags(
                    player,
                    called_tile,
                    self.prevailing_wind,
                    is_last_tile,
                    true,
                );
                if win_result.is_win {
                    available_calls[i].push(AvailableCall::Ron);
                    responded[i] = false;
                }
            }
        }

        let has_any_calls = available_calls.iter().any(|calls| !calls.is_empty());
        if has_any_calls {
            self.phase = TurnPhase::WaitForCalls;
            let caller_wind = self.players[caller].seat_wind;
            for i in 0..4 {
                if !available_calls[i].is_empty() {
                    self.events.push((
                        i,
                        ServerEvent::CallAvailable {
                            tile: called_tile,
                            discarder: caller_wind,
                            calls: available_calls[i].clone(),
                        },
                    ));
                }
            }

            self.call_state = Some(CallState {
                discarded_tile: called_tile,
                discarder: caller,
                available_calls,
                responded,
                ron_declared: Vec::new(),
                pon_declared: None,
                daiminkan_declared: None,
                chi_declared: None,
                resolution: CallResolution::AfterKakan { caller, tile_type },
            });
        } else {
            self.execute_kakan(caller, tile_type);
        }
    }

    /// 暗カン/加カンを実行する
    pub fn do_kan(&mut self, tile_type: TileType) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }

        let player_idx = self.current_player;
        if self.players[player_idx].is_riichi {
            return false;
        }

        // 場全体で4回カン済みなら追加のカン不可
        if self.total_kan_count() >= 4 {
            return false;
        }

        if self.players[player_idx]
            .ankan_options()
            .contains(&tile_type)
        {
            self.players[player_idx].do_ankan(tile_type);
        } else if self.players[player_idx]
            .kakan_options()
            .contains(&tile_type)
        {
            self.check_kakan_ron_and_resolve(player_idx, tile_type);
            return true;
        } else {
            return false;
        }
        for i in 0..4 {
            self.players[i].is_ippatsu = false;
            self.players[i].first_turn_interrupted = true;
        }

        let caller_wind = self.players[player_idx].seat_wind;
        let open = self.players[player_idx].hand.melds().last().unwrap();
        let mut tiles = open.tiles.to_vec();
        tiles.push(open.tiles[0]);
        let called_tile = Tile::new(tile_type);

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::PlayerCalled {
                    player: caller_wind,
                    call_type: CallType::Ankan,
                    called_tile,
                    tiles: tiles.clone(),
                },
            ));
        }

        self.events.push((
            player_idx,
            ServerEvent::HandUpdated {
                hand: self.players[player_idx].hand.tiles().to_vec(),
            },
        ));

        self.reveal_new_dora_indicator();
        self.draw_after_kan(player_idx);
        true
    }

    fn reveal_new_dora_indicator(&mut self) {
        self.wall.add_dora_indicator();
        let dora_indicators = self.wall.dora_indicators();
        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::DoraIndicatorsUpdated {
                    dora_indicators: dora_indicators.clone(),
                },
            ));
        }
    }

    fn draw_after_kan(&mut self, player_idx: usize) {
        // 四槓散了チェック: 4回目のカン直後に判定（設定がありの場合のみ）
        if self.settings.four_kans_draw && self.check_four_kans_draw() {
            self.declare_special_draw(DrawReason::FourKans);
            return;
        }

        // 同巡フリテンを解除（嶺上ツモも自分のツモ番）
        self.players[player_idx].is_temporary_furiten = false;

        let Some(tile) = self.wall.draw_rinshan() else {
            self.do_exhaustive_draw();
            return;
        };

        self.current_player = player_idx;
        self.phase = TurnPhase::WaitForDiscard;
        self.last_draw_was_dead_wall = true;
        self.players[player_idx].draw(tile);

        let remaining = self.wall.remaining();
        let can_tsumo = self.can_tsumo();
        let can_riichi = self.can_player_riichi(player_idx);
        #[cfg(debug_assertions)]
        self.log_draw_diagnostics(player_idx, "kan_draw", can_tsumo, can_riichi);

        let is_furiten = self.players[player_idx].is_furiten();
        self.events.push((
            player_idx,
            ServerEvent::TileDrawn {
                tile,
                remaining_tiles: remaining,
                can_tsumo,
                can_riichi,
                is_furiten,
            },
        ));

        let current_wind = self.players[player_idx].seat_wind;
        for i in 0..4 {
            if i != player_idx {
                self.events.push((
                    i,
                    ServerEvent::OtherPlayerDrew {
                        player: current_wind,
                        remaining_tiles: remaining,
                    },
                ));
            }
        }
    }

    fn can_player_riichi_with_discard(&self, player_idx: usize, tile: Option<Tile>) -> bool {
        let player = &self.players[player_idx];
        let mut hand = player.hand.clone();

        match tile {
            Some(target) => {
                let drawn = hand.drawn();
                let tiles = hand.tiles_mut();
                let Some(idx) = tiles.iter().position(|t| *t == target) else {
                    return false;
                };
                tiles.remove(idx);
                if let Some(drawn_tile) = drawn {
                    tiles.push(drawn_tile);
                    tiles.sort();
                }
                hand.set_drawn(None);
            }
            None => {
                if hand.drawn().is_none() {
                    return false;
                }
                hand.set_drawn(None);
            }
        }

        hand_analyzer::calc_shanten_number(&hand).is_ready()
    }

    /// プレイヤーがリーチ宣言可能か判定する
    ///
    /// 条件:
    /// - 門前（鳴いていない）
    /// - 持ち点が1000点以上
    /// - まだリーチしていない
    /// - 山に1枚以上残っている（打牌後に少なくとも1回はツモが行われる）
    /// - 14枚の手牌から、聴牌を維持する打牌が1つ以上ある
    fn can_player_riichi(&self, player_idx: usize) -> bool {
        let player = &self.players[player_idx];

        if player.is_riichi {
            if player_idx == 0 {
                eprintln!(
                    "[riichi-reject] reason=already_riichi player={}",
                    player_idx
                );
            }
            return false;
        }
        if !player.is_menzen() {
            if player_idx == 0 {
                eprintln!("[riichi-reject] reason=not_menzen player={}", player_idx);
            }
            return false;
        }
        if player.score < 1000 {
            if player_idx == 0 {
                eprintln!(
                    "[riichi-reject] reason=score_too_low player={} score={}",
                    player_idx, player.score
                );
            }
            return false;
        }
        if self.wall.remaining() < 1 {
            if player_idx == 0 {
                eprintln!(
                    "[riichi-reject] reason=wall_empty player={} remaining={}",
                    player_idx,
                    self.wall.remaining()
                );
            }
            return false;
        }
        if player.hand.drawn().is_none() {
            if player_idx == 0 {
                eprintln!("[riichi-reject] reason=no_drawn player={}", player_idx);
            }
            return false;
        }

        if self.can_player_riichi_with_discard(player_idx, None) {
            return true;
        }

        player
            .hand
            .tiles()
            .iter()
            .copied()
            .any(|tile| self.can_player_riichi_with_discard(player_idx, Some(tile)))
    }

    /// リーチ宣言を実行する
    ///
    /// リーチ宣言 + 打牌を同時に行う。
    /// tile で指定した牌を捨てた後、手牌が聴牌であることを確認する。
    /// tile が None の場合はツモ切りリーチ。
    pub fn do_riichi(&mut self, tile: Option<Tile>) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }

        let player_idx = self.current_player;

        // リーチ条件チェック
        if !self.can_player_riichi(player_idx) {
            return false;
        }
        if !self.can_player_riichi_with_discard(player_idx, tile) {
            return false;
        }

        // ダブルリーチ判定（第一ツモかつ副露による中断なし）
        let is_double = self.players[player_idx].is_first_turn
            && !self.players[player_idx].first_turn_interrupted;

        // リーチ宣言
        self.players[player_idx].declare_riichi(is_double);
        self.riichi_sticks += 1;

        // 全プレイヤーにリーチ通知
        let player_wind = self.players[player_idx].seat_wind;
        let scores = self.get_scores();
        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::PlayerRiichi {
                    player: player_wind,
                    scores,
                    riichi_sticks: self.riichi_sticks,
                },
            ));
        }

        // リーチ宣言牌を打牌
        // （declare_riichi内でippatsu=trueが設定されるが、
        //   直後のdiscardでippatsu=falseにされてしまう。
        //   これを防ぐため、一時的にippatsuを保護する）
        let is_tsumogiri = tile.is_none();
        let discarded = self.players[player_idx].discard(tile);
        // リーチ宣言直後の打牌なのでippatsuを復元
        self.players[player_idx].is_ippatsu = true;

        // 打牌をリーチ宣言牌としてマーク
        if let Some(last_discard) = self.players[player_idx].discards.last_mut() {
            last_discard.is_riichi_declaration = true;
        }

        let current_wind = self.players[player_idx].seat_wind;
        let discarder = player_idx;

        // 全プレイヤーに打牌を通知
        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::TileDiscarded {
                    player: current_wind,
                    tile: discarded,
                    is_tsumogiri,
                },
            ));
        }

        // 鳴き候補をチェック
        let call_state = self.check_available_calls(discarded, discarder);
        let has_any_calls = call_state.available_calls.iter().any(|c| !c.is_empty());

        if has_any_calls {
            self.phase = TurnPhase::WaitForCalls;

            let discarder_wind = self.players[discarder].seat_wind;
            for i in 0..4 {
                if !call_state.available_calls[i].is_empty() {
                    self.events.push((
                        i,
                        ServerEvent::CallAvailable {
                            tile: discarded,
                            discarder: discarder_wind,
                            calls: call_state.available_calls[i].clone(),
                        },
                    ));
                }
            }

            self.call_state = Some(call_state);
        } else {
            self.current_player = (discarder + 1) % 4;
            self.phase = TurnPhase::Draw;

            // 特殊流局チェック（四家立直チェック含む）
            self.check_special_draws();
        }

        true
    }

    /// 現在のプレイヤーがツモ和了できるか判定する
    pub fn can_tsumo(&self) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }
        let player = &self.players[self.current_player];
        let is_last_tile = self.wall.is_empty();
        let result = scoring::check_win(
            player,
            self.prevailing_wind,
            true,
            is_last_tile,
            self.last_draw_was_dead_wall,
        );
        result.is_win
    }

    /// ツモ和了を実行する
    /// 点数移動を行い、局を終了させる
    pub fn do_tsumo(&mut self) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }

        let player = &self.players[self.current_player];
        let is_last_tile = self.wall.is_empty();
        let win_result = scoring::check_win(
            player,
            self.prevailing_wind,
            true,
            is_last_tile,
            self.last_draw_was_dead_wall,
        );

        if !win_result.is_win {
            return false;
        }

        let mut score_result = win_result.score_result.unwrap();
        let winner = self.current_player;
        let winning_tile = self.players[winner]
            .hand
            .drawn()
            .expect("ツモ和了時にはdrawnが必要");
        let winner_is_dealer = self.players[winner].is_dealer();

        // ドラ・赤ドラ・裏ドラを加算
        let dora_indicators = self.wall.dora_indicators();
        let uradora_indicators = if self.players[winner].is_riichi {
            self.wall.uradora_indicators()
        } else {
            vec![]
        };
        scoring::add_dora_to_score(
            &mut score_result,
            &self.players[winner].hand,
            None,
            &dora_indicators,
            &uradora_indicators,
        );

        // 点数移動を計算
        let deltas = scoring::calculate_tsumo_score_deltas(
            winner,
            &score_result,
            winner_is_dealer,
            self.dealer,
            self.honba,
        );
        let riichi_sticks = self.riichi_sticks;

        // 点数を適用
        for i in 0..4 {
            self.players[i].score += deltas[i];
        }
        if riichi_sticks > 0 {
            self.players[winner].score += (riichi_sticks as i32) * 1000;
            self.riichi_sticks = 0;
        }

        let scores = self.get_scores();
        let winner_wind = self.players[winner].seat_wind;

        // 役情報を構築
        let yaku_list: Vec<(String, u32)> = score_result
            .yaku_list
            .iter()
            .map(|(name, han)| (name.to_string(), *han))
            .collect();
        let rank_name = scoring::rank_to_string(&score_result.rank).to_string();
        let player_hands = self.build_player_hands();

        // 全プレイヤーに和了イベントを送信
        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::RoundWon {
                    winner: winner_wind,
                    loser: None, // ツモなのでloserなし
                    winning_tile,
                    scores,
                    yaku_list: yaku_list.clone(),
                    han: score_result.han,
                    fu: score_result.fu,
                    score_points: deltas[winner] + (riichi_sticks as i32) * 1000,
                    rank_name: rank_name.clone(),
                    uradora_indicators: uradora_indicators.clone(),
                    riichi_sticks,
                    player_hands: player_hands.clone(),
                },
            ));
        }

        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::Tsumo {
            winner,
            winning_tile,
        });

        true
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

    /// WaitForCalls フェーズでCPUプレイヤーを全員パスさせる
    ///
    /// human_player 以外のプレイヤーで鳴き候補がある者を自動パスさせる。
    /// 全員パスしたらフェーズが自動進行する。
    pub fn auto_pass_cpu(&mut self, human_player: usize) {
        if self.phase != TurnPhase::WaitForCalls {
            return;
        }

        for i in 0..4 {
            if i == human_player {
                continue;
            }
            // まだ応答していないCPUプレイヤーをパスさせる
            if let Some(ref call_state) = self.call_state {
                if !call_state.responded[i] {
                    self.respond_to_call(i, CallResponse::Pass);
                    // resolve_calls で call_state が消えたら終了
                    if self.call_state.is_none() {
                        return;
                    }
                }
            }
        }
    }

    /// 局を最後まで自動進行する（全員ツモ切り・鳴きなし）
    /// テスト・デバッグ用
    pub fn play_to_end(&mut self) {
        while self.phase != TurnPhase::RoundOver {
            match self.phase {
                TurnPhase::Draw => {
                    self.do_draw();
                }
                TurnPhase::WaitForDiscard => {
                    self.do_discard(None);
                }
                TurnPhase::WaitForCalls => {
                    // 全員パス
                    for i in 0..4 {
                        if let Some(ref cs) = self.call_state {
                            if !cs.responded[i] {
                                self.respond_to_call(i, CallResponse::Pass);
                                if self.call_state.is_none() {
                                    break;
                                }
                            }
                        }
                    }
                }
                TurnPhase::WaitForNineTerminals => {
                    // テスト用: 常に流局宣言する
                    self.do_nine_terminals(self.current_player, true);
                }
                TurnPhase::RoundOver => break,
            }
        }
    }

    /// 局が終了したかどうか
    pub fn is_over(&self) -> bool {
        self.phase == TurnPhase::RoundOver
    }

    /// 荒牌流局を処理する（ノーテン罰符を含む）
    fn do_exhaustive_draw(&mut self) {
        // テンパイ判定
        let mut tenpai_players = Vec::new();
        let mut noten_players = Vec::new();

        for i in 0..4 {
            if scoring::is_ready(&self.players[i]) {
                tenpai_players.push(i);
            } else {
                noten_players.push(i);
            }
        }

        // ノーテン罰符の計算（テンパイ者と非テンパイ者がいる場合のみ）
        if !tenpai_players.is_empty() && !noten_players.is_empty() {
            let total_penalty = 3000i32;
            let tenpai_count = tenpai_players.len() as i32;
            let noten_count = noten_players.len() as i32;

            let gain_each = total_penalty / tenpai_count;
            let loss_each = total_penalty / noten_count;

            for &i in &tenpai_players {
                self.players[i].score += gain_each;
            }
            for &i in &noten_players {
                self.players[i].score -= loss_each;
            }
        }

        let scores = self.get_scores();
        let tenpai_winds: Vec<Wind> = tenpai_players
            .iter()
            .map(|&i| self.players[i].seat_wind)
            .collect();

        let dealer_tenpai = tenpai_players.contains(&self.dealer);
        let player_hands = self.build_player_hands();

        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::ExhaustiveDraw { dealer_tenpai });

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::RoundDraw {
                    scores,
                    reason: DrawReason::Exhaustive,
                    tenpai: tenpai_winds.clone(),
                    riichi_sticks: self.riichi_sticks,
                    player_hands: player_hands.clone(),
                },
            ));
        }
    }

    /// 特殊流局をチェックする（四風連打、四家立直）
    fn check_special_draws(&mut self) {
        // 四風連打チェック: 全員が1枚ずつ捨てて、全て同じ風牌
        if self.settings.four_winds_draw && self.check_four_winds_draw() {
            self.declare_special_draw(DrawReason::FourWinds);
            return;
        }

        // 四家立直チェック: 全員がリーチ宣言済み
        if self.settings.four_riichi_draw && self.check_four_riichi_draw() {
            self.declare_special_draw(DrawReason::FourRiichi);
        }
    }

    /// 四風連打を判定する
    ///
    /// 条件: 各プレイヤーがちょうど1枚ずつ捨てており、
    /// 全て同じ風牌で、鳴きが発生していない
    fn check_four_winds_draw(&self) -> bool {
        // 全プレイヤーがちょうど1枚捨てていること
        for player in &self.players {
            if player.discards.len() != 1 {
                return false;
            }
            // 鳴かれていたら不成立
            if player.discards[0].is_called {
                return false;
            }
        }

        // 全て同じ風牌であること
        let first_tile = self.players[0].discards[0].tile;
        if !first_tile.is_wind() {
            return false;
        }

        self.players
            .iter()
            .all(|p| p.discards[0].tile.get() == first_tile.get())
    }

    /// 四家立直を判定する
    ///
    /// 条件: 全4プレイヤーがリーチ宣言済み
    fn check_four_riichi_draw(&self) -> bool {
        self.players.iter().all(|p| p.is_riichi)
    }

    /// 九種九牌の宣言条件を判定する
    ///
    /// 条件: 現在のプレイヤーが一度も捨牌しておらず、
    /// 手牌＋ツモ牌に9種類以上のヤオ九牌（老頭牌・字牌）がある
    fn check_nine_terminals(&self) -> bool {
        let player = &self.players[self.current_player];
        // 初回ツモのみ（捨牌済みなら宣言不可）
        if !player.discards.is_empty() {
            return false;
        }
        let mut tile_types = std::collections::HashSet::new();
        for tile in player.hand.tiles() {
            if tile.is_1_9_honor() {
                tile_types.insert(tile.get());
            }
        }
        if let Some(tile) = player.hand.drawn() {
            if tile.is_1_9_honor() {
                tile_types.insert(tile.get());
            }
        }
        tile_types.len() >= 9
    }

    /// 九種九牌の宣言を処理する
    ///
    /// - `declare=true`: 流局を宣言する
    /// - `declare=false`: 続行する（通常の打牌フェーズへ移行）
    pub fn do_nine_terminals(&mut self, player_idx: usize, declare: bool) -> bool {
        if self.phase != TurnPhase::WaitForNineTerminals {
            return false;
        }
        if self.current_player != player_idx {
            return false;
        }
        if declare {
            self.declare_special_draw(DrawReason::NineTerminals);
        } else {
            self.phase = TurnPhase::WaitForDiscard;
        }
        true
    }

    /// 場全体のカン回数を返す
    fn total_kan_count(&self) -> usize {
        self.players.iter().map(|p| p.kan_count()).sum()
    }

    /// 四槓散了を判定する
    ///
    /// 条件: 場全体で4回カンが成立し、かつ2人以上がカンしている
    /// （1人が4回カンした場合は四槓子の可能性があるため続行）
    fn check_four_kans_draw(&self) -> bool {
        if self.total_kan_count() < 4 {
            return false;
        }
        let players_with_kan = self.players.iter().filter(|p| p.kan_count() > 0).count();
        players_with_kan >= 2
    }

    /// 特殊流局を宣言する
    fn declare_special_draw(&mut self, reason: DrawReason) {
        let scores = self.get_scores();
        let player_hands = self.build_player_hands();
        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::SpecialDraw);

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::RoundDraw {
                    scores,
                    reason: reason.clone(),
                    tenpai: Vec::new(),
                    riichi_sticks: self.riichi_sticks,
                    player_hands: player_hands.clone(),
                },
            ));
        }
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
mod tests {
    use super::*;

    #[test]
    fn test_round_new() {
        let round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        assert_eq!(round.prevailing_wind, Wind::East);
        assert_eq!(round.current_player, 0);
        assert_eq!(round.phase, TurnPhase::Draw);
        assert!(round.result.is_none());

        // 各プレイヤーに13枚配られている
        for i in 0..4 {
            assert_eq!(round.players[i].hand.tiles().len(), 13);
        }

        // 親（プレイヤー0）が東家
        assert_eq!(round.players[0].seat_wind, Wind::East);
    }

    #[test]
    fn test_round_draw() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        round.drain_events(); // 初期イベントをクリア

        assert!(round.do_draw());
        assert_eq!(round.phase, TurnPhase::WaitForDiscard);
        assert!(round.players[0].hand.drawn().is_some());

        // イベントを確認: 1つのTileDrawn + 3つのOtherPlayerDrew = 4イベント
        let events = round.drain_events();
        assert_eq!(events.len(), 4);
    }

    #[test]
    fn test_round_discard() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        round.drain_events();
        round.do_draw();
        round.drain_events();

        // ツモ切り
        assert!(round.do_discard(None));

        // 打牌後のフェーズは Draw か WaitForCalls
        assert!(
            round.phase == TurnPhase::Draw || round.phase == TurnPhase::WaitForCalls,
            "phase should be Draw or WaitForCalls, got: {:?}",
            round.phase
        );

        // 鳴き待ちなら全員パスして進める
        if round.phase == TurnPhase::WaitForCalls {
            for i in 0..4 {
                if let Some(ref cs) = round.call_state {
                    if !cs.responded[i] {
                        round.respond_to_call(i, CallResponse::Pass);
                        if round.call_state.is_none() {
                            break;
                        }
                    }
                }
            }
        }

        assert_eq!(round.phase, TurnPhase::Draw);
        assert_eq!(round.current_player, 1); // 次のプレイヤーへ
    }

    #[test]
    fn test_round_turn_flow() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        round.drain_events();

        // 4人分のターンを回す
        for expected_player in 0..4 {
            assert_eq!(round.current_player, expected_player);

            // draw
            round.do_draw();
            if round.phase == TurnPhase::RoundOver {
                break;
            }

            // discard
            round.do_discard(None);

            // WaitForCalls なら全員パス
            if round.phase == TurnPhase::WaitForCalls {
                for i in 0..4 {
                    if let Some(ref cs) = round.call_state {
                        if !cs.responded[i] {
                            round.respond_to_call(i, CallResponse::Pass);
                            if round.call_state.is_none() {
                                break;
                            }
                        }
                    }
                }
            }
        }

        if round.phase != TurnPhase::RoundOver {
            // 一巡して最初のプレイヤーに戻る
            assert_eq!(round.current_player, 0);
        }
    }

    #[test]
    fn test_round_play_to_end() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        round.play_to_end();

        assert!(round.is_over());
        assert!(round.result.is_some());
    }

    #[test]
    fn test_round_scores() {
        let round = Round::new(
            Wind::East,
            0,
            [25000, 30000, 20000, 25000],
            0,
            0,
            0,
            Settings::new(),
        );
        let scores = round.get_scores();
        assert_eq!(scores, [25000, 30000, 20000, 25000]);
    }

    #[test]
    fn test_round_events_on_start() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        let events = round.drain_events();

        // 4人分のGameStartedイベント
        assert_eq!(events.len(), 4);
        for (i, (player_idx, event)) in events.iter().enumerate() {
            assert_eq!(*player_idx, i);
            match event {
                ServerEvent::GameStarted {
                    seat_wind,
                    hand,
                    scores,
                    prevailing_wind,
                    ..
                } => {
                    assert_eq!(hand.len(), 13);
                    assert_eq!(*scores, [25000; 4]);
                    assert_eq!(*prevailing_wind, Wind::East);
                    assert_eq!(*seat_wind, round.players[i].seat_wind);
                }
                _ => panic!("Expected GameStarted event"),
            }
        }
    }

    #[test]
    fn test_wait_for_calls_and_pass() {
        // 打牌後に WaitForCalls になった場合、全員パスで Draw に進む
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        round.drain_events();
        round.do_draw();
        round.drain_events();
        round.do_discard(None);

        if round.phase == TurnPhase::WaitForCalls {
            // 全員パス
            for i in 0..4 {
                if let Some(ref cs) = round.call_state {
                    if !cs.responded[i] {
                        assert!(round.respond_to_call(i, CallResponse::Pass));
                        if round.call_state.is_none() {
                            break;
                        }
                    }
                }
            }
            assert_eq!(round.phase, TurnPhase::Draw);
        }
    }

    #[test]
    fn test_check_available_calls_offers_pon_but_not_ron_for_5z() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        let seat_wind = round.players[1].seat_wind;
        let hand = mahjong_core::hand::Hand::from("234678m56p567s55z");
        round.players[1] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);

        let call_state = round.check_available_calls(Tile::new(Tile::Z5), 0);
        assert!(
            call_state.available_calls[1]
                .iter()
                .any(|call| matches!(call, AvailableCall::Pon { .. }))
        );
        assert!(
            !call_state.available_calls[1]
                .iter()
                .any(|call| matches!(call, AvailableCall::Ron))
        );
    }

    #[test]
    fn test_do_riichi_requires_tenpai_after_discard() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        let seat_wind = round.players[0].seat_wind;
        let hand = mahjong_core::hand::Hand::from("123m123p123s45z67m 8m");
        round.players[0] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
        round.players[0].draw(hand.drawn().unwrap());
        round.phase = TurnPhase::WaitForDiscard;
        round.current_player = 0;
        round.drain_events();

        assert!(!round.do_riichi(None));
        assert!(!round.players[0].is_riichi);
        assert_eq!(round.players[0].hand.drawn(), Some(Tile::new(Tile::M8)));

        assert!(round.do_riichi(Some(Tile::new(Tile::Z4))));
        assert!(round.players[0].is_riichi);
    }

    #[test]
    fn test_do_riichi_deducts_score_and_adds_stick() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        let seat_wind = round.players[0].seat_wind;
        let hand = mahjong_core::hand::Hand::from("123m123p123s45z67m 8m");
        round.players[0] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
        round.players[0].draw(hand.drawn().unwrap());
        round.phase = TurnPhase::WaitForDiscard;
        round.current_player = 0;
        round.drain_events();

        assert!(round.do_riichi(Some(Tile::new(Tile::Z4))));
        assert_eq!(round.players[0].score, 24000);
        assert_eq!(round.riichi_sticks, 1);
    }

    #[test]
    fn test_check_available_calls_offers_daiminkan() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        let seat_wind = round.players[1].seat_wind;
        let hand = mahjong_core::hand::Hand::from("111m234p567s789m");
        round.players[1] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);

        let call_state = round.check_available_calls(Tile::new(Tile::M1), 0);
        assert!(
            call_state.available_calls[1]
                .iter()
                .any(|call| matches!(call, AvailableCall::Daiminkan))
        );
    }

    #[test]
    fn test_do_ankan_draws_rinshan_and_reveals_dora() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        let seat_wind = round.players[0].seat_wind;
        let hand = mahjong_core::hand::Hand::from("111m234p567s789m 1m");
        round.players[0] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
        round.players[0].draw(hand.drawn().unwrap());
        round.current_player = 0;
        round.phase = TurnPhase::WaitForDiscard;
        round.drain_events();

        assert!(round.do_kan(Tile::M1));
        assert_eq!(round.phase, TurnPhase::WaitForDiscard);
        assert!(round.players[0].hand.drawn().is_some());
        assert_eq!(round.players[0].hand.melds().len(), 1);
        assert_eq!(round.wall.dora_indicators().len(), 2);
    }

    #[test]
    fn test_do_kakan_draws_rinshan_and_reveals_dora() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        let seat_wind = round.players[0].seat_wind;
        let mut player = Player::new(seat_wind, vec![], 25000);
        player.hand = mahjong_core::hand::Hand::from("234p567s789m1z 111m 1m");
        round.players[0] = player;
        round.current_player = 0;
        round.phase = TurnPhase::WaitForDiscard;
        round.drain_events();

        assert!(round.do_kan(Tile::M1));
        assert_eq!(round.phase, TurnPhase::WaitForDiscard);
        assert!(round.players[0].hand.drawn().is_some());
        assert_eq!(
            round.players[0].hand.melds()[0].category,
            mahjong_core::hand_info::meld::MeldType::Kakan
        );
        assert_eq!(round.wall.dora_indicators().len(), 2);
    }

    #[test]
    fn test_do_kakan_keeps_unrelated_drawn_tile_in_hand() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        let seat_wind = round.players[0].seat_wind;
        let mut player = Player::new(seat_wind, vec![], 25000);
        player.hand = mahjong_core::hand::Hand::from("127m234p567s1z 111m 9s");
        round.players[0] = player;
        round.current_player = 0;
        round.phase = TurnPhase::WaitForDiscard;
        round.drain_events();

        assert!(round.do_kan(Tile::M1));
        assert_eq!(round.phase, TurnPhase::WaitForDiscard);
        assert!(round.players[0].hand.drawn().is_some());
        assert_eq!(round.players[0].hand.tiles().len(), 10);
        assert!(
            round.players[0]
                .hand
                .tiles()
                .contains(&mahjong_core::tile::Tile::new(Tile::S9))
        );
    }

    #[test]
    fn test_temporary_furiten_set_on_ron_pass() {
        // プレイヤー1がロン可能な状態で、パスすると同巡フリテンが設定される
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());

        // プレイヤー1にテンパイ手を設定: 123m456p789s11z 待ち1z（場風東）
        let seat1 = round.players[1].seat_wind;
        let hand1 = mahjong_core::hand::Hand::from("123m456p789s1122z");
        round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);

        // プレイヤー0が1z（東）を捨てた場合をチェック
        let call_state = round.check_available_calls(Tile::new(Tile::Z1), 0);

        // ロンが可能であること
        assert!(
            call_state.available_calls[1]
                .iter()
                .any(|c| matches!(c, AvailableCall::Ron)),
            "player 1 should be able to ron"
        );

        // CallStateをセットしてパスで応答
        round.phase = TurnPhase::WaitForCalls;
        round.call_state = Some(call_state);
        for i in 0..4 {
            if let Some(ref cs) = round.call_state {
                if !cs.responded[i] {
                    round.respond_to_call(i, CallResponse::Pass);
                    if round.call_state.is_none() {
                        break;
                    }
                }
            }
        }

        // 同巡フリテンが設定されていること
        assert!(round.players[1].is_temporary_furiten);
        assert!(!round.players[1].is_riichi_furiten);
    }

    #[test]
    fn test_temporary_furiten_cleared_on_draw() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        round.drain_events();

        // プレイヤー1に同巡フリテンを設定
        round.players[1].is_temporary_furiten = true;

        // プレイヤー1のツモ番にする
        round.current_player = 1;
        round.phase = TurnPhase::Draw;
        round.do_draw();

        // 同巡フリテンが解除されていること
        assert!(!round.players[1].is_temporary_furiten);
    }

    #[test]
    fn test_riichi_furiten_set_on_ron_pass() {
        // リーチ中のプレイヤーがロンを見逃すとリーチ後フリテンが設定される
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());

        let seat1 = round.players[1].seat_wind;
        let hand1 = mahjong_core::hand::Hand::from("123m456p789s1122z");
        round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);
        round.players[1].is_riichi = true;

        let call_state = round.check_available_calls(Tile::new(Tile::Z1), 0);
        assert!(
            call_state.available_calls[1]
                .iter()
                .any(|c| matches!(c, AvailableCall::Ron)),
            "riichi player should be able to ron"
        );

        round.phase = TurnPhase::WaitForCalls;
        round.call_state = Some(call_state);
        for i in 0..4 {
            if let Some(ref cs) = round.call_state {
                if !cs.responded[i] {
                    round.respond_to_call(i, CallResponse::Pass);
                    if round.call_state.is_none() {
                        break;
                    }
                }
            }
        }

        // リーチ後フリテンが設定されていること
        assert!(round.players[1].is_riichi_furiten);
        assert!(!round.players[1].is_temporary_furiten);
    }

    #[test]
    fn test_riichi_furiten_persists_after_draw() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        round.drain_events();

        // リーチ後フリテンを設定
        round.players[1].is_riichi_furiten = true;
        round.players[1].is_riichi = true;

        // プレイヤー1がツモ
        round.current_player = 1;
        round.phase = TurnPhase::Draw;
        round.do_draw();

        // リーチ後フリテンは解除されないこと
        assert!(round.players[1].is_riichi_furiten);
    }

    #[test]
    fn test_temporary_furiten_blocks_ron() {
        // 同巡フリテンのプレイヤーにはロンが提供されない
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());

        let seat1 = round.players[1].seat_wind;
        let hand1 = mahjong_core::hand::Hand::from("123m456p789s1122z");
        round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);
        round.players[1].is_temporary_furiten = true;

        let call_state = round.check_available_calls(Tile::new(Tile::Z1), 0);

        // フリテンなのでロンが提供されないこと
        assert!(
            !call_state.available_calls[1]
                .iter()
                .any(|c| matches!(c, AvailableCall::Ron)),
            "furiten player should not be offered ron"
        );
    }

    #[test]
    fn test_kakan_ron_pass_sets_furiten() {
        // 加カンで搶槓可能だがパスした場合、フリテンが設定される
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());

        let seat0 = round.players[0].seat_wind;
        let mut player0 = Player::new(seat0, vec![], 25000);
        player0.hand = mahjong_core::hand::Hand::from("234p567s789m1z 111m 1m");
        round.players[0] = player0;

        let seat1 = round.players[1].seat_wind;
        let hand1 = mahjong_core::hand::Hand::from("11m234p567p789s55z");
        round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);

        round.current_player = 0;
        round.phase = TurnPhase::WaitForDiscard;
        round.drain_events();

        assert!(round.do_kan(Tile::M1));
        assert_eq!(round.phase, TurnPhase::WaitForCalls);
        let call_state = round.call_state.as_ref().unwrap();
        assert!(
            call_state.available_calls[1]
                .iter()
                .any(|call| matches!(call, AvailableCall::Ron))
        );

        // ロンせずパス → フリテンが設定されること
        assert!(round.respond_to_call(1, CallResponse::Pass));
        assert!(round.players[1].is_temporary_furiten);
    }

    #[test]
    fn test_riichi_with_specific_tenpai_hand() {
        // 再現テスト: 6m7m1p2p3p3p4p5p5p6p7s8s9s ツモ8m
        // shanten=0 で riichi_discards がある（3p,3p,5p,5p,6p）
        // → can_riichi = true であるべき
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());

        let seat0 = round.players[0].seat_wind;
        let hand = mahjong_core::hand::Hand::from("67m12334556p789s");
        round.players[0] = Player::new(seat0, hand.tiles().to_vec(), 25000);
        round.players[0].hand.set_drawn(Some(Tile::new(Tile::M8)));

        // 前提条件チェック
        assert!(!round.players[0].is_riichi, "should not be in riichi");
        assert!(round.players[0].is_menzen(), "should be menzen");
        assert!(round.players[0].score >= 1000, "should have >= 1000 score");
        assert!(round.wall.remaining() >= 1, "wall should have tiles");
        assert!(
            round.players[0].hand.drawn().is_some(),
            "should have drawn tile"
        );

        // リーチ可能であるべき
        assert!(
            round.can_player_riichi(0),
            "should be able to declare riichi with tenpai hand"
        );
    }

    #[test]
    fn test_kakan_offers_rob_ron() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());

        let seat0 = round.players[0].seat_wind;
        let mut player0 = Player::new(seat0, vec![], 25000);
        player0.hand = mahjong_core::hand::Hand::from("234p567s789m1z 111m 1m");
        round.players[0] = player0;

        let seat1 = round.players[1].seat_wind;
        let hand1 = mahjong_core::hand::Hand::from("11m234p567p789s55z");
        round.players[1] = Player::new(seat1, hand1.tiles().to_vec(), 25000);

        round.current_player = 0;
        round.phase = TurnPhase::WaitForDiscard;
        round.drain_events();

        assert!(round.do_kan(Tile::M1));
        assert_eq!(round.phase, TurnPhase::WaitForCalls);
        let call_state = round.call_state.as_ref().unwrap();
        assert!(
            call_state.available_calls[1]
                .iter()
                .any(|call| matches!(call, AvailableCall::Ron))
        );

        assert!(round.respond_to_call(1, CallResponse::Ron));
        assert_eq!(round.phase, TurnPhase::RoundOver);
        match round.result {
            Some(RoundResult::Ron {
                ref winners,
                loser,
                winning_tile,
            }) => {
                assert_eq!(winners, &vec![1]);
                assert_eq!(loser, 0);
                assert_eq!(winning_tile, Tile::new(Tile::M1));
            }
            _ => panic!("expected ron result after robbing a quad"),
        }
    }

    // ─── 九種九牌テスト ───────────────────────────────────────────────────────────

    /// 九種九牌の条件を満たす手牌をセットアップするヘルパー
    ///
    /// 1m9m1p9p1s9s1z2z3z4z5z6z7z (13種全ヤオ九牌) + ツモ牌1枚
    fn setup_nine_terminals_hand(round: &mut Round, player_idx: usize) {
        let seat = round.players[player_idx].seat_wind;
        let mut player = Player::new(seat, vec![], 25000);
        // 14枚: 1m9m1p9p1s9s1z2z3z4z5z6z7z + ツモ1m（重複は問題なし）
        player.hand = mahjong_core::hand::Hand::from("1m9m1p9p1s9s1z2z3z4z5z6z7z 1m");
        round.players[player_idx] = player;
        round.current_player = player_idx;
        round.phase = TurnPhase::WaitForDiscard;
    }

    #[test]
    fn test_check_nine_terminals_qualifies() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        setup_nine_terminals_hand(&mut round, 0);
        // 初回ツモ（捨て牌0枚）かつヤオ九牌9種以上
        assert!(round.check_nine_terminals());
    }

    #[test]
    fn test_check_nine_terminals_insufficient_types() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        let seat = round.players[0].seat_wind;
        let mut player = Player::new(seat, vec![], 25000);
        // ヤオ九牌が8種類のみ（6z・7zがなく中張牌が多い）
        // 1m,9m,1p,9p,1s,9s,1z,2z = 8種
        player.hand = mahjong_core::hand::Hand::from("1m9m1p9p1s9s1z2z5m5p5s5s 1m");
        round.players[0] = player;
        round.current_player = 0;
        round.phase = TurnPhase::WaitForDiscard;
        assert!(!round.check_nine_terminals());
    }

    #[test]
    fn test_check_nine_terminals_after_discard() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        setup_nine_terminals_hand(&mut round, 0);
        // 捨て牌を1枚追加（既に1巡した状態を再現）
        round.players[0].discards.push(crate::player::Discard {
            tile: Tile::new(Tile::M5),
            is_tsumogiri: true,
            is_riichi_declaration: false,
            is_called: false,
        });
        // 捨て牌済みなので宣言不可
        assert!(!round.check_nine_terminals());
    }

    #[test]
    fn test_do_nine_terminals_declare() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        setup_nine_terminals_hand(&mut round, 0);
        round.phase = TurnPhase::WaitForNineTerminals;
        round.drain_events();

        assert!(round.do_nine_terminals(0, true));
        assert_eq!(round.phase, TurnPhase::RoundOver);
        assert!(matches!(round.result, Some(RoundResult::SpecialDraw)));

        let events = round.drain_events();
        let has_round_draw = events.iter().any(|(_idx, e)| {
            matches!(
                e,
                ServerEvent::RoundDraw {
                    reason: DrawReason::NineTerminals,
                    ..
                }
            )
        });
        assert!(has_round_draw, "九種九牌流局イベントが生成されていない");
    }

    #[test]
    fn test_do_nine_terminals_continue() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        setup_nine_terminals_hand(&mut round, 0);
        round.phase = TurnPhase::WaitForNineTerminals;
        round.drain_events();

        assert!(round.do_nine_terminals(0, false));
        // 続行 → 打牌フェーズへ
        assert_eq!(round.phase, TurnPhase::WaitForDiscard);
        assert!(round.result.is_none());
    }

    #[test]
    fn test_do_nine_terminals_wrong_player() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        setup_nine_terminals_hand(&mut round, 0);
        round.phase = TurnPhase::WaitForNineTerminals;

        // 別プレイヤーからのアクションは無効
        assert!(!round.do_nine_terminals(1, true));
        assert_eq!(round.phase, TurnPhase::WaitForNineTerminals);
    }

    #[test]
    fn test_do_draw_triggers_nine_terminals_phase() {
        // 牌山の先頭を7z（13種目のヤオ九牌）に設定する
        // Wall::from_tiles は先頭から draw() するため、先頭に7zを置く
        let mut wall_tiles: Vec<Tile> = vec![Tile::new(Tile::Z7)];
        // 残りは適当な牌で埋める（最低 14 枚の王牌分が必要）
        for _ in 0..(70 + 14) {
            wall_tiles.push(Tile::new(Tile::M5));
        }
        let wall = Wall::from_tiles(wall_tiles);

        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        round.wall = wall;

        // 手牌をヤオ九牌12種に設定（ツモで7zが来て13種になる）
        let seat = round.players[0].seat_wind;
        let mut player = Player::new(seat, vec![], 25000);
        player.hand = mahjong_core::hand::Hand::from("1m9m1p9p1s9s1z2z3z4z5z6z5m");
        round.players[0] = player;
        round.current_player = 0;
        round.phase = TurnPhase::Draw;
        round.drain_events();

        round.do_draw();

        assert_eq!(
            round.phase,
            TurnPhase::WaitForNineTerminals,
            "九種九牌条件達成時にWaitForNineTerminalsになるべき"
        );

        let events = round.drain_events();
        let has_available = events
            .iter()
            .any(|(_idx, e)| matches!(e, ServerEvent::NineTerminalsAvailable));
        assert!(
            has_available,
            "NineTerminalsAvailableイベントが生成されていない"
        );
    }

    #[test]
    fn test_nine_terminals_disabled_by_setting() {
        let mut wall_tiles: Vec<Tile> = vec![Tile::new(Tile::Z7)];
        for _ in 0..(70 + 14) {
            wall_tiles.push(Tile::new(Tile::M5));
        }
        let wall = Wall::from_tiles(wall_tiles);

        let mut settings = Settings::new();
        settings.nine_terminals_draw = false;
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, settings);
        round.wall = wall;

        let seat = round.players[0].seat_wind;
        let mut player = Player::new(seat, vec![], 25000);
        player.hand = mahjong_core::hand::Hand::from("1m9m1p9p1s9s1z2z3z4z5z6z5m");
        round.players[0] = player;
        round.current_player = 0;
        round.phase = TurnPhase::Draw;
        round.drain_events();

        round.do_draw();

        // 設定オフなら通常の打牌フェーズになる
        assert_eq!(round.phase, TurnPhase::WaitForDiscard);
    }

    // ─── 三家和流局テスト ─────────────────────────────────────────────────────────

    /// 3人がロン可能な状態を作るヘルパー
    ///
    /// - プレイヤー0: 打牌側（5sを捨てる）
    /// - プレイヤー1,2,3: 5sでロン可能な手牌（タンヤオ形）
    ///
    /// 全員の手牌は同じ点数になる（非親・同一役・同一符）ため点数テストに使える。
    fn setup_triple_ron(round: &mut Round) {
        // プレイヤー0: 5sをツモ切り
        let seat0 = round.players[0].seat_wind;
        let mut p0 = Player::new(seat0, vec![], 25000);
        // 12枚クローズ + ツモ牌5s
        p0.hand = mahjong_core::hand::Hand::from("234m456m234p456p 5s");
        round.players[0] = p0;

        // プレイヤー1,2,3: 5sでロン可能な手牌（234m456m234p456p5s で 55s 待ち）
        // 全員タンヤオ（2〜8のみ）で同一役・同一符
        for i in 1..=3 {
            let seat = round.players[i].seat_wind;
            let mut p = Player::new(seat, vec![], 25000);
            p.hand = mahjong_core::hand::Hand::from("234m456m234p456p5s");
            round.players[i] = p;
        }

        round.current_player = 0;
        round.phase = TurnPhase::WaitForDiscard;
    }

    #[test]
    fn test_triple_ron_draw_enabled() {
        let mut settings = Settings::new();
        settings.triple_ron_draw = true;
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, settings);
        setup_triple_ron(&mut round);
        round.drain_events();

        // プレイヤー0が1mを捨てる
        assert!(round.do_discard(None));
        assert_eq!(round.phase, TurnPhase::WaitForCalls);

        // 3人全員がロン宣言
        assert!(round.respond_to_call(1, CallResponse::Ron));
        assert!(round.respond_to_call(2, CallResponse::Ron));
        assert!(round.respond_to_call(3, CallResponse::Ron));

        // 三家和流局になること
        assert_eq!(round.phase, TurnPhase::RoundOver);
        assert!(matches!(round.result, Some(RoundResult::SpecialDraw)));

        let events = round.drain_events();
        let has_triple_ron = events.iter().any(|(_idx, e)| {
            matches!(
                e,
                ServerEvent::RoundDraw {
                    reason: DrawReason::TripleRon,
                    ..
                }
            )
        });
        assert!(has_triple_ron, "三家和流局イベントが生成されていない");
    }

    #[test]
    fn test_triple_ron_draw_takes_priority_over_multiple_ron() {
        // triple_ron_draw=true かつ multiple_ron=true の両方が有効な場合、
        // 三家和流局が優先されてトリロン（全員和了）にはならないことを明示的に確認する
        let mut settings = Settings::new();
        settings.triple_ron_draw = true;
        settings.multiple_ron = true;
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, settings);
        setup_triple_ron(&mut round);
        round.drain_events();

        assert!(round.do_discard(None));
        assert!(round.respond_to_call(1, CallResponse::Ron));
        assert!(round.respond_to_call(2, CallResponse::Ron));
        assert!(round.respond_to_call(3, CallResponse::Ron));

        assert_eq!(round.phase, TurnPhase::RoundOver);
        assert!(
            matches!(round.result, Some(RoundResult::SpecialDraw)),
            "triple_ron_draw が multiple_ron より優先されること"
        );
        let events = round.drain_events();
        assert!(events.iter().any(|(_, e)| matches!(
            e,
            ServerEvent::RoundDraw {
                reason: DrawReason::TripleRon,
                ..
            }
        )));
    }

    #[test]
    fn test_triple_ron_draw_disabled_multiple_ron_disabled_picks_winner() {
        // triple_ron_draw=false, multiple_ron=false の場合は上家取り（頭ハネ）の1人ロン
        let mut settings = Settings::new();
        settings.triple_ron_draw = false;
        settings.multiple_ron = false;
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, settings);
        setup_triple_ron(&mut round);
        round.drain_events();

        assert!(round.do_discard(None));
        assert_eq!(round.phase, TurnPhase::WaitForCalls);

        assert!(round.respond_to_call(1, CallResponse::Ron));
        assert!(round.respond_to_call(2, CallResponse::Ron));
        assert!(round.respond_to_call(3, CallResponse::Ron));

        // multiple_ron=false → 上家（プレイヤー1）が優先してロン
        assert_eq!(round.phase, TurnPhase::RoundOver);
        match &round.result {
            Some(RoundResult::Ron { winners, loser, .. }) => {
                assert_eq!(winners, &vec![1]);
                assert_eq!(*loser, 0);
            }
            _ => panic!("ロン結果が期待されたが別の結果: {:?}", round.result),
        }
    }

    #[test]
    fn test_two_ron_no_draw() {
        // 2人ロンは三家和流局にならない（triple_ron_draw=true でも2人なら流局しない）
        let mut settings = Settings::new();
        settings.triple_ron_draw = true;
        // multiple_ron=true（デフォルト）なので両方和了
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, settings);
        setup_triple_ron(&mut round);
        round.drain_events();

        assert!(round.do_discard(None));
        assert_eq!(round.phase, TurnPhase::WaitForCalls);

        assert!(round.respond_to_call(1, CallResponse::Ron));
        assert!(round.respond_to_call(2, CallResponse::Ron));
        assert!(round.respond_to_call(3, CallResponse::Pass));

        // 2人ロンは流局でなくダブロン
        assert_eq!(round.phase, TurnPhase::RoundOver);
        match &round.result {
            Some(RoundResult::Ron { winners, loser, .. }) => {
                assert_eq!(winners, &vec![1, 2]);
                assert_eq!(*loser, 0);
            }
            _ => panic!("Ron結果が期待されたが別の結果: {:?}", round.result),
        }
    }

    #[test]
    fn test_two_ron_disabled_picks_winner() {
        // multiple_ron=false の場合は上家取り（頭ハネ）の1人ロン
        let mut settings = Settings::new();
        settings.multiple_ron = false;
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, settings);
        setup_triple_ron(&mut round);
        round.drain_events();

        assert!(round.do_discard(None));
        assert_eq!(round.phase, TurnPhase::WaitForCalls);

        assert!(round.respond_to_call(1, CallResponse::Ron));
        assert!(round.respond_to_call(2, CallResponse::Ron));
        assert!(round.respond_to_call(3, CallResponse::Pass));

        // multiple_ron=false → 上家（プレイヤー1）のみロン
        assert_eq!(round.phase, TurnPhase::RoundOver);
        match &round.result {
            Some(RoundResult::Ron { winners, loser, .. }) => {
                assert_eq!(winners, &vec![1]);
                assert_eq!(*loser, 0);
            }
            _ => panic!("Ron結果が期待されたが別の結果: {:?}", round.result),
        }
    }

    #[test]
    fn test_double_ron_both_win() {
        // multiple_ron=true（デフォルト）: 2人ロンで両方和了
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        setup_triple_ron(&mut round);
        round.drain_events();

        assert!(round.do_discard(None));

        assert!(round.respond_to_call(1, CallResponse::Ron));
        assert!(round.respond_to_call(2, CallResponse::Ron));
        assert!(round.respond_to_call(3, CallResponse::Pass));

        assert_eq!(round.phase, TurnPhase::RoundOver);
        match &round.result {
            Some(RoundResult::Ron { winners, loser, .. }) => {
                assert_eq!(winners, &vec![1, 2], "打順優先順で並んでいること");
                assert_eq!(*loser, 0);
            }
            _ => panic!("Ron結果が期待されたが別の結果: {:?}", round.result),
        }
    }

    #[test]
    fn test_triple_ron_all_win() {
        // multiple_ron=true かつ triple_ron_draw=false: 3人ロンで全員和了
        let mut settings = Settings::new();
        settings.multiple_ron = true;
        settings.triple_ron_draw = false;
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, settings);
        setup_triple_ron(&mut round);
        round.drain_events();

        assert!(round.do_discard(None));

        assert!(round.respond_to_call(1, CallResponse::Ron));
        assert!(round.respond_to_call(2, CallResponse::Ron));
        assert!(round.respond_to_call(3, CallResponse::Ron));

        assert_eq!(round.phase, TurnPhase::RoundOver);
        match &round.result {
            Some(RoundResult::Ron { winners, loser, .. }) => {
                assert_eq!(winners, &vec![1, 2, 3]);
                assert_eq!(*loser, 0);
            }
            _ => panic!("Ron結果が期待されたが別の結果: {:?}", round.result),
        }
    }

    #[test]
    fn test_double_ron_scores() {
        // ダブロン時のスコア: 各和了者が放銃者から独立して点数を受け取る
        // 本場ボーナスは上家取りで最初の和了者（プレイヤー1）のみ
        let mut round = Round::new(Wind::East, 0, [25000; 4], 1, 0, 0, Settings::new()); // honba=1
        setup_triple_ron(&mut round);
        round.drain_events();

        let initial_score_loser = round.players[0].score;
        let initial_score_p1 = round.players[1].score;
        let initial_score_p2 = round.players[2].score;

        assert!(round.do_discard(None));
        assert!(round.respond_to_call(1, CallResponse::Ron));
        assert!(round.respond_to_call(2, CallResponse::Ron));
        assert!(round.respond_to_call(3, CallResponse::Pass));

        // プレイヤー1: 本場ボーナスあり (honba=1 → 300点加算)
        // プレイヤー2: 本場ボーナスなし
        let p1_gain = round.players[1].score - initial_score_p1;
        let p2_gain = round.players[2].score - initial_score_p2;
        assert!(
            p1_gain > p2_gain,
            "最初の和了者が本場ボーナスを得ること: p1={}, p2={}",
            p1_gain,
            p2_gain
        );
        assert_eq!(
            p1_gain - p2_gain,
            300,
            "本場ボーナスの差は1本場=300点であること"
        );

        // 放銃者は両方の点数を払う
        let loser_loss = initial_score_loser - round.players[0].score;
        let total_gain = p1_gain + p2_gain;
        assert_eq!(
            loser_loss, total_gain,
            "放銃者の支払いが全和了者の取得合計と一致すること"
        );
    }

    #[test]
    fn test_double_ron_events_generated() {
        // ダブロン時に各和了者分のRoundWonイベントが生成されること
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0, 0, Settings::new());
        setup_triple_ron(&mut round);
        round.drain_events();

        assert!(round.do_discard(None));
        assert!(round.respond_to_call(1, CallResponse::Ron));
        assert!(round.respond_to_call(2, CallResponse::Ron));
        assert!(round.respond_to_call(3, CallResponse::Pass));

        let events = round.drain_events();
        let won_events: Vec<_> = events
            .iter()
            .filter(|(idx, e)| *idx == 0 && matches!(e, ServerEvent::RoundWon { .. }))
            .collect();
        assert_eq!(
            won_events.len(),
            2,
            "ダブロンで2件のRoundWonイベントが生成されること"
        );
    }

    #[test]
    fn test_multi_ron_riichi_sticks_first_winner_only() {
        // 供託棒は最初の和了者（プレイヤー1）のみ取得
        let settings = Settings::new();
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 2, 0, settings); // riichi_sticks=2
        setup_triple_ron(&mut round);
        round.drain_events();

        let initial_p1 = round.players[1].score;
        let initial_p2 = round.players[2].score;

        assert!(round.do_discard(None));
        assert!(round.respond_to_call(1, CallResponse::Ron));
        assert!(round.respond_to_call(2, CallResponse::Ron));
        assert!(round.respond_to_call(3, CallResponse::Pass));

        let p1_gain = round.players[1].score - initial_p1;
        let p2_gain = round.players[2].score - initial_p2;
        // プレイヤー1は供託2本（2000点）分多く得点しているはず
        assert_eq!(
            p1_gain - p2_gain,
            2000,
            "供託2本はプレイヤー1のみ取得: 差は2000点"
        );
        assert_eq!(round.riichi_sticks, 0, "供託棒はすべて消費されること");
    }
}
