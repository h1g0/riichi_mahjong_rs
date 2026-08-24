/// Yaku evaluation entry point
pub mod checker;

/// Yaku names
pub mod name;

/// 1-han yaku checks
mod check_1_han;

/// 2-han yaku checks
mod check_2_han;

/// 3-han yaku checks
mod check_3_han;

/// 5-han (mangan) yaku checks
mod check_5_han;

/// 6-han yaku checks
mod check_6_han;

/// Yakuman checks
mod check_yakuman;

/// Drift tests between `name` and the published `data/yaku.json`
#[cfg(test)]
mod yaku_data_tests;
