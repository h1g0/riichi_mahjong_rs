//! 局の管理
//!
//! 1局分のゲーム進行を管理する。
//! ツモ → 打牌 → 鳴き判定 → 次の手番 のターンフローを制御する。

use mahjong_core::hand_info::hand_analyzer::HandAnalyzer;
use mahjong_core::tile::{Tile, TileType, Wind};

use crate::player::Player;
use crate::protocol::{AvailableCall, CallType, DrawReason, ServerEvent};
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
    /// 局終了
    RoundOver,
}

/// 局の結果
#[derive(Debug, Clone)]
pub enum RoundResult {
    /// ツモ和了
    Tsumo {
        winner: usize,
        winning_tile: Tile,
    },
    /// ロン和了
    Ron {
        winner: usize,
        loser: usize,
        winning_tile: Tile,
    },
    /// 荒牌流局（牌山切れ）
    ExhaustiveDraw,
    /// 途中流局（四風連打、四家立直、九種九牌）
    SpecialDraw,
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
    /// ポンを宣言したプレイヤー
    pub pon_declared: Option<usize>,
    /// チーを宣言したプレイヤーと使う牌種
    pub chi_declared: Option<(usize, [TileType; 2])>,
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
    /// ターンフェーズ
    pub phase: TurnPhase,
    /// 局の結果（終了時にセット）
    pub result: Option<RoundResult>,
    /// 溜まったイベントキュー
    events: Vec<(usize, ServerEvent)>,
    /// 鳴き待ち中の状態
    pub call_state: Option<CallState>,
}

impl Round {
    /// 新しい局を開始する
    ///
    /// - `prevailing_wind`: 場風（東場なら East）
    /// - `dealer`: 親のプレイヤーインデックス（0-3）
    /// - `initial_scores`: 各プレイヤーの初期点数
    pub fn new(prevailing_wind: Wind, dealer: usize, initial_scores: [i32; 4], honba: usize, round_number: usize) -> Self {
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
            phase: TurnPhase::Draw,
            result: None,
            events,
            call_state: None,
        }
    }

    /// 各プレイヤーの点数を返す
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

    /// ツモフェーズを実行する
    /// 山から1枚引いて現在のプレイヤーに配る
    pub fn do_draw(&mut self) -> bool {
        if self.phase != TurnPhase::Draw {
            return false;
        }

        // 牌山が空なら流局
        if self.wall.is_empty() {
            self.do_exhaustive_draw();
            return true;
        }

        let tile = self.wall.draw().unwrap();
        let remaining = self.wall.remaining();
        self.players[self.current_player].draw(tile);

        // ツモ和了チェック
        let can_tsumo = self.can_tsumo();

        // リーチ可能チェック
        let can_riichi = self.can_player_riichi(self.current_player);

        // 自分にはツモ牌を公開
        self.events.push((
            self.current_player,
            ServerEvent::TileDrawn {
                tile,
                remaining_tiles: remaining,
                can_tsumo,
                can_riichi,
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

        self.phase = TurnPhase::WaitForDiscard;
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
        let mut available_calls: [Vec<AvailableCall>; 4] = [
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ];
        let mut responded = [true; 4]; // デフォルトは応答済み（対象外）

        for i in 0..4 {
            if i == discarder {
                continue;
            }

            let player = &self.players[i];

            // リーチ中は鳴き不可（ロンのみ可）
            // ロン判定: フリテンでなく、和了形であること
            if !player.is_furiten() {
                let win_result = scoring::check_ron(
                    player,
                    discarded_tile,
                    self.prevailing_wind,
                    is_last_tile,
                );
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
            if player.can_pon(discarded_tile) {
                available_calls[i].push(AvailableCall::Pon);
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
            chi_declared: None,
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
            CallResponse::Pon => {
                if call_state.available_calls[player_idx]
                    .iter()
                    .any(|c| matches!(c, AvailableCall::Pon))
                {
                    call_state.pon_declared = Some(player_idx);
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

    /// 鳴きを解決する（優先度: ロン > ポン > チー > パス）
    fn resolve_calls(&mut self) {
        let call_state = self.call_state.take().unwrap();

        // 1. ロン（最優先）
        if !call_state.ron_declared.is_empty() {
            // 複数ロンの場合、捨てたプレイヤーから反時計回りで最初のプレイヤーが和了
            let discarder = call_state.discarder;
            let winner = call_state
                .ron_declared
                .iter()
                .min_by_key(|&&p| (p + 4 - discarder) % 4)
                .copied()
                .unwrap();

            self.execute_ron(winner, discarder, call_state.discarded_tile);
            return;
        }

        // 2. ポン
        if let Some(caller) = call_state.pon_declared {
            self.execute_pon(caller, call_state.discarder, call_state.discarded_tile);
            return;
        }

        // 3. チー
        if let Some((caller, hand_tile_types)) = call_state.chi_declared {
            self.execute_chi(caller, call_state.discarder, call_state.discarded_tile, hand_tile_types);
            return;
        }

        // 4. 全員パス → 次のプレイヤーへ
        self.current_player = (call_state.discarder + 1) % 4;
        self.phase = TurnPhase::Draw;

        // 特殊流局チェック
        self.check_special_draws();
    }

    /// ロン和了を実行する
    fn execute_ron(&mut self, winner: usize, loser: usize, winning_tile: Tile) {
        let is_last_tile = self.wall.is_empty();

        // 一時的にdrawnを設定して点数計算
        let win_result = scoring::check_ron(
            &self.players[winner],
            winning_tile,
            self.prevailing_wind,
            is_last_tile,
        );

        if !win_result.is_win {
            // ロンできないはずだが安全のため
            self.current_player = (loser + 1) % 4;
            self.phase = TurnPhase::Draw;
            return;
        }

        let mut score_result = win_result.score_result.unwrap();

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
            self.honba,
        );

        for i in 0..4 {
            self.players[i].score += deltas[i];
        }

        // 捨て牌を「鳴かれた」としてマーク
        if let Some(last_discard) = self.players[loser].discards.last_mut() {
            last_discard.is_called = true;
        }

        let scores = self.get_scores();
        let winner_wind = self.players[winner].seat_wind;
        let loser_wind = self.players[loser].seat_wind;

        // 役情報を構築
        let yaku_list: Vec<(String, u32)> = score_result
            .yaku_list
            .iter()
            .map(|(name, han)| (name.to_string(), *han))
            .collect();
        let rank_name = scoring::rank_to_string(&score_result.rank).to_string();

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::RoundWon {
                    winner: winner_wind,
                    loser: Some(loser_wind),
                    winning_tile,
                    scores,
                    yaku_list: yaku_list.clone(),
                    han: score_result.han,
                    fu: score_result.fu,
                    score_points: deltas[winner],
                    rank_name: rank_name.clone(),
                },
            ));
        }

        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::Ron {
            winner,
            loser,
            winning_tile,
        });
    }

    /// ポンを実行する
    fn execute_pon(&mut self, caller: usize, discarder: usize, called_tile: Tile) {
        let from = Player::open_from_relative(caller, discarder);
        self.players[caller].do_pon(called_tile, from);

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
            .opened()
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

    /// チーを実行する
    fn execute_chi(
        &mut self,
        caller: usize,
        discarder: usize,
        called_tile: Tile,
        hand_tile_types: [TileType; 2],
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
            .opened()
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

        match HandAnalyzer::new(&hand) {
            Ok(analyzer) => analyzer.shanten == 0,
            Err(_) => false,
        }
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
            return false;
        }
        if !player.is_menzen() {
            return false;
        }
        if player.score < 1000 {
            return false;
        }
        if self.wall.remaining() < 1 {
            return false;
        }
        if player.hand.drawn().is_none() {
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

        // 全プレイヤーにリーチ通知
        let player_wind = self.players[player_idx].seat_wind;
        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::PlayerRiichi {
                    player: player_wind,
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
        let result = scoring::check_win(player, self.prevailing_wind, true, is_last_tile);
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
        let win_result = scoring::check_win(player, self.prevailing_wind, true, is_last_tile);

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

        // 点数を適用
        for i in 0..4 {
            self.players[i].score += deltas[i];
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
                    score_points: deltas[winner],
                    rank_name: rank_name.clone(),
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
            if scoring::is_tenpai(&self.players[i]) {
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

        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::ExhaustiveDraw);

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::RoundDraw {
                    scores,
                    reason: DrawReason::Exhaustive,
                    tenpai: tenpai_winds.clone(),
                },
            ));
        }
    }

    /// 特殊流局をチェックする（四風連打、四家立直）
    fn check_special_draws(&mut self) {
        // 四風連打チェック: 全員が1枚ずつ捨てて、全て同じ風牌
        if self.check_four_winds_draw() {
            self.declare_special_draw(DrawReason::FourWinds);
            return;
        }

        // 四家立直チェック: 全員がリーチ宣言済み
        if self.check_four_riichi_draw() {
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

        self.players.iter().all(|p| p.discards[0].tile.get() == first_tile.get())
    }

    /// 四家立直を判定する
    ///
    /// 条件: 全4プレイヤーがリーチ宣言済み
    fn check_four_riichi_draw(&self) -> bool {
        self.players.iter().all(|p| p.is_riichi)
    }

    /// 特殊流局を宣言する
    fn declare_special_draw(&mut self, reason: DrawReason) {
        let scores = self.get_scores();
        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::SpecialDraw);

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::RoundDraw {
                    scores,
                    reason: reason.clone(),
                    tenpai: Vec::new(),
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
    /// ポン
    Pon,
    /// チー（手牌から使う牌の種類2つ）
    Chi { hand_tile_types: [TileType; 2] },
    /// パス
    Pass,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_new() {
        let round = Round::new(Wind::East, 0, [25000; 4], 0, 0);
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
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0);
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
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0);
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
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0);
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
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0);
        round.play_to_end();

        assert!(round.is_over());
        assert!(round.result.is_some());
    }

    #[test]
    fn test_round_scores() {
        let round = Round::new(Wind::East, 0, [25000, 30000, 20000, 25000], 0, 0);
        let scores = round.get_scores();
        assert_eq!(scores, [25000, 30000, 20000, 25000]);
    }

    #[test]
    fn test_round_events_on_start() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0);
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
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0);
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
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0);
        let seat_wind = round.players[1].seat_wind;
        let hand = mahjong_core::hand::Hand::from("234678m56p567s55z");
        round.players[1] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);

        let call_state = round.check_available_calls(Tile::new(Tile::Z5), 0);
        assert!(call_state.available_calls[1]
            .iter()
            .any(|call| matches!(call, AvailableCall::Pon)));
        assert!(!call_state.available_calls[1]
            .iter()
            .any(|call| matches!(call, AvailableCall::Ron)));
    }

    #[test]
    fn test_do_riichi_requires_tenpai_after_discard() {
        let mut round = Round::new(Wind::East, 0, [25000; 4], 0, 0);
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
}
