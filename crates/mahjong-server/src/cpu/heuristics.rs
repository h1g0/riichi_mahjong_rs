//! 定石（heuristics）フレームワーク
//!
//! 人間らしい打牌判断の定石を「打牌候補へのスコア補正」として表現し、
//! CPU の強さレベルに応じて有効な定石だけを適用する（issue #142）。
//!
//! 個々の定石は `DiscardHeuristic` として定義し、`DISCARD_HEURISTICS` に
//! 登録する。定石をハードコードの分岐で書かないことで、
//! レベルごとの有効/無効切り替えと定石単位のテストを可能にする。

use super::client::{CpuConfig, CpuLevel};
use super::evaluator::DiscardCandidate;
use super::state::CpuGameState;

/// 打牌補正を計算する際の局面コンテキスト
pub struct DiscardContext<'a> {
    /// CPU が観測しているゲーム状態
    pub state: &'a CpuGameState,
    /// CPU 設定
    pub config: &'a CpuConfig,
    /// 攻撃継続中か（false なら守備優先）
    pub attacking: bool,
}

/// 定石1つの定義
///
/// `apply` は「この牌を捨てる」ことの良さに対する補正値を返す。
/// 正の値はその牌を切りやすく、負の値は切りにくくする。
/// スケールは `select_best_discard` の基本スコア（向聴数1段階 = 100.0）に合わせる。
pub struct DiscardHeuristic {
    /// 定石名（ログ・テスト用）
    pub name: &'static str,
    /// この定石が有効になる最低レベル
    pub min_level: CpuLevel,
    /// 補正関数
    pub apply: fn(&DiscardContext, &DiscardCandidate) -> f64,
}

/// 打牌定石のレジストリ
///
/// issue #142 の定石を後続の変更でここに追加していく。
/// 補正は合算されるため、登録順は結果に影響しない。
pub const DISCARD_HEURISTICS: &[DiscardHeuristic] = &[];

/// 打牌候補1つに対して、有効な全定石の補正値を合算する
pub fn discard_adjustment(ctx: &DiscardContext, candidate: &DiscardCandidate) -> f64 {
    discard_adjustment_with(DISCARD_HEURISTICS, ctx, candidate)
}

/// レジストリを指定して補正値を合算する
///
/// レベルが `min_level` 未満の定石は適用しない。
fn discard_adjustment_with(
    heuristics: &[DiscardHeuristic],
    ctx: &DiscardContext,
    candidate: &DiscardCandidate,
) -> f64 {
    heuristics
        .iter()
        .filter(|h| ctx.config.level >= h.min_level)
        .map(|h| (h.apply)(ctx, candidate))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mahjong_core::hand::Hand;
    use mahjong_core::hand_info::hand_analyzer::calc_shanten_number;
    use mahjong_core::tile::Tile;

    use crate::cpu::client::CpuPersonality;

    fn make_candidate(tile_type: u32) -> DiscardCandidate {
        let hand = Hand::new(vec![Tile::new(tile_type)], None);
        DiscardCandidate {
            tile: Tile::new(tile_type),
            shanten: calc_shanten_number(&hand),
            acceptance_count: 0,
            estimated_value: 0.0,
            safety: 0.5,
        }
    }

    fn fixed_bonus_heuristic(
        name: &'static str,
        min_level: CpuLevel,
        apply: fn(&DiscardContext, &DiscardCandidate) -> f64,
    ) -> DiscardHeuristic {
        DiscardHeuristic {
            name,
            min_level,
            apply,
        }
    }

    #[test]
    fn test_empty_registry_returns_zero() {
        let state = CpuGameState::new();
        let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
        let ctx = DiscardContext {
            state: &state,
            config: &config,
            attacking: true,
        };
        let candidate = make_candidate(Tile::M1);
        assert_eq!(discard_adjustment(&ctx, &candidate), 0.0);
    }

    #[test]
    fn test_level_gating() {
        let heuristics = [
            fixed_bonus_heuristic("weak-rule", CpuLevel::Weak, |_, _| 1.0),
            fixed_bonus_heuristic("normal-rule", CpuLevel::Normal, |_, _| 10.0),
            fixed_bonus_heuristic("strong-rule", CpuLevel::Strong, |_, _| 100.0),
        ];
        let state = CpuGameState::new();
        let candidate = make_candidate(Tile::M1);

        // Weak: 弱以上の定石のみ
        let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
        let ctx = DiscardContext {
            state: &state,
            config: &config,
            attacking: true,
        };
        assert_eq!(discard_adjustment_with(&heuristics, &ctx, &candidate), 1.0);

        // Normal: 弱以上 + 中以上
        let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
        let ctx = DiscardContext {
            state: &state,
            config: &config,
            attacking: true,
        };
        assert_eq!(discard_adjustment_with(&heuristics, &ctx, &candidate), 11.0);

        // Strong: 全て
        let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
        let ctx = DiscardContext {
            state: &state,
            config: &config,
            attacking: true,
        };
        assert_eq!(
            discard_adjustment_with(&heuristics, &ctx, &candidate),
            111.0
        );
    }

    #[test]
    fn test_heuristic_can_reference_candidate_and_context() {
        // 候補とコンテキストの両方を参照する定石が書けることを確認する
        let heuristics = [fixed_bonus_heuristic(
            "honor-in-defense",
            CpuLevel::Weak,
            |ctx, c| {
                if !ctx.attacking && c.tile.get() >= 27 {
                    50.0
                } else {
                    0.0
                }
            },
        )];
        let state = CpuGameState::new();
        let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);

        let honor = make_candidate(Tile::Z1);
        let number = make_candidate(Tile::M5);

        let defending = DiscardContext {
            state: &state,
            config: &config,
            attacking: false,
        };
        assert_eq!(
            discard_adjustment_with(&heuristics, &defending, &honor),
            50.0
        );
        assert_eq!(
            discard_adjustment_with(&heuristics, &defending, &number),
            0.0
        );

        let attacking = DiscardContext {
            state: &state,
            config: &config,
            attacking: true,
        };
        assert_eq!(
            discard_adjustment_with(&heuristics, &attacking, &honor),
            0.0
        );
    }
}
