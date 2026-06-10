# refactor-status.md

plc-comm-hostlink-rust の `refactor-instructions.md` 対応メモ。

## できたこと

- golden frame vectors を 20 件から 36 件へ拡充。
- 未収録だった送信フレームを現在実装の出力から採取して固定。
  - `check_error_no`: `?E`
  - `query_model`: `?K`
  - `read_device_range_catalog`: `?K`
  - `confirm_operating_mode`: `?M`
  - `forced_set`: `ST R000`
  - `forced_reset`: `RS R000`
  - `read_monitor_bits`: `MBR`
  - `read_monitor_words`: `MWR`
  - `forced_set_consecutive`: `STS R000 3`
  - `forced_reset_consecutive`: `RSS R000 3`
  - `write_consecutive_legacy`: `WRE DM100.U 3 100 200 300`
  - `write_set_value_consecutive`: `WSS T0.D 2 1000 2000`
  - `switch_bank`: `BE 3`
  - `read_expansion_unit_buffer`: `URD 01 100.U 2`
  - `write_expansion_unit_buffer`: `UWR 02 200.S 2 7 8`
  - `read_comments`: `RDC DM20`
- `tests/frame_vectors.rs` の dispatcher を上記コマンドに対応。
- `src/helpers.rs` 内の read-plan private 機構を `src/read_plan.rs` に move-only 分離。
- `src/lib.rs` は private module 追加の `mod read_plan;` のみ。
- 公開 API、既存 vector、送信フレーム文字列、`Cargo.toml`、`docs/`、`CHANGELOG.md` は変更なし。

## 検証結果

Baseline と最終検証はいずれも通過。

- `cargo fmt --all --check`: ok
- `cargo clippy --all-targets --all-features -- -D warnings`: ok
- `cargo test --all-targets --all-features`: ok, 50 tests passed
- `cargo build --features cli --bin hostlink_verify_client`: ok
- `cargo doc --no-deps`: ok
- `cargo test --manifest-path ../PlcIoChecker_Android/rust-core/Cargo.toml --all-targets`: ok
- `cargo check --manifest-path ../PlcIoChecker_iOS/rust/melsec-io-core-ffi/Cargo.toml`: ok

## 途中で失敗したが解消済み

- D1 後の `cargo fmt --all --check` が rustfmt の折り返し差分で一度失敗。
  - `cargo fmt --all` を実行して解消。
- D2 分離直後に `helpers::compile_read_named_plan` 参照が残って一度 compile 失敗。
  - `crate::read_plan::compile_read_named_plan` に差し替えて解消。

## 未実施・見送り

- `set_time(None)` の golden vector は追加していない。
  - 現在時刻依存で固定 golden に向かないため。
- コミットは未実施。
- 実機 PLC への接続は未実施。

## Stop And Ask

- 発生なし。
- 文書との食い違いは見つかっていない。
