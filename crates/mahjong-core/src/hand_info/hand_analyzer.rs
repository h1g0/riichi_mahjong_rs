use anyhow::Result;

use std::cmp::*;
use std::fmt;

use crate::hand::Hand;
use crate::hand_info::block::*;
use crate::tile::*;
use crate::winning_hand::name::Form;

/// Shanten number (向聴数): how many tile exchanges away from tenpai.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ShantenNumber(i32);

impl ShantenNumber {
    /// Not applicable, e.g. Seven Pairs / Thirteen Orphans with an open hand.
    const UNAVAILABLE: ShantenNumber = ShantenNumber(i32::MAX);

    /// The hand is complete (shanten == -1).
    pub fn has_won(&self) -> bool {
        self.0 == -1
    }

    /// The hand is tenpai (shanten == 0).
    pub fn is_ready(&self) -> bool {
        self.0 == 0
    }

    /// The hand is tenpai or complete (shanten <= 0).
    pub fn is_ready_or_won(&self) -> bool {
        self.0 <= 0
    }

    /// Returns the raw `i32` value.
    pub fn as_i32(&self) -> i32 {
        self.0
    }
}

impl PartialEq<i32> for ShantenNumber {
    fn eq(&self, other: &i32) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<i32> for ShantenNumber {
    fn partial_cmp(&self, other: &i32) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

impl fmt::Display for ShantenNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The block decomposition of a hand that minimizes its shanten number.
///
/// For the normal form and Seven Pairs the groups/pairs are stored in the
/// Vecs below; for Thirteen Orphans only the shanten number is meaningful.
#[derive(Debug, Eq)]
pub struct HandAnalyzer {
    /// Shanten number
    pub shanten: ShantenNumber,
    /// Which winning form the decomposition targets
    pub form: Form,
    /// Triplets (kōtsu / 刻子)
    pub same3: Vec<Same3>,
    /// Sequences (shuntsu / 順子)
    pub sequential3: Vec<Sequential3>,
    /// Pairs (toitsu / 対子)
    pub same2: Vec<Same2>,
    /// Partial sequences: two adjacent tiles or a gapped pair (塔子・嵌張)
    pub sequential2: Vec<Sequential2>,
    /// Tiles that belong to no block
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
            shanten: ShantenNumber::UNAVAILABLE,
            form,
            same3: Vec::new(),
            sequential3: Vec::new(),
            same2: Vec::new(),
            sequential2: Vec::new(),
            single: Vec::new(),
        }
    }

    /// Computes the shanten number for each of the three winning forms
    /// (Seven Pairs, Thirteen Orphans, normal) and returns the minimum.
    /// # Examples
    ///
    /// ```
    /// use mahjong_core::hand::*;
    /// use mahjong_core::hand_info::hand_analyzer::*;
    /// use mahjong_core::winning_hand::name::*;
    ///
    /// // Winning with the normal form
    /// let nm_test_str = "222333444666s6z 6z";
    /// let nm_test = Hand::from(nm_test_str);
    /// let analyzer = HandAnalyzer::new(&nm_test).unwrap();
    /// assert!(analyzer.shanten.has_won());
    /// assert_eq!(
    ///   analyzer.form,
    ///   Form::Normal
    /// );
    /// ```
    pub fn new(hand: &Hand) -> Result<HandAnalyzer> {
        let sp = HandAnalyzer::new_by_form(hand, Form::SevenPairs)?;
        let to = HandAnalyzer::new_by_form(hand, Form::ThirteenOrphans)?;
        let normal = HandAnalyzer::new_by_form(hand, Form::Normal)?;
        // Highest-value interpretation: prefer the normal form on a win.
        // E.g. Double Twin Sequences (3 han) outscores the same tiles read
        // as Seven Pairs (2 han).
        if normal.shanten.has_won() {
            Ok(normal)
        } else {
            Ok(min(min(sp, to), normal))
        }
    }

    /// Computes the shanten number for one specific winning form.
    /// # Examples
    ///
    /// ```
    /// use mahjong_core::hand::*;
    /// use mahjong_core::hand_info::hand_analyzer::*;
    /// use mahjong_core::winning_hand::name::*;
    ///
    /// // Winning with Thirteen Orphans
    /// let to_test_str = "19m19p19s1234567z 1m";
    /// let to_test = Hand::from(to_test_str);
    /// assert!(HandAnalyzer::new_by_form(&to_test, Form::ThirteenOrphans).unwrap().shanten.has_won());
    ///
    /// // Winning with Seven Pairs
    /// let sp_test_str = "1122m3344p5566s7z 7z";
    /// let sp_test = Hand::from(sp_test_str);
    /// assert!(HandAnalyzer::new_by_form(&sp_test, Form::SevenPairs).unwrap().shanten.has_won());
    ///
    /// // Winning with the normal form
    /// let nm_test_str = "1112345678999m 5m";
    /// let nm_test = Hand::from(nm_test_str);
    /// assert!(HandAnalyzer::new_by_form(&nm_test, Form::Normal).unwrap().shanten.has_won());
    /// ```
    pub fn new_by_form(hand: &Hand, form: Form) -> Result<HandAnalyzer> {
        Ok(match form {
            Form::SevenPairs => HandAnalyzer::analyze_seven_pairs(hand)?,
            Form::ThirteenOrphans => HandAnalyzer::analyze_thirteen_orphans(hand)?,
            Form::Normal => HandAnalyzer::analyze_normal_form(hand)?,
        })
    }

    /// Shanten and block decomposition towards Seven Pairs.
    ///
    /// Everything except pairs goes into `single`. The decomposition is still
    /// needed because Seven Pairs can combine with block-based yaku such as
    /// All Inside, Common Terminals, Common Flush, and Perfect Flush.
    fn analyze_seven_pairs(hand: &Hand) -> Result<HandAnalyzer> {
        if !hand.melds().is_empty() {
            return Ok(HandAnalyzer::unavailable(Form::SevenPairs));
        }

        let mut t = hand.summarize_tiles();
        let (shanten_raw, _pair_count) = calc_seven_pairs_shanten(&t);

        let mut same2: Vec<Same2> = Vec::new();
        for (i, count) in t.iter_mut().enumerate().take(Tile::LEN) {
            if *count >= 2 {
                same2.push(Same2::new(i as TileType, i as TileType)?);
                *count -= 2;
            }
        }
        let mut single: Vec<TileType> = Vec::new();
        for (i, &count) in t.iter().enumerate().take(Tile::LEN) {
            for _ in 0..count {
                single.push(i as TileType);
            }
        }
        Ok(HandAnalyzer {
            shanten: ShantenNumber(shanten_raw),
            form: Form::SevenPairs,
            same3: Vec::new(),
            sequential3: Vec::new(),
            same2,
            sequential2: Vec::new(),
            single,
        })
    }

    /// Shanten towards Thirteen Orphans.
    ///
    /// No block decomposition: the form has no groups to decompose into.
    fn analyze_thirteen_orphans(hand: &Hand) -> Result<HandAnalyzer> {
        if !hand.melds().is_empty() {
            return Ok(HandAnalyzer::unavailable(Form::ThirteenOrphans));
        }

        let t = hand.summarize_tiles();
        let shanten_raw = calc_thirteen_orphans_shanten(&t);
        Ok(HandAnalyzer {
            shanten: ShantenNumber(shanten_raw),
            form: Form::ThirteenOrphans,
            same3: Vec::new(),
            sequential3: Vec::new(),
            same2: Vec::new(),
            sequential2: Vec::new(),
            single: Vec::new(),
        })
    }

    /// Shanten and block decomposition for the normal form.
    fn analyze_normal_form(hand: &Hand) -> Result<HandAnalyzer> {
        let (shanten_raw, tracking) = calc_normal_shanten::<FullTracking>(hand)?;
        let FullTracking {
            same3,
            sequential3,
            same2,
            sequential2,
            single,
        } = tracking;
        Ok(HandAnalyzer {
            shanten: ShantenNumber(shanten_raw),
            form: Form::Normal,
            same3,
            sequential3,
            same2,
            sequential2,
            single,
        })
    }
}

/// Computes only the shanten number, quickly.
///
/// Returns the same value as `HandAnalyzer::new().shanten` but skips the
/// block decomposition and Vec bookkeeping, for hot paths such as the CPU
/// discard evaluation.
pub fn calc_shanten_number(hand: &Hand) -> ShantenNumber {
    let t = hand.summarize_tiles();
    let is_closed = hand.melds().is_empty();
    let sp = if is_closed {
        calc_seven_pairs_shanten(&t).0
    } else {
        i32::MAX
    };
    let to = if is_closed {
        calc_thirteen_orphans_shanten(&t)
    } else {
        i32::MAX
    };
    let nm = calc_normal_shanten::<CountOnly>(hand)
        .map(|(s, _)| s)
        .unwrap_or(i32::MAX);
    ShantenNumber(min(min(sp, to), nm))
}

/// Computes only the shanten number for one winning form, quickly.
///
/// Returns the same value as `HandAnalyzer::new_by_form(hand, form).shanten`
/// but skips the block decomposition, for hot paths such as the CPU's
/// form comparison (normal vs Seven Pairs vs Thirteen Orphans).
///
/// With an open hand, Seven Pairs and Thirteen Orphans return
/// the unavailable sentinel.
pub fn calc_shanten_number_by_form(hand: &Hand, form: Form) -> ShantenNumber {
    let is_closed = hand.melds().is_empty();
    match form {
        Form::SevenPairs => {
            if is_closed {
                let t = hand.summarize_tiles();
                ShantenNumber(calc_seven_pairs_shanten(&t).0)
            } else {
                ShantenNumber::UNAVAILABLE
            }
        }
        Form::ThirteenOrphans => {
            if is_closed {
                let t = hand.summarize_tiles();
                ShantenNumber(calc_thirteen_orphans_shanten(&t))
            } else {
                ShantenNumber::UNAVAILABLE
            }
        }
        Form::Normal => calc_normal_shanten::<CountOnly>(hand)
            .map(|(s, _)| ShantenNumber(s))
            .unwrap_or(ShantenNumber::UNAVAILABLE),
    }
}

/// Core Seven Pairs shanten computation.
///
/// Returns `(shanten, pair_count)`.
fn calc_seven_pairs_shanten(t: &TileSummarize) -> (i32, u32) {
    let mut pair: u32 = 0;
    let mut kind: u32 = 0;
    for &count in t.iter().take(Tile::LEN) {
        if count > 0 {
            kind += 1;
            if count >= 2 {
                pair += 1;
            }
        }
    }
    let shanten = (7 - pair + 7_u32.saturating_sub(kind)) as i32 - 1;
    (shanten, pair)
}

/// Core Thirteen Orphans shanten computation.
fn calc_thirteen_orphans_shanten(t: &TileSummarize) -> i32 {
    const TO_TILES: [usize; 13] = [
        Tile::M1 as usize,
        Tile::M9 as usize,
        Tile::P1 as usize,
        Tile::P9 as usize,
        Tile::S1 as usize,
        Tile::S9 as usize,
        Tile::Z1 as usize,
        Tile::Z2 as usize,
        Tile::Z3 as usize,
        Tile::Z4 as usize,
        Tile::Z5 as usize,
        Tile::Z6 as usize,
        Tile::Z7 as usize,
    ];
    let mut pair: u32 = 0;
    let mut kind: u32 = 0;
    for &i in &TO_TILES {
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
// Shared shanten computation engine.
//
// The ShantenAccumulator trait lets the same recursive search run in two
// modes: FullTracking records the block decomposition in Vecs, CountOnly
// keeps bare counters. Monomorphization makes CountOnly effectively free.
// ============================================================================

/// Independent blocks extracted by preprocessing.
trait PreprocessResult {
    fn same3_count(&self) -> usize;
    fn seq3_count(&self) -> usize;
}

/// Abstracts block bookkeeping during the shanten search.
trait ShantenAccumulator: Sized {
    type Preprocess: PreprocessResult;

    /// Preprocessing: pull out independent triplets, sequences, and isolated tiles.
    fn preprocess(t: &mut TileSummarize) -> Result<Self::Preprocess>;

    /// Creates an empty tracking state.
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

    /// Called when a new best result is found; snapshots the current state.
    fn snapshot_best(&self, pre: &Self::Preprocess, t: &TileSummarize, head: usize) -> Self;

    /// Merges the preprocessed independent blocks into the final result.
    fn finalize(self, pre: Self::Preprocess) -> Self;
}

// Fast counter-only mode.
struct CountOnlyPreprocess {
    same3: usize,
    seq3: usize,
}

impl PreprocessResult for CountOnlyPreprocess {
    #[inline(always)]
    fn same3_count(&self) -> usize {
        self.same3
    }
    #[inline(always)]
    fn seq3_count(&self) -> usize {
        self.seq3
    }
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
        CountOnly {
            same3: 0,
            seq3: 0,
            same2: 0,
            seq2: 0,
        }
    }

    #[inline(always)]
    fn push_same3(&mut self, _tile: usize) {
        self.same3 += 1;
    }
    #[inline(always)]
    fn pop_same3(&mut self) {
        self.same3 -= 1;
    }
    #[inline(always)]
    fn same3_count(&self) -> usize {
        self.same3
    }

    #[inline(always)]
    fn push_seq3(&mut self, _tile: usize) {
        self.seq3 += 1;
    }
    #[inline(always)]
    fn pop_seq3(&mut self) {
        self.seq3 -= 1;
    }
    #[inline(always)]
    fn seq3_count(&self) -> usize {
        self.seq3
    }

    #[inline(always)]
    fn push_same2(&mut self, _tile: usize) {
        self.same2 += 1;
    }
    #[inline(always)]
    fn pop_same2(&mut self) {
        self.same2 -= 1;
    }
    #[inline(always)]
    fn same2_count(&self) -> usize {
        self.same2
    }

    #[inline(always)]
    fn push_seq2(&mut self, _tile1: usize, _tile2: usize) {
        self.seq2 += 1;
    }
    #[inline(always)]
    fn pop_seq2(&mut self) {
        self.seq2 -= 1;
    }
    #[inline(always)]
    fn seq2_count(&self) -> usize {
        self.seq2
    }

    #[inline(always)]
    fn snapshot_best(&self, _pre: &CountOnlyPreprocess, _t: &TileSummarize, _head: usize) -> Self {
        // Counters carry no state worth snapshotting.
        CountOnly {
            same3: 0,
            seq3: 0,
            same2: 0,
            seq2: 0,
        }
    }

    #[inline(always)]
    fn finalize(self, _pre: CountOnlyPreprocess) -> Self {
        self
    }
}

// Full mode: records every block in Vecs for yaku and fu evaluation.

struct FullTrackingPreprocess {
    same3: Vec<Same3>,
    seq3: Vec<Sequential3>,
    singles: Vec<TileType>,
}

impl PreprocessResult for FullTrackingPreprocess {
    fn same3_count(&self) -> usize {
        self.same3.len()
    }
    fn seq3_count(&self) -> usize {
        self.seq3.len()
    }
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
        Ok(FullTrackingPreprocess {
            same3,
            seq3,
            singles,
        })
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
        self.same3
            .push(Same3::new(tile as TileType, tile as TileType, tile as TileType).unwrap());
    }
    fn pop_same3(&mut self) {
        self.same3.pop();
    }
    fn same3_count(&self) -> usize {
        self.same3.len()
    }

    fn push_seq3(&mut self, tile: usize) {
        self.sequential3.push(
            Sequential3::new(
                tile as TileType,
                (tile + 1) as TileType,
                (tile + 2) as TileType,
            )
            .unwrap(),
        );
    }
    fn pop_seq3(&mut self) {
        self.sequential3.pop();
    }
    fn seq3_count(&self) -> usize {
        self.sequential3.len()
    }

    fn push_same2(&mut self, tile: usize) {
        self.same2
            .push(Same2::new(tile as TileType, tile as TileType).unwrap());
    }
    fn pop_same2(&mut self) {
        self.same2.pop();
    }
    fn same2_count(&self) -> usize {
        self.same2.len()
    }

    fn push_seq2(&mut self, tile1: usize, tile2: usize) {
        self.sequential2
            .push(Sequential2::new(tile1 as TileType, tile2 as TileType).unwrap());
    }
    fn pop_seq2(&mut self) {
        self.sequential2.pop();
    }
    fn seq2_count(&self) -> usize {
        self.sequential2.len()
    }

    fn snapshot_best(
        &self,
        _pre: &FullTrackingPreprocess,
        t: &TileSummarize,
        _head: usize,
    ) -> Self {
        let mut single = Vec::new();
        for (i, &count) in t.iter().enumerate().take(Tile::LEN) {
            for _ in 0..count {
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

/// Entry point for the normal-form shanten search.
fn calc_normal_shanten<A: ShantenAccumulator>(hand: &Hand) -> Result<(i32, A)> {
    let mut t = hand.summarize_tiles();
    let mut best = i32::MAX;

    let pre = A::preprocess(&mut t)?;
    let mut acc = A::new_tracking();
    let mut best_acc = A::new_tracking();

    // Try each candidate pair as the head.
    for i in 0..Tile::LEN {
        if t[i] >= 2 {
            t[i] -= 2;
            acc.push_same2(i);
            find_mentsu(0, &pre, &mut acc, 1, &mut t, &mut best, &mut best_acc);
            acc.pop_same2();
            t[i] += 2;
        }
    }
    // Also try with no head.
    find_mentsu(0, &pre, &mut acc, 0, &mut t, &mut best, &mut best_acc);

    let result = best_acc.finalize(pre);
    Ok((best, result))
}

/// Phase 1: recursively extract groups (triplets and sequences).
fn find_mentsu<A: ShantenAccumulator>(
    idx: usize,
    pre: &A::Preprocess,
    acc: &mut A,
    head: usize,
    t: &mut TileSummarize,
    best: &mut i32,
    best_acc: &mut A,
) {
    for i in idx..Tile::LEN {
        if t[i] >= 3 {
            t[i] -= 3;
            acc.push_same3(i);
            find_mentsu(i, pre, acc, head, t, best, best_acc);
            acc.pop_same3();
            t[i] += 3;
        }
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

    // With all groups extracted, move on to partial sequences and pairs.
    // Leftover tiles can sit below the current index, so restart from 0.
    let block3 = pre.same3_count() + pre.seq3_count() + acc.same3_count() + acc.seq3_count();
    let mut ctx = TatsuSearch {
        block3,
        head,
        pre,
        best,
        best_acc,
    };
    find_tatsu(0, &mut ctx, acc, t);
}

/// Phase 2: recursively extract pairs and partial sequences.
struct TatsuSearch<'a, A: ShantenAccumulator> {
    block3: usize,
    head: usize,
    pre: &'a A::Preprocess,
    best: &'a mut i32,
    best_acc: &'a mut A,
}

fn find_tatsu<A: ShantenAccumulator>(
    idx: usize,
    ctx: &mut TatsuSearch<'_, A>,
    acc: &mut A,
    t: &mut TileSummarize,
) {
    // Score the current decomposition.
    let block2_raw = acc.same2_count() + acc.seq2_count();
    // The pair used as the head must not count as a block.
    let block2_net = block2_raw.saturating_sub(ctx.head);
    let block2_capped = block2_net.min(4usize.saturating_sub(ctx.block3));
    let shanten = 8i32 - (ctx.block3 * 2 + block2_capped + ctx.head) as i32;
    if shanten < *ctx.best {
        *ctx.best = shanten;
        *ctx.best_acc = acc.snapshot_best(ctx.pre, t, ctx.head);
    }

    // Prune: more partial blocks cannot improve the result.
    if block2_net >= 4usize.saturating_sub(ctx.block3) {
        return;
    }

    for i in idx..Tile::LEN {
        if t[i] >= 2 {
            t[i] -= 2;
            acc.push_same2(i);
            find_tatsu(i + 1, ctx, acc, t);
            acc.pop_same2();
            t[i] += 2;
        }
        if i < 27 && i % 9 <= 7 && t[i] >= 1 && t[i + 1] >= 1 {
            t[i] -= 1;
            t[i + 1] -= 1;
            acc.push_seq2(i, i + 1);
            find_tatsu(i, ctx, acc, t);
            acc.pop_seq2();
            t[i] += 1;
            t[i + 1] += 1;
        }
        // Gapped pair (kanchan shape).
        if i < 27 && i % 9 <= 6 && t[i] >= 1 && t[i + 1] == 0 && t[i + 2] >= 1 {
            t[i] -= 1;
            t[i + 2] -= 1;
            acc.push_seq2(i, i + 2);
            find_tatsu(i, ctx, acc, t);
            acc.pop_seq2();
            t[i] += 1;
            t[i + 2] += 1;
        }
    }
}

// ============================================================================
// Preprocessing: independent block extraction.
// ============================================================================

/// Whether no other tile sits within two ranks of this suit tile.
fn is_isolated(t: &TileSummarize, i: usize) -> bool {
    if i >= 27 {
        return true; // Honours cannot form sequences, so they are always isolated.
    }
    let pos = i % 9;
    let base = i - pos;
    let left2 = pos < 2 || t[base + pos - 2] == 0;
    let left1 = pos < 1 || t[base + pos - 1] == 0;
    let right1 = pos > 7 || t[base + pos + 1] == 0;
    let right2 = pos > 6 || t[base + pos + 2] == 0;
    left2 && left1 && right1 && right2
}

/// Extracts independent triplets (count only).
fn extract_independent_same3(t: &mut TileSummarize) -> usize {
    let mut count = 0;
    for i in 0..Tile::LEN {
        if t[i] >= 3 && is_isolated(t, i) {
            t[i] -= 3;
            count += 1;
        }
    }
    count
}

/// Extracts independent triplets (as a Vec).
fn extract_independent_same3_full(t: &mut TileSummarize) -> Result<Vec<Same3>> {
    let mut result = Vec::new();
    for i in 0..Tile::LEN {
        if t[i] >= 3 && is_isolated(t, i) {
            t[i] -= 3;
            let tile = i as TileType;
            result.push(Same3::new(tile, tile, tile)?);
        }
    }
    Ok(result)
}

/// Extracts independent sequences (shared logic).
///
/// Doubled sequences (iipeikō shape) are handled before single ones.
/// `on_found` receives the starting index and the multiplicity (1 or 2).
fn extract_independent_seq3_impl(t: &mut TileSummarize, mut on_found: impl FnMut(usize, u32)) {
    for n in (1u32..=2).rev() {
        for suit_start in (0..27).step_by(9) {
            for k in 0..=6usize {
                let l = suit_start + k;
                if k >= 2 && t[l - 2] > 0 {
                    continue;
                }
                if k >= 1 && t[l - 1] > 0 {
                    continue;
                }
                if k <= 5 && t[l + 3] > 0 {
                    continue;
                }
                if k <= 4 && t[l + 4] > 0 {
                    continue;
                }
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

/// Extracts independent sequences (count only).
fn extract_independent_seq3(t: &mut TileSummarize) -> usize {
    let mut count = 0usize;
    extract_independent_seq3_impl(t, |_l, n| {
        count += n as usize;
    });
    count
}

/// Extracts independent sequences (as a Vec).
fn extract_independent_seq3_full(t: &mut TileSummarize) -> Result<Vec<Sequential3>> {
    let mut result = Vec::new();
    let mut err: Option<anyhow::Error> = None;
    extract_independent_seq3_impl(t, |l, n| {
        if err.is_some() {
            return;
        }
        for _ in 0..n {
            match Sequential3::new(l as TileType, (l + 1) as TileType, (l + 2) as TileType) {
                Ok(s) => result.push(s),
                Err(e) => {
                    err = Some(e);
                    return;
                }
            }
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    Ok(result)
}

/// Removes independent isolated tiles (count only).
fn remove_independent_singles(t: &mut TileSummarize) -> usize {
    let mut count = 0;
    for i in 0..Tile::LEN {
        if t[i] == 1 && is_isolated(t, i) {
            t[i] -= 1;
            count += 1;
        }
    }
    count
}

/// Removes independent isolated tiles (as a Vec).
fn extract_independent_singles_full(t: &mut TileSummarize) -> Result<Vec<TileType>> {
    let mut result = Vec::new();
    for i in 0..Tile::LEN {
        if t[i] == 1 && is_isolated(t, i) {
            t[i] -= 1;
            result.push(i as TileType);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_shanten_to_seven_pairs() {
        let test_str = "226699m99p228s66z 1z";
        let test = Hand::from(test_str);
        assert!(
            HandAnalyzer::new_by_form(&test, Form::SevenPairs)
                .unwrap()
                .shanten
                .is_ready()
        );
    }
    /// A triplet among the pairs must not break the tenpai judgement.
    #[test]
    fn zero_shanten_to_seven_pairs_2() {
        let test_str = "226699m99p222s66z 1z";
        let test = Hand::from(test_str);
        assert!(
            HandAnalyzer::new_by_form(&test, Form::SevenPairs)
                .unwrap()
                .shanten
                .is_ready()
        );
    }
    #[test]
    fn zero_shanten_to_orphans() {
        let test_str = "19m19p11s1234567z 5m";
        let test = Hand::from(test_str);
        assert!(
            HandAnalyzer::new_by_form(&test, Form::ThirteenOrphans)
                .unwrap()
                .shanten
                .is_ready()
        );
    }

    /// The fast path must agree with HandAnalyzer::new_by_form.
    #[test]
    fn calc_shanten_number_by_form_matches_analyzer() {
        let test_strs = [
            "226699m99p228s66z 1z", // seven pairs tenpai
            "19m19p11s1234567z 5m", // thirteen orphans tenpai
            "123456789m123p11z 2p", // normal form tenpai
            "1122m3344p5555s1z 1z", // seven pairs with four of a kind
            "139m258p47s12345z 6z", // scattered hand
            "111222333m44455p 5p",  // winning hand
        ];
        for test_str in test_strs {
            let hand = Hand::from(test_str);
            for form in [Form::Normal, Form::SevenPairs, Form::ThirteenOrphans] {
                assert_eq!(
                    calc_shanten_number_by_form(&hand, form),
                    HandAnalyzer::new_by_form(&hand, form).unwrap().shanten,
                    "form {form:?} mismatch for {test_str}"
                );
            }
        }
    }

    #[test]
    fn calc_shanten_number_by_form_melded_hand() {
        use crate::hand_info::meld::{Meld, MeldFrom, MeldType};
        let tiles = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z1),
        ];
        let melds = vec![Meld {
            tiles: vec![Tile::new(Tile::Z5); 3],
            category: MeldType::Pon,
            from: MeldFrom::Unknown,
            called_tile: Some(Tile::new(Tile::Z5)),
        }];
        let hand = Hand::new_with_melds(tiles, melds, None);

        assert_eq!(
            calc_shanten_number_by_form(&hand, Form::SevenPairs),
            ShantenNumber::UNAVAILABLE
        );
        assert_eq!(
            calc_shanten_number_by_form(&hand, Form::ThirteenOrphans),
            ShantenNumber::UNAVAILABLE
        );
        // The normal form still computes: tenpai waiting on 6s/9s.
        assert!(calc_shanten_number_by_form(&hand, Form::Normal).is_ready());
    }

    /// Four of a kind counts as only one pair for Seven Pairs,
    /// so this hand is one away from tenpai, not tenpai.
    #[test]
    fn seven_pairs_with_4_same_tiles() {
        let test_str = "1122m3344p5555s1z 1z";
        let test = Hand::from(test_str);
        assert_eq!(
            HandAnalyzer::new_by_form(&test, Form::SevenPairs)
                .unwrap()
                .shanten,
            ShantenNumber(1)
        );
    }

    #[test]
    fn win_by_ready_hand() {
        let test_str = "123m444p789s1112z 2z";
        let test = Hand::from(test_str);
        assert!(
            HandAnalyzer::new_by_form(&test, Form::Normal)
                .unwrap()
                .shanten
                .has_won()
        );
    }

    #[test]
    fn win_by_honour_tiles_players_wind() {
        let test_str = "333m456p1789s 333z 1s";
        let test = Hand::from(test_str);
        assert!(
            HandAnalyzer::new_by_form(&test, Form::Normal)
                .unwrap()
                .shanten
                .has_won()
        );
    }

    #[test]
    fn win_by_honour_tiles_prevailing_wind() {
        let test_str = "234567m6789s 111z 6s";
        let test = Hand::from(test_str);
        assert!(
            HandAnalyzer::new_by_form(&test, Form::Normal)
                .unwrap()
                .shanten
                .has_won()
        );
    }
    #[test]
    fn win_by_honour_tiles_dragons() {
        let test_str = "5m123456p888s 777z 5m";
        let test = Hand::from(test_str);
        assert!(
            HandAnalyzer::new_by_form(&test, Form::Normal)
                .unwrap()
                .shanten
                .has_won()
        );
    }
    #[test]
    fn win_by_all_simples() {
        let test_str = "234m8s 567m 333p 456s 8s";
        let test = Hand::from(test_str);
        assert!(
            HandAnalyzer::new_by_form(&test, Form::Normal)
                .unwrap()
                .shanten
                .has_won()
        );
    }

    #[test]
    fn win_by_no_points() {
        let test_str = "123567m234p6799s 5s";
        let test = Hand::from(test_str);
        assert!(
            HandAnalyzer::new_by_form(&test, Form::Normal)
                .unwrap()
                .shanten
                .has_won()
        );
    }

    #[test]
    fn tenpai_with_89_wait() {
        let test_str = "55m123567p56789s 9m";
        let test = Hand::from(test_str);
        assert!(HandAnalyzer::new(&test).unwrap().shanten.is_ready());
    }

    #[test]
    fn tenpai_with_89s_toitsu() {
        let test_str = "11m234p567p234s89s 1z";
        let test = Hand::from(test_str);
        assert!(HandAnalyzer::new(&test).unwrap().shanten.is_ready());
    }

    #[test]
    fn tenpai_with_89m_toitsu() {
        let test_str = "89m11p234p567s234s 2z";
        let test = Hand::from(test_str);
        assert!(HandAnalyzer::new(&test).unwrap().shanten.is_ready());
    }

    /// Four groups plus a partial sequence is tenpai, not a win.
    #[test]
    fn four_melds_and_one_taatsu_is_ready_not_win() {
        let test = Hand::from("234678m56p567s55z 5z");
        assert!(HandAnalyzer::new(&test).unwrap().shanten.is_ready());
    }

    #[test]
    fn kan_hand_with_unrelated_rinshan_tile_is_not_a_win() {
        let test = Hand::from("567p123s678s8s 5555s 1m");
        assert!(HandAnalyzer::new(&test).unwrap().shanten.is_ready());
    }

    #[test]
    fn opened_hand_cannot_be_seven_pairs_or_thirteen_orphans() {
        let test = Hand::from("123456789m11p 789s 1p");
        assert!(
            !HandAnalyzer::new_by_form(&test, Form::SevenPairs)
                .unwrap()
                .shanten
                .is_ready_or_won()
        );
        assert!(
            !HandAnalyzer::new_by_form(&test, Form::ThirteenOrphans)
                .unwrap()
                .shanten
                .is_ready_or_won()
        );
    }

    /// Shanten regression across a spread of hand shapes.
    #[rstest::rstest]
    #[case::seven_pairs_ready("226699m99p228s66z 1z", 0)]
    #[case::thirteen_orphans_ready("19m19p11s1234567z 5m", 0)]
    #[case::normal_win_triplets("123m444p789s1112z 2z", -1)]
    #[case::normal_win_flush("222333444666s6z 6z", -1)]
    #[case::normal_win_nine_gates("1112345678999m 5m", -1)]
    #[case::seven_pairs_win("1122m3344p5566s7z 7z", -1)]
    #[case::thirteen_orphans_win("19m19p19s1234567z 1m", -1)]
    #[case::normal_13_tiles_with_isolated_honours("123m456p789s1234z", 2)]
    #[case::far_from_ready("147m258p369s1234z", 6)]
    #[case::with_open_melds("333m456p1789s 333z 1s", -1)]
    #[case::leftover_tatsu_at_lower_index("23444p22334567s", 0)]
    #[case::leftover_tatsu_at_lower_index_with_drawn("23444p22334567s 1z", 0)]
    fn shanten_regression(#[case] hand_str: &str, #[case] expected: i32) {
        let hand = Hand::from(hand_str);
        let shanten = HandAnalyzer::new(&hand).unwrap().shanten;
        assert_eq!(
            shanten,
            ShantenNumber(expected),
            "hand '{hand_str}': expected {expected}, got {shanten}"
        );
    }
}
