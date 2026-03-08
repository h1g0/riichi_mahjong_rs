//! 局の管理
//!
//! 1局分のゲーム進行を管理する。
//! ツモ → 打牌 → 次の手番 のターンフローを制御する。

use mahjong_core::tile::{Tile, Wind};

use crate::player::Player;
use crate::protocol::ServerEvent;
use crate::wall::Wall;

/// ターンのフェーズ
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnPhase {
    /// ツモフェーズ: 現在のプレイヤーがツモる
    Draw,
    /// 打牌待ち: 現在のプレイヤーの打牌を待つ
    WaitForDiscard,
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
}

/// 1局分の状態
pub struct Round {
    /// 牌山
    pub wall: Wall,
    /// 4人のプレイヤー
    pub players: [Player; 4],
    /// 場風
    pub prevailing_wind: Wind,
    /// 現在の手番プレイヤー（0-3）
    pub current_player: usize,
    /// ターンフェーズ
    pub phase: TurnPhase,
    /// 局の結果（終了時にセット）
    pub result: Option<RoundResult>,
    /// 溜まったイベントキュー
    events: Vec<(usize, ServerEvent)>,
}

impl Round {
    /// 新しい局を開始する
    ///
    /// - `prevailing_wind`: 場風（東場なら East）
    /// - `dealer`: 親のプレイヤーインデックス（0-3）
    /// - `initial_scores`: 各プレイヤーの初期点数
    pub fn new(prevailing_wind: Wind, dealer: usize, initial_scores: [i32; 4]) -> Self {
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
                },
            ));
        }

        Round {
            wall,
            players,
            prevailing_wind,
            current_player: dealer,
            phase: TurnPhase::Draw,
            result: None,
            events,
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
            self.phase = TurnPhase::RoundOver;
            self.result = Some(RoundResult::ExhaustiveDraw);
            let scores = self.get_scores();
            for i in 0..4 {
                self.events.push((i, ServerEvent::RoundDraw { scores }));
            }
            return true;
        }

        let tile = self.wall.draw().unwrap();
        let remaining = self.wall.remaining();
        self.players[self.current_player].draw(tile);

        // 自分にはツモ牌を公開
        self.events.push((
            self.current_player,
            ServerEvent::TileDrawn {
                tile,
                remaining_tiles: remaining,
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
    /// - `tile_index`: 手牌のインデックス（Noneならツモ切り）
    pub fn do_discard(&mut self, tile_index: Option<usize>) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }

        let discarded = self.players[self.current_player].discard(tile_index);
        let is_tsumogiri = tile_index.is_none();
        let current_wind = self.players[self.current_player].seat_wind;

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

        // 次のプレイヤーへ
        self.current_player = (self.current_player + 1) % 4;
        self.phase = TurnPhase::Draw;

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

    /// 局を最後まで自動進行する（全員ツモ切り）
    /// テスト・デバッグ用
    pub fn play_to_end(&mut self) {
        while self.phase != TurnPhase::RoundOver {
            self.advance_auto_player();
        }
    }

    /// 局が終了したかどうか
    pub fn is_over(&self) -> bool {
        self.phase == TurnPhase::RoundOver
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_new() {
        let round = Round::new(Wind::East, 0, [25000; 4]);
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
        let mut round = Round::new(Wind::East, 0, [25000; 4]);
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
        let mut round = Round::new(Wind::East, 0, [25000; 4]);
        round.drain_events();
        round.do_draw();
        round.drain_events();

        // ツモ切り
        assert!(round.do_discard(None));
        assert_eq!(round.phase, TurnPhase::Draw);
        assert_eq!(round.current_player, 1); // 次のプレイヤーへ

        // 4つのTileDiscardedイベント
        let events = round.drain_events();
        assert_eq!(events.len(), 4);
    }

    #[test]
    fn test_round_turn_flow() {
        let mut round = Round::new(Wind::East, 0, [25000; 4]);
        round.drain_events();

        // 4人分のターンを回す
        for expected_player in 0..4 {
            assert_eq!(round.current_player, expected_player);
            assert!(round.advance_auto_player());
        }

        // 一巡して最初のプレイヤーに戻る
        assert_eq!(round.current_player, 0);
    }

    #[test]
    fn test_round_play_to_end() {
        let mut round = Round::new(Wind::East, 0, [25000; 4]);
        round.play_to_end();

        assert!(round.is_over());
        assert!(round.result.is_some());

        match round.result.as_ref().unwrap() {
            RoundResult::ExhaustiveDraw => {} // 全員ツモ切りなので流局
            _ => panic!("全員ツモ切りなので流局になるはず"),
        }
    }

    #[test]
    fn test_round_scores() {
        let round = Round::new(Wind::East, 0, [25000, 30000, 20000, 25000]);
        let scores = round.get_scores();
        assert_eq!(scores, [25000, 30000, 20000, 25000]);
    }

    #[test]
    fn test_round_events_on_start() {
        let mut round = Round::new(Wind::East, 0, [25000; 4]);
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
}
