# Pixen rename verification

Verified 2026-10-17 on Windows.

- `cargo test --locked`: 105 passed, zero failed; 5 optional external checks ignored.
- `cargo build --release --locked`: passed; CLI reports `pixen 0.2.0`.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo fmt --all --check` and `git diff --check`: passed.
- Release CLI validated the renamed animated JSON and existing sword-slash container.
- Regression tests confirm legacy `.spritex` containers/manifests and Aseprite metadata remain readable.

The checkout directory remains `D:\Dev\5-tools\spritex`. No commit or publication was performed.
