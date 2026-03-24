use anyhow::Result;

use std::cmp::*;

use crate::hand::Hand;
use crate::hand_info::block::*;
use crate::tile::*;
use crate::winning_hand::name::Form;

const UNAVAILABLE_SHANTEN: i32 = i32::MAX;

/// 与えられた手牌について、向聴数が最小になる時の面子・対子等の組み合わせを計算して格納する
///
/// 通常形・七対子の場合は面子・対子等の情報もVecに格納される。
/// 国士無双の場合は（今のところ）向聴数のみが格納される。
#[derive(Debug, Eq)]
pub struct HandAnalyzer {
    /// 向聴数：あと牌を何枚交換すれば聴牌できるかの最小数。聴牌状態が`0`、和了が`-1`。
    pub shanten: i32,
    /// どの和了形か
    pub form: Form,
    /// 刻子（同じ牌が3枚）が入るVec
    pub same3: Vec<Same3>,
    /// 順子（連続した牌が3枚）が入るVec
    pub sequential3: Vec<Sequential3>,
    /// 対子（同じ牌が2枚）が入るVec
    pub same2: Vec<Same2>,
    /// 塔子（連続した牌が2枚）もしくは嵌張（順子の真ん中が抜けている2枚）が入るVec
    pub sequential2: Vec<Sequential2>,
    /// 面子や対子・塔子などを構成しない、単独の牌が入るVec
    pub single: Vec<TileType>,
}
impl Ord for HandAnalyzer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.shanten.cmp(&other.shanten)
    }
}

impl PartialOrd for HandAnalyzer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for HandAnalyzer {
    fn eq(&self, other: &Self) -> bool {
        self.shanten == other.shanten
    }
}

impl HandAnalyzer {
    fn unavailable(form: Form) -> HandAnalyzer {
        HandAnalyzer {
            shanten: UNAVAILABLE_SHANTEN,
            form,
            same3: Vec::new(),
            sequential3: Vec::new(),
            same2: Vec::new(),
            sequential2: Vec::new(),
            single: Vec::new(),
        }
    }

    /// 七対子・国士無双・通常の3つの和了形に対してそれぞれ向聴数を求め、最小のものを返す。
    /// # Examples
    ///
    /// ```
    /// use mahjong_core::hand::*;
    /// use mahjong_core::hand_info::hand_analyzer::*;
    /// use mahjong_core::winning_hand::name::*;
    ///
    /// // 通常型で和了る
    /// let nm_test_str = "222333444666s6z 6z";
    /// let nm_test = Hand::from(nm_test_str);
    /// let analyzer = HandAnalyzer::new(&nm_test).unwrap();
    /// assert_eq!(
    ///   analyzer.shanten,
    ///   -1
    /// );
    /// assert_eq!(
    ///   analyzer.form,
    ///   Form::Normal
    /// );
    /// ```
    pub fn new(hand: &Hand) -> Result<HandAnalyzer> {
        let sp = HandAnalyzer::new_by_form(hand, Form::SevenPairs)?;
        let to = HandAnalyzer::new_by_form(hand, Form::ThirteenOrphans)?;
        let normal = HandAnalyzer::new_by_form(hand, Form::Normal)?;
        // 高点法: 和了している場合（shanten == -1）、通常形を優先する。
        // 二盃口（3翻）は七対子（2翻）より高得点であるため、通常形で和了できるならそちらを採用する。
        if normal.shanten == -1 {
            Ok(normal)
        } else {
            Ok(min(min(sp, to), normal))
        }
    }

    /// 和了形を指定して向聴数を計算する
    /// # Examples
    ///
    /// ```
    /// use mahjong_core::hand::*;
    /// use mahjong_core::hand_info::hand_analyzer::*;
    /// use mahjong_core::winning_hand::name::*;
    ///
    /// // 国士無双で和了る
    /// let to_test_str = "19m19p19s1234567z 1m";
    /// let to_test = Hand::from(to_test_str);
    /// assert_eq!(
    ///   HandAnalyzer::new_by_form(&to_test, Form::ThirteenOrphans).unwrap().shanten,
    ///   -1
    /// );
    ///
    /// // 七対子で和了る
    /// let sp_test_str = "1122m3344p5566s7z 7z";
    /// let sp_test = Hand::from(sp_test_str);
    /// assert_eq!(
    ///   HandAnalyzer::new_by_form(&sp_test, Form::SevenPairs).unwrap().shanten,
    ///   -1
    /// );
    ///
    /// // 通常型で和了る
    /// let nm_test_str = "1112345678999m 5m";
    /// let nm_test = Hand::from(nm_test_str);
    /// assert_eq!(
    ///   HandAnalyzer::new_by_form(&nm_test, Form::Normal).unwrap().shanten,
    ///   -1
    /// );
    /// ```
    pub fn new_by_form(hand: &Hand, form: Form) -> Result<HandAnalyzer> {
        Ok(match form {
            Form::SevenPairs => HandAnalyzer::calc_seven_pairs(hand)?,
            Form::ThirteenOrphans => HandAnalyzer::calc_thirteen_orphans(hand)?,
            Form::Normal => HandAnalyzer::calc_normal_form(hand)?,
        })
    }

    /// 七対子への向聴数を計算する
    ///
    /// Vecへの詰め込みは`same2`（対子）以外は`single`（単独）に詰め込まれる。
    /// 七対子はVecを使用する役として断么九・混老頭・混一色・清一色と複合しうる
    fn calc_seven_pairs(hand: &Hand) -> Result<HandAnalyzer> {
        if !hand.opened().is_empty() {
            return Ok(HandAnalyzer::unavailable(Form::SevenPairs));
        }

        let mut pair: u32 = 0;
        let mut kind: u32 = 0;
        let mut t = hand.summarize_tiles();
        let mut same2: Vec<Same2> = Vec::new();

        for i in 0..Tile::LEN {
            if t[i] > 0 {
                kind += 1;
                if t[i] >= 2 {
                    pair += 1;
                    same2.push(Same2::new(i as TileType, i as TileType)?);
                    t[i] -= 2;
                }
            }
        }
        let num_to_win: i32 = (7 - pair + if kind < 7 { 7 - kind } else { 0 }) as i32;
        let mut single: Vec<TileType> = Vec::new();
        for i in 0..Tile::LEN {
            if t[i] > 0 {
                for _ in 0..t[i] {
                    single.push(i as TileType);
                }
            }
        }
        Ok(HandAnalyzer {
            shanten: num_to_win - 1,
            form: Form::SevenPairs,
            same3: Vec::new(),
            sequential3: Vec::new(),
            same2,
            sequential2: Vec::new(),
            single,
        })
    }

    /// 国士無双への向聴数を計算する
    ///
    /// Vecへの詰め込みは未実装（詰め込んでも意味がない）
    fn calc_thirteen_orphans(hand: &Hand) -> Result<HandAnalyzer> {
        if !hand.opened().is_empty() {
            return Ok(HandAnalyzer::unavailable(Form::ThirteenOrphans));
        }

        let to_tiles = [
            Tile::M1,
            Tile::M9,
            Tile::P1,
            Tile::P9,
            Tile::S1,
            Tile::S9,
            Tile::Z1,
            Tile::Z2,
            Tile::Z3,
            Tile::Z4,
            Tile::Z5,
            Tile::Z6,
            Tile::Z7,
        ];
        let mut pair: u32 = 0;
        let mut kind: u32 = 0;
        let t = hand.summarize_tiles();

        for i in &to_tiles {
            if t[*i as usize] > 0 {
                kind = kind + 1;
                if t[*i as usize] >= 2 {
                    pair += 1;
                }
            }
        }
        let num_to_win: i32 = (14 - kind - if pair > 0 { 1 } else { 0 }) as i32;
        Ok(HandAnalyzer {
            shanten: num_to_win - 1,
            form: Form::ThirteenOrphans,
            same3: Vec::new(),
            sequential3: Vec::new(),
            same2: Vec::new(),
            sequential2: Vec::new(),
            single: Vec::new(),
        })
    }

    /// 通常の役への向聴数を計算する
    fn calc_normal_form(hand: &Hand) -> Result<HandAnalyzer> {
        let (shanten, tracking) = calc_normal_shanten::<FullTracking>(hand)?;
        let FullTracking {
            same3,
            sequential3,
            same2,
            sequential2,
            single,
        } = tracking;
        Ok(HandAnalyzer {
            shanten,
            form: Form::Normal,
            same3,
            sequential3,
            same2,
            sequential2,
            single,
        })
    }
}

/// 和了しているか否か
pub fn has_won(hand: &HandAnalyzer) -> bool {
    hand.shanten == -1
}

/// 向聴数のみを高速に計算する（ブロック分解のVec割り当てなし）
///
/// `HandAnalyzer::new()` と同じ結果を返すが、向聴数の数値のみを返す。
/// CPU打牌評価など大量に呼び出す箇所で使用する。
pub fn shanten_number(hand: &Hand) -> i32 {
    let sp = shanten_seven_pairs(hand);
    let to = shanten_thirteen_orphans(hand);
    let nm = calc_normal_shanten::<CountOnly>(hand)
        .map(|(s, _)| s)
        .unwrap_or(UNAVAILABLE_SHANTEN);
    min(min(sp, to), nm)
}

fn shanten_seven_pairs(hand: &Hand) -> i32 {
    if !hand.opened().is_empty() {
        return UNAVAILABLE_SHANTEN;
    }
    let t = hand.summarize_tiles();
    let mut pair: u32 = 0;
    let mut kind: u32 = 0;
    for i in 0..Tile::LEN as usize {
        if t[i] > 0 {
            kind += 1;
            if t[i] >= 2 {
                pair += 1;
            }
        }
    }
    (7 - pair + if kind < 7 { 7 - kind } else { 0 }) as i32 - 1
}

fn shanten_thirteen_orphans(hand: &Hand) -> i32 {
    if !hand.opened().is_empty() {
        return UNAVAILABLE_SHANTEN;
    }
    let to_tiles: [usize; 13] = [
        Tile::M1 as usize, Tile::M9 as usize,
        Tile::P1 as usize, Tile::P9 as usize,
        Tile::S1 as usize, Tile::S9 as usize,
        Tile::Z1 as usize, Tile::Z2 as usize, Tile::Z3 as usize,
        Tile::Z4 as usize, Tile::Z5 as usize, Tile::Z6 as usize, Tile::Z7 as usize,
    ];
    let t = hand.summarize_tiles();
    let mut pair: u32 = 0;
    let mut kind: u32 = 0;
    for &i in &to_tiles {
        if t[i] > 0 {
            kind += 1;
            if t[i] >= 2 {
                pair += 1;
            }
        }
    }
    (14 - kind - if pair > 0 { 1 } else { 0 }) as i32 - 1
}

// ============================================================================
// 共通シャンテン数計算エンジン
//
// ShantenAccumulator トレイトにより、ブロック分解を Vec で追跡する FullTracking と
// カウンタのみで追跡する CountOnly の2つのモードを、同一の再帰ロジックで実行する。
// Rust のモノモーフィゼーションにより CountOnly ではゼロコストで最適化される。
// ============================================================================

/// 前処理で抽出された独立ブロックの情報
trait PreprocessResult {
    fn same3_count(&self) -> usize;
    fn seq3_count(&self) -> usize;
}

/// シャンテン数計算中のブロック蓄積を抽象化するトレイト
trait ShantenAccumulator: Sized {
    type Preprocess: PreprocessResult;

    /// 前処理: 独立した刻子・順子・孤立牌を抽出する
    fn preprocess(t: &mut TileSummarize) -> Result<Self::Preprocess>;

    /// 新しい空の追跡状態を作成する
    fn new_tracking() -> Self;

    fn push_same3(&mut self, tile: usize);
    fn pop_same3(&mut self);
    fn same3_count(&self) -> usize;

    fn push_seq3(&mut self, tile: usize);
    fn pop_seq3(&mut self);
    fn seq3_count(&self) -> usize;

    fn push_same2(&mut self, tile: usize);
    fn pop_same2(&mut self);
    fn same2_count(&self) -> usize;

    fn push_seq2(&mut self, tile1: usize, tile2: usize);
    fn pop_seq2(&mut self);
    fn seq2_count(&self) -> usize;

    /// 新しい最良結果が見つかったときに呼ばれる。現在の状態をスナップショットする。
    fn snapshot_best(&self, pre: &Self::Preprocess, t: &TileSummarize, head: usize) -> Self;

    /// 最終結果に独立ブロックをマージする
    fn finalize(self, pre: Self::Preprocess) -> Self;
}

// --- CountOnly: カウンタのみ（高速版） ---

struct CountOnlyPreprocess {
    same3: usize,
    seq3: usize,
}

impl PreprocessResult for CountOnlyPreprocess {
    #[inline(always)]
    fn same3_count(&self) -> usize { self.same3 }
    #[inline(always)]
    fn seq3_count(&self) -> usize { self.seq3 }
}

struct CountOnly {
    same3: usize,
    seq3: usize,
    same2: usize,
    seq2: usize,
}

impl ShantenAccumulator for CountOnly {
    type Preprocess = CountOnlyPreprocess;

    fn preprocess(t: &mut TileSummarize) -> Result<CountOnlyPreprocess> {
        let same3 = extract_independent_same3(t);
        let seq3 = extract_independent_seq3(t);
        let _ = remove_independent_singles(t);
        Ok(CountOnlyPreprocess { same3, seq3 })
    }

    #[inline(always)]
    fn new_tracking() -> Self {
        CountOnly { same3: 0, seq3: 0, same2: 0, seq2: 0 }
    }

    #[inline(always)]
    fn push_same3(&mut self, _tile: usize) { self.same3 += 1; }
    #[inline(always)]
    fn pop_same3(&mut self) { self.same3 -= 1; }
    #[inline(always)]
    fn same3_count(&self) -> usize { self.same3 }

    #[inline(always)]
    fn push_seq3(&mut self, _tile: usize) { self.seq3 += 1; }
    #[inline(always)]
    fn pop_seq3(&mut self) { self.seq3 -= 1; }
    #[inline(always)]
    fn seq3_count(&self) -> usize { self.seq3 }

    #[inline(always)]
    fn push_same2(&mut self, _tile: usize) { self.same2 += 1; }
    #[inline(always)]
    fn pop_same2(&mut self) { self.same2 -= 1; }
    #[inline(always)]
    fn same2_count(&self) -> usize { self.same2 }

    #[inline(always)]
    fn push_seq2(&mut self, _tile1: usize, _tile2: usize) { self.seq2 += 1; }
    #[inline(always)]
    fn pop_seq2(&mut self) { self.seq2 -= 1; }
    #[inline(always)]
    fn seq2_count(&self) -> usize { self.seq2 }

    #[inline(always)]
    fn snapshot_best(&self, _pre: &CountOnlyPreprocess, _t: &TileSummarize, _head: usize) -> Self {
        // カウンタのみなのでスナップショット不要
        CountOnly { same3: 0, seq3: 0, same2: 0, seq2: 0 }
    }

    #[inline(always)]
    fn finalize(self, _pre: CountOnlyPreprocess) -> Self { self }
}

// --- FullTracking: Vec 追跡（役判定・符計算用） ---

struct FullTrackingPreprocess {
    same3: Vec<Same3>,
    seq3: Vec<Sequential3>,
    singles: Vec<TileType>,
}

impl PreprocessResult for FullTrackingPreprocess {
    fn same3_count(&self) -> usize { self.same3.len() }
    fn seq3_count(&self) -> usize { self.seq3.len() }
}

struct FullTracking {
    same3: Vec<Same3>,
    sequential3: Vec<Sequential3>,
    same2: Vec<Same2>,
    sequential2: Vec<Sequential2>,
    single: Vec<TileType>,
}

impl ShantenAccumulator for FullTracking {
    type Preprocess = FullTrackingPreprocess;

    fn preprocess(t: &mut TileSummarize) -> Result<FullTrackingPreprocess> {
        let same3 = extract_independent_same3_full(t)?;
        let seq3 = extract_independent_seq3_full(t)?;
        let singles = extract_independent_singles_full(t)?;
        Ok(FullTrackingPreprocess { same3, seq3, singles })
    }

    fn new_tracking() -> Self {
        FullTracking {
            same3: Vec::new(),
            sequential3: Vec::new(),
            same2: Vec::new(),
            sequential2: Vec::new(),
            single: Vec::new(),
        }
    }

    fn push_same3(&mut self, tile: usize) {
        self.same3.push(Same3::new(tile as TileType, tile as TileType, tile as TileType).unwrap());
    }
    fn pop_same3(&mut self) { self.same3.pop(); }
    fn same3_count(&self) -> usize { self.same3.len() }

    fn push_seq3(&mut self, tile: usize) {
        self.sequential3.push(Sequential3::new(
            tile as TileType, (tile + 1) as TileType, (tile + 2) as TileType,
        ).unwrap());
    }
    fn pop_seq3(&mut self) { self.sequential3.pop(); }
    fn seq3_count(&self) -> usize { self.sequential3.len() }

    fn push_same2(&mut self, tile: usize) {
        self.same2.push(Same2::new(tile as TileType, tile as TileType).unwrap());
    }
    fn pop_same2(&mut self) { self.same2.pop(); }
    fn same2_count(&self) -> usize { self.same2.len() }

    fn push_seq2(&mut self, tile1: usize, tile2: usize) {
        self.sequential2.push(Sequential2::new(tile1 as TileType, tile2 as TileType).unwrap());
    }
    fn pop_seq2(&mut self) { self.sequential2.pop(); }
    fn seq2_count(&self) -> usize { self.sequential2.len() }

    fn snapshot_best(&self, _pre: &FullTrackingPreprocess, t: &TileSummarize, _head: usize) -> Self {
        let mut single = Vec::new();
        for i in 0..Tile::LEN as usize {
            for _ in 0..t[i] {
                single.push(i as TileType);
            }
        }
        FullTracking {
            same3: self.same3.clone(),
            sequential3: self.sequential3.clone(),
            same2: self.same2.clone(),
            sequential2: self.sequential2.clone(),
            single,
        }
    }

    fn finalize(mut self, mut pre: FullTrackingPreprocess) -> Self {
        self.same3.append(&mut pre.same3);
        self.sequential3.append(&mut pre.seq3);
        self.single.append(&mut pre.singles);
        self
    }
}

// --- 共通再帰ロジック ---

/// 通常形のシャンテン数を計算する共通エントリポイント
fn calc_normal_shanten<A: ShantenAccumulator>(hand: &Hand) -> Result<(i32, A)> {
    let mut t = hand.summarize_tiles();
    let mut best = UNAVAILABLE_SHANTEN;

    let pre = A::preprocess(&mut t)?;
    let mut acc = A::new_tracking();
    let mut best_acc = A::new_tracking();

    // 雀頭を抜き出す
    for i in 0..Tile::LEN as usize {
        if t[i] >= 2 {
            t[i] -= 2;
            acc.push_same2(i);
            find_mentsu(0, &pre, &mut acc, 1, &mut t, &mut best, &mut best_acc);
            acc.pop_same2();
            t[i] += 2;
        }
    }
    // 雀頭なし
    find_mentsu(0, &pre, &mut acc, 0, &mut t, &mut best, &mut best_acc);

    let result = best_acc.finalize(pre);
    Ok((best, result))
}

/// フェーズ1: 面子（刻子・順子）を再帰的に抽出する
fn find_mentsu<A: ShantenAccumulator>(
    idx: usize,
    pre: &A::Preprocess,
    acc: &mut A,
    head: usize,
    t: &mut TileSummarize,
    best: &mut i32,
    best_acc: &mut A,
) {
    for i in idx..Tile::LEN as usize {
        // 刻子
        if t[i] >= 3 {
            t[i] -= 3;
            acc.push_same3(i);
            find_mentsu(i, pre, acc, head, t, best, best_acc);
            acc.pop_same3();
            t[i] += 3;
        }
        // 順子
        if i < 27 && i % 9 <= 6 && t[i] >= 1 && t[i + 1] >= 1 && t[i + 2] >= 1 {
            t[i] -= 1;
            t[i + 1] -= 1;
            t[i + 2] -= 1;
            acc.push_seq3(i);
            find_mentsu(i, pre, acc, head, t, best, best_acc);
            acc.pop_seq3();
            t[i] += 1;
            t[i + 1] += 1;
            t[i + 2] += 1;
        }
    }

    // 面子を全て抽出し終えたら、塔子・対子の探索に移行する。
    // 面子抽出後の残り牌は元のインデックスより前に存在し得るため、常に先頭から探索する。
    let block3 = pre.same3_count() + pre.seq3_count() + acc.same3_count() + acc.seq3_count();
    find_tatsu(0, block3, head, pre, acc, t, best, best_acc);
}

/// フェーズ2: 塔子（対子・両面/辺張・嵌張）を再帰的に抽出する
fn find_tatsu<A: ShantenAccumulator>(
    idx: usize,
    block3: usize,
    head: usize,
    pre: &A::Preprocess,
    acc: &mut A,
    t: &mut TileSummarize,
    best: &mut i32,
    best_acc: &mut A,
) {
    // 現在の分解で向聴数を計算
    let block2_raw = acc.same2_count() + acc.seq2_count();
    // 雀頭として使っている same2 は block2 に含めない
    let block2_net = block2_raw.saturating_sub(head);
    let block2_capped = block2_net.min(4usize.saturating_sub(block3));
    let shanten = 8i32 - (block3 * 2 + block2_capped + head) as i32;
    if shanten < *best {
        *best = shanten;
        *best_acc = acc.snapshot_best(pre, t, head);
    }

    // 枝刈り: これ以上 block2 を増やしても改善しない場合
    if block2_net >= 4usize.saturating_sub(block3) {
        return;
    }

    for i in idx..Tile::LEN as usize {
        // 対子
        if t[i] >= 2 {
            t[i] -= 2;
            acc.push_same2(i);
            find_tatsu(i + 1, block3, head, pre, acc, t, best, best_acc);
            acc.pop_same2();
            t[i] += 2;
        }
        // 塔子（隣接する2枚）
        if i < 27 && i % 9 <= 7 && t[i] >= 1 && t[i + 1] >= 1 {
            t[i] -= 1;
            t[i + 1] -= 1;
            acc.push_seq2(i, i + 1);
            find_tatsu(i, block3, head, pre, acc, t, best, best_acc);
            acc.pop_seq2();
            t[i] += 1;
            t[i + 1] += 1;
        }
        // 嵌張（間が空いた2枚）
        if i < 27 && i % 9 <= 6 && t[i] >= 1 && t[i + 1] == 0 && t[i + 2] >= 1 {
            t[i] -= 1;
            t[i + 2] -= 1;
            acc.push_seq2(i, i + 2);
            find_tatsu(i, block3, head, pre, acc, t, best, best_acc);
            acc.pop_seq2();
            t[i] += 1;
            t[i + 2] += 1;
        }
    }
}

// ============================================================================
// 前処理: 独立ブロック抽出
// ============================================================================

/// 数牌において、隣接2マス以内に他の牌がないかを判定する
fn is_isolated(t: &TileSummarize, i: usize) -> bool {
    if i >= 27 {
        return true; // 字牌は常に独立
    }
    let pos = i % 9;
    let base = i - pos;
    let left2  = pos < 2 || t[base + pos - 2] == 0;
    let left1  = pos < 1 || t[base + pos - 1] == 0;
    let right1 = pos > 7 || t[base + pos + 1] == 0;
    let right2 = pos > 6 || t[base + pos + 2] == 0;
    left2 && left1 && right1 && right2
}

/// 独立した刻子を抽出する（カウントのみ返す）
fn extract_independent_same3(t: &mut TileSummarize) -> usize {
    let mut count = 0;
    for i in 0..Tile::LEN as usize {
        if t[i] >= 3 && is_isolated(t, i) {
            t[i] -= 3;
            count += 1;
        }
    }
    count
}

/// 独立した刻子を抽出する（Vec で返す）
fn extract_independent_same3_full(t: &mut TileSummarize) -> Result<Vec<Same3>> {
    let mut result = Vec::new();
    for i in 0..Tile::LEN as usize {
        if t[i] >= 3 && is_isolated(t, i) {
            t[i] -= 3;
            let tile = i as TileType;
            result.push(Same3::new(tile, tile, tile)?);
        }
    }
    Ok(result)
}

/// 独立した順子を抽出する（共通ロジック）
///
/// 一盃口を先に処理してから通常処理する。
/// `on_found` は見つかった順子の先頭インデックスと個数（1 or 2）を受け取る。
fn extract_independent_seq3_impl(
    t: &mut TileSummarize,
    mut on_found: impl FnMut(usize, u32),
) {
    for n in (1u32..=2).rev() {
        for suit_start in (0..27).step_by(9) {
            for k in 0..=6usize {
                let l = suit_start + k;
                if k >= 2 && t[l - 2] > 0 { continue; }
                if k >= 1 && t[l - 1] > 0 { continue; }
                if k <= 5 && t[l + 3] > 0 { continue; }
                if k <= 4 && t[l + 4] > 0 { continue; }
                if t[l] == n && t[l + 1] == n && t[l + 2] == n {
                    t[l] -= n;
                    t[l + 1] -= n;
                    t[l + 2] -= n;
                    on_found(l, n);
                }
            }
        }
    }
}

/// 独立した順子を抽出する（カウントのみ返す）
fn extract_independent_seq3(t: &mut TileSummarize) -> usize {
    let mut count = 0usize;
    extract_independent_seq3_impl(t, |_l, n| {
        count += n as usize;
    });
    count
}

/// 独立した順子を抽出する（Vec で返す）
fn extract_independent_seq3_full(t: &mut TileSummarize) -> Result<Vec<Sequential3>> {
    let mut result = Vec::new();
    let mut err: Option<anyhow::Error> = None;
    extract_independent_seq3_impl(t, |l, n| {
        if err.is_some() { return; }
        for _ in 0..n {
            match Sequential3::new(l as TileType, (l + 1) as TileType, (l + 2) as TileType) {
                Ok(s) => result.push(s),
                Err(e) => { err = Some(e); return; }
            }
        }
    });
    if let Some(e) = err { return Err(e); }
    Ok(result)
}

/// 独立した孤立牌を除去する（カウントのみ返す）
fn remove_independent_singles(t: &mut TileSummarize) -> usize {
    let mut count = 0;
    for i in 0..Tile::LEN as usize {
        if t[i] == 1 && is_isolated(t, i) {
            t[i] -= 1;
            count += 1;
        }
    }
    count
}

/// 独立した孤立牌を除去する（Vec で返す）
fn extract_independent_singles_full(t: &mut TileSummarize) -> Result<Vec<TileType>> {
    let mut result = Vec::new();
    for i in 0..Tile::LEN as usize {
        if t[i] == 1 && is_isolated(t, i) {
            t[i] -= 1;
            result.push(i as TileType);
        }
    }
    Ok(result)
}

/// ユニットテスト
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// 七対子を聴牌
    fn zero_shanten_to_seven_pairs() {
        let test_str = "226699m99p228s66z 1z";
        let test = Hand::from(test_str);
        assert_eq!(
            HandAnalyzer::new_by_form(&test, Form::SevenPairs)
                .unwrap()
                .shanten,
            0
        );
    }
    #[test]
    /// 同じ牌が3枚ある状態で七対子を聴牌
    fn zero_shanten_to_seven_pairs_2() {
        let test_str = "226699m99p222s66z 1z";
        let test = Hand::from(test_str);
        assert_eq!(
            HandAnalyzer::new_by_form(&test, Form::SevenPairs)
                .unwrap()
                .shanten,
            0
        );
    }
    #[test]
    /// 国士無双を聴牌
    fn zero_shanten_to_orphans() {
        let test_str = "19m19p11s1234567z 5m";
        let test = Hand::from(test_str);
        assert_eq!(
            HandAnalyzer::new_by_form(&test, Form::ThirteenOrphans)
                .unwrap()
                .shanten,
            0
        );
    }

    #[test]
    /// 同じ牌が4枚ある状態で七対子は認められない（一向聴とみなす）
    fn seven_pairs_with_4_same_tiles() {
        let test_str = "1122m3344p5555s1z 1z";
        let test = Hand::from(test_str);
        assert_eq!(
            HandAnalyzer::new_by_form(&test, Form::SevenPairs)
                .unwrap()
                .shanten,
            1
        );
    }

    #[test]
    /// 立直で和了った
    fn win_by_ready_hand() {
        let test_str = "123m444p789s1112z 2z";
        let test = Hand::from(test_str);
        assert_eq!(
            HandAnalyzer::new_by_form(&test, Form::Normal)
                .unwrap()
                .shanten,
            -1
        );
    }

    #[test]
    /// 自風牌で和了った
    fn win_by_honor_tiles_players_wind() {
        let test_str = "333m456p1789s 333z 1s";
        let test = Hand::from(test_str);
        assert_eq!(
            HandAnalyzer::new_by_form(&test, Form::Normal)
                .unwrap()
                .shanten,
            -1
        );
    }

    #[test]
    /// 場風で和了った
    fn win_by_honor_tiles_prevailing_wind() {
        let test_str = "234567m6789s 111z 6s";
        let test = Hand::from(test_str);
        assert_eq!(
            HandAnalyzer::new_by_form(&test, Form::Normal)
                .unwrap()
                .shanten,
            -1
        );
    }
    #[test]
    /// 三元牌で和了った
    fn win_by_honor_tiles_dragons() {
        let test_str = "5m123456p888s 777z 5m";
        let test = Hand::from(test_str);
        assert_eq!(
            HandAnalyzer::new_by_form(&test, Form::Normal)
                .unwrap()
                .shanten,
            -1
        );
    }
    #[test]
    /// 断么九で和了った
    fn win_by_all_simples() {
        let test_str = "234m8s 567m 333p 456s 8s";
        let test = Hand::from(test_str);
        assert_eq!(
            HandAnalyzer::new_by_form(&test, Form::Normal)
                .unwrap()
                .shanten,
            -1
        );
    }

    #[test]
    /// 平和で和了った
    fn win_by_no_points() {
        let test_str = "123567m234p6799s 5s";
        let test = Hand::from(test_str);
        assert_eq!(
            HandAnalyzer::new_by_form(&test, Form::Normal)
                .unwrap()
                .shanten,
            -1
        );
    }

    #[test]
    /// 55m123567p56789s + ツモ9m → 聴牌（シャンテン数0）
    fn tenpai_with_89_wait() {
        let test_str = "55m123567p56789s 9m";
        let test = Hand::from(test_str);
        assert_eq!(HandAnalyzer::new(&test).unwrap().shanten, 0);
    }

    #[test]
    /// 89sの塔子を含む聴牌
    fn tenpai_with_89s_toitsu() {
        let test_str = "11m234p567p234s89s 1z";
        let test = Hand::from(test_str);
        assert_eq!(HandAnalyzer::new(&test).unwrap().shanten, 0);
    }

    #[test]
    /// 89mの塔子を含む聴牌
    fn tenpai_with_89m_toitsu() {
        let test_str = "89m11p234p567s234s 2z";
        let test = Hand::from(test_str);
        assert_eq!(HandAnalyzer::new(&test).unwrap().shanten, 0);
    }

    #[test]
    /// 4面子1塔子は和了ではなく聴牌
    fn four_melds_and_one_taatsu_is_tenpai_not_win() {
        let test = Hand::from("234678m56p567s55z 5z");
        assert_eq!(HandAnalyzer::new(&test).unwrap().shanten, 0);
    }

    #[test]
    fn kan_hand_with_unrelated_rinshan_tile_is_not_a_win() {
        let test = Hand::from("567p123s678s8s 5555s 1m");
        assert_eq!(HandAnalyzer::new(&test).unwrap().shanten, 0);
    }

    #[test]
    fn opened_hand_cannot_be_seven_pairs_or_thirteen_orphans() {
        let test = Hand::from("123456789m11p 789s 1p");
        assert!(HandAnalyzer::new_by_form(&test, Form::SevenPairs).unwrap().shanten > 0);
        assert!(HandAnalyzer::new_by_form(&test, Form::ThirteenOrphans).unwrap().shanten > 0);
    }

    /// shanten_number() が HandAnalyzer::new() と同じ結果を返すことを検証
    #[rstest::rstest]
    #[case::seven_pairs_tenpai("226699m99p228s66z 1z")]
    #[case::thirteen_orphans_tenpai("19m19p11s1234567z 5m")]
    #[case::normal_win_triplets("123m444p789s1112z 2z")]
    #[case::normal_win_flush("222333444666s6z 6z")]
    #[case::normal_win_nine_gates("1112345678999m 5m")]
    #[case::seven_pairs_win("1122m3344p5566s7z 7z")]
    #[case::thirteen_orphans_win("19m19p19s1234567z 1m")]
    #[case::normal_tenpai_13_tiles("123m456p789s1234z")]
    #[case::far_from_ready("147m258p369s1234z")]
    #[case::with_open_melds("333m456p1789s 333z 1s")]
    #[case::leftover_tatsu_at_lower_index("23444p22334567s")]
    #[case::leftover_tatsu_at_lower_index_with_drawn("23444p22334567s 1z")]
    fn shanten_number_matches_full_analyzer(#[case] hand_str: &str) {
        let hand = Hand::from(hand_str);
        let full = HandAnalyzer::new(&hand).unwrap().shanten;
        let fast = shanten_number(&hand);
        assert_eq!(full, fast, "shanten mismatch for hand '{hand_str}': full={full}, fast={fast}");
    }
}
