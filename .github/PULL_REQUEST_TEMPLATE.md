<!-- Written in English, please — see CONTRIBUTING.md. -->

## What

<!-- The change, in a sentence or two. -->

Closes #

## Why

<!-- The reason behind it. The diff already shows what changed. -->

## How it was verified

<!-- Which tests, and any manual steps. For CPU-AI changes, include the
     before/after simulation numbers:
     cargo run -p mahjong-server --release --example cpu_simulation -- 100 42 -->

## Checklist

- [ ] `cargo fmt` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` are clean
- [ ] `cargo test` passes
- [ ] Tests added for new behaviour, or a regression test for the bug fixed
- [ ] New UI strings go through `i18n` with every language filled in
- [ ] New terminology matches `docs/glossary.md`
- [ ] Screenshot attached, if the UI changed
