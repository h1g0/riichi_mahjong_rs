//! Custom randomness backend for WASM.
//!
//! Miniquad's (Macroquad's) WASM loader does not use wasm-bindgen, so
//! getrandom's wasm_js backend is unavailable. An XorShift64 PRNG seeded
//! from miniquad's date::now() stands in instead.

use core::cell::Cell;
use getrandom::Error;

// XorShift64 state; WASM is single-threaded, so a Cell suffices.
thread_local! {
    static RNG_STATE: Cell<u64> = Cell::new(0);
}

/// One XorShift64 step.
fn xorshift64(state: u64) -> u64 {
    let mut s = state;
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    s
}

/// Produces the initial seed from the bit pattern of miniquad's
/// date::now() (JS Date.now() / 1000.0).
fn init_seed() -> u64 {
    let now = macroquad::miniquad::date::now();
    let bits = now.to_bits();
    // Zero is a fixed point of XorShift; avoid it.
    if bits == 0 { 1 } else { bits }
}

/// Entry point for getrandom 0.4's custom backend: the symbol getrandom
/// links against under `getrandom_backend = "custom"`.
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
