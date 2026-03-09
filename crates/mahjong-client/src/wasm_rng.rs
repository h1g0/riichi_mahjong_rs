//! WASM用カスタム乱数バックエンド
//!
//! miniquad（Macroquad）のWASMローダーはwasm-bindgenを使わないため、
//! getrandomのwasm_jsバックエンドが使えない。
//! 代わりにXorShift64ベースのPRNGを使い、miniquadのdate::now()でシードする。

use core::cell::Cell;
use getrandom::Error;

// XorShift64 の状態（スレッドローカル）
// WASM はシングルスレッドなので Cell で十分。
thread_local! {
    static RNG_STATE: Cell<u64> = Cell::new(0);
}

/// XorShift64 一ステップ
fn xorshift64(state: u64) -> u64 {
    let mut s = state;
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    s
}

/// 初期シードを生成する。
/// miniquad の date::now() は JS の Date.now()/1000.0 を返すので、
/// そのビットパターンをシードにする。
fn init_seed() -> u64 {
    let now = macroquad::miniquad::date::now();
    let bits = now.to_bits();
    // 0にならないようにする（XorShift は 0 が固定点）
    if bits == 0 { 1 } else { bits }
}

/// getrandom 0.4 カスタムバックエンド用のエントリポイント。
/// `getrandom_backend = "custom"` 時に getrandom がリンクするシンボル。
#[unsafe(no_mangle)]
pub unsafe fn __getrandom_v03_custom(dest: *mut u8, len: usize) -> Result<(), Error> {
    RNG_STATE.with(|cell| {
        let mut state = cell.get();
        if state == 0 {
            state = init_seed();
        }

        let slice = unsafe { core::slice::from_raw_parts_mut(dest, len) };
        for byte in slice.iter_mut() {
            state = xorshift64(state);
            *byte = (state & 0xFF) as u8;
        }
        cell.set(state);
    });
    Ok(())
}
