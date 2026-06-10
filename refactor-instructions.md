# refactor-instructions.md

plc-comm-hostlink-rust のリファクタリング指示書。
この文書は実装担当モデル向けの完結した作業指示である。実装前にこの文書全体を読むこと。

> **最重要の前提**: このクレートは crates.io に公開済み(`plc-comm-hostlink-rust` 0.1.3)であり、
> Host Link の ASCII フレームボディは実機 KV-5000 / KV-7500 / KV-X500 で検証済みの記録
> (`docs/KV5000_LIVE_VALIDATION_2026-05-03.md` 等、`TODO.md`)に紐づく。
> **公開 API と送信フレームの文字列は 1 文字たりとも変えてはならない。**
>
> このリポジトリは関連 4 リポジトリ(Android / iOS / slmp-rust / 本リポジトリ)の中で
> **最も健全**である: CI は fmt + clippy `-D warnings` + 全テストを既に強制し、
> golden フレームベクトル(`tests/vectors/hostlink_frame_vectors.json`)とスクリプト式
> モックサーバテスト(`tests/high_level.rs`)があり、モジュールも責務分割済み。
> したがって本タスクの中心は**構造変更ではなく安全網(golden ベクトル)の拡充**である。
> 変更すべきものが見つからなければ、それを正直に報告して終了してよい。
> 無理に変更量を増やすことを最も強く禁ずる。

---

## Objective

公開 API・送信フレーム文字列・クロススタック互換(Python / .NET / Node-RED)を一切壊さずに:

1. **golden フレームベクトルのコマンド網羅を広げる**(現状 20 ベクトル / 12 コマンド種のみ。
   未収録コマンドの現在の送信フレームを特性テストとして固定する)
2. (任意・小)`src/helpers.rs` 内の read-plan 最適化機構(private)の move-only モジュール分離

「全面書き換え」「モジュール再設計」「公開 API の整理」は目的ではなく禁止事項である。

---

## Project Understanding

### 何のライブラリか

KEYENCE KV シリーズの上位リンク(Host Link)プロトコルの async Rust クライアント。
TCP / UDP 対応。ASCII コマンド(`RD` / `RDS` / `WR` / `WRS` / `ST` / `RS` / `WS` / `WSS` 等)を
組み立てて送る。`.NET` 版(`plc-comm-hostlink-dotnet`)を基準実装とし、Python / Node-RED 版と
意味的互換を保つ。

### 利用者(壊すと影響が出る範囲)

1. **crates.io の一般利用者**(公開クレート。semver 契約)
2. **PLC IO Checker Android**: `../PlcIoChecker_Android/rust-core/`(path 依存)
3. **PLC IO Checker iOS**: `../PlcIoChecker_iOS/rust/melsec-io-core-ffi/`(path 依存)
4. **検証ラッパ**: `src/bin/hostlink_verify_client.rs`(`--features cli`)

### モジュール構成(src/、計約 4,400 行)

| ファイル | 行数 | 内容 |
|---|---|---|
| `client.rs` | 1,099 | `HostLinkClient`(`.NET` 基準のフラットなコマンド面)+ `QueuedHostLinkClient`(直列化ラッパ)+ `HostLinkPayloadValue` trait |
| `helpers.rs` | 935 | `read_typed` / `read_named` / `poll` 等の高水準ヘルパ + private な read-plan 最適化機構(`ReadPlan*` 型、`try_parse_optimizable_read_named_request` 788 行〜) |
| `address.rs` | 887 | KV アドレス解析・各種バリデーション(inline `#[cfg(test)]` 742 行〜) |
| `device_ranges.rs` | 715 | モデル別デバイスレンジカタログ(inline test あり) |
| `model.rs` | 124 | 接続オプション、モデル情報、トレースフック(inline test あり) |
| `protocol.rs` | 75 | フレーム組立(CR / CRLF)と応答トリム、Shift-JIS 処理 |
| `error.rs` | 56 | `HostLinkError`、`decode_error_code` |
| `lib.rs` | 38 | 公開 re-export(**公開 API の一覧表**) |

### テスト(既存の安全網)

- `tests/frame_vectors.rs` + `tests/vectors/hostlink_frame_vectors.json`:
  モック TCP サーバで受信した**送信フレームボディの文字列**を golden 比較。
  現在の収録: `change_mode`(2) / `clear_error`(1) / `read`(3) / `read_consecutive`(3) /
  `read_consecutive_legacy`(1) / `read_format`(2) / `register_monitor_bits`(2) /
  `register_monitor_words`(1) / `set_time`(1) / `write`(2) / `write_consecutive`(1) /
  `write_set_value`(1) — 計 20 ベクトル
- `tests/high_level.rs`(647 行): スクリプト式モックサーバで `read_named` のバッチ化、
  Shift-JIS コメント読取、chunked 読み書き等を検証
- src 内 inline test: `address.rs` / `device_ranges.rs` / `model.rs`
- 実機検証記録: `docs/KV5000_LIVE_VALIDATION_2026-05-03.md` / `KV7000_...`(編集禁止)

### CI(`.github/workflows/ci.yml`)

`cargo fmt --all --check` → `cargo clippy --all-targets --all-features -- -D warnings` →
`cargo test --all-targets --all-features`。**完備しており変更不要。**

### ツールチェーン

edition 2024、`rust-version = "1.85"`。変更禁止。

---

## Behaviors To Preserve(絶対に壊さない既存挙動)

1. **公開 API**: `src/lib.rs` の `pub use` 一覧がそのまま公開面。公開アイテムの rename / 削除 /
   シグネチャ変更 / 追加を一切しない。
2. **送信フレームの文字列**: `tests/vectors/hostlink_frame_vectors.json` の既存ベクトルが契約。
   既存ベクトルの編集禁止(追加は Phase 1 の手順でのみ可)。
3. **プロトコル仕様の固定事項**(README / TODO に根拠):
   - `T` / `C` のプリセット書込(`WS` / `WSS`)は KV-8000 / 7000 系のみ。他 CPU は `E1`
   - `AT` は WR / WRS デバイス表に無いため、書込ヘルパは**送信前に**拒否する
   - `AT` は 32bit 点扱い(`AT0` は `AT0:D` 既定、`AT7:D` が終端)
   - `read_typed("T10","D")` / `read_named(&["T10"])` はプリセット値を返す互換挙動。
     複合値(`status` / `current` / `preset`)は `read_timer_counter` のみ
   - コメント読取の XYM エイリアス(`D10` / `E20` / `F30` / `M100` 等)の受理
4. **Shift-JIS デコード**(コメント読取)。`encoding_rs` の使い方を変えない。
5. **`HostLinkError` の variant と `decode_error_code` の対応**(アプリのエラー分類が依存)。
6. **`QueuedHostLinkClient` の直列化セマンティクス**(アプリ側ブリッジが利用)。
7. **feature 構成**: `default = []`、`cli` のみ。default に依存を増やさない。
8. **crates.io 公開**: 本タスクで `cargo publish` をしない。`version` / `CHANGELOG.md` を
   変更しない。

---

## Non-Negotiables(交渉不可の制約)

- 最初に `git status` を確認する。未コミット変更があれば混ぜず、報告して停止する。
- 編集前に Baseline Commands をすべて実行し、結果(テスト件数含む)を記録する。
- 変更は小さく戻しやすい単位。コミットはユーザーの指示があるまで行わない。
- 無関係な整形・「ついで」リファクタリングをしない。
- 新しい依存クレートを追加しない。`Cargo.toml` を変更しない。
- 移動した関数の可視性は `pub(crate)` まで。`pub` にしない。
- `docs/`、`examples/`、`CHANGELOG.md`、既存テストファイルの既存内容を変更しない
  (Phase 1 のベクトル**追加**と、新規テストファイル追加のみ可)。
- Phase 1 で追加する golden 値は「実装の現在の出力」を機械的に採取したものに限る。
  手で組み立てた期待値・マニュアルから起こした期待値を**勝手に正として書かない**
  (現在の実装出力とマニュアルが食い違って見えた場合は Stop And Ask)。
- 実機 PLC への接続を行わない。
- 正しさが不明な場合は実装を止め、「Stop And Ask」として質問を報告書に書く。

---

## Stop And Ask Conditions(即時停止して質問する条件)

- ベクトル採取中に、実装の送信フレームが README / `.NET` 基準 / KEYENCE マニュアルの記述と
  食い違って見えた(**修正せず**、差異の内容を質問として残す。バグでも勝手に直さない —
  直すと実機検証記録との対応が崩れる)
- 既存テスト(`frame_vectors` / `high_level` / inline)が自分の変更後に落ちた
  ⇒ 即座に巻き戻して報告
- Phase 2 の移動対象が予想に反して `&self` や状態に依存していた ⇒ スキップして報告
- 公開 API・エラーメッセージ・フレーム文字列に影響しうる変更が必要に見えた
- 依存先アプリ(`../PlcIoChecker_Android/rust-core`、`../PlcIoChecker_iOS/rust/melsec-io-core-ffi`)
  のビルドが自分の変更後に失敗した
- 本書の Debt Map に無い大きな問題を発見した(報告のみ)

---

## Baseline Commands

作業ディレクトリ: リポジトリルート。Rust 1.85+。OS は問わない
(テストは localhost のモックサーバのみ。実機 PLC 不要・接続禁止)。

```bash
git status                                              # クリーンであることを確認
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features                 # CI と同一
cargo build --features cli --bin hostlink_verify_client
cargo doc --no-deps
```

兄弟リポジトリがある環境では、依存側の baseline も記録(無ければスキップと明記):

```bash
cargo test --manifest-path ../PlcIoChecker_Android/rust-core/Cargo.toml --all-targets
cargo check --manifest-path ../PlcIoChecker_iOS/rust/melsec-io-core-ffi/Cargo.toml
```

---

## Debt Map

行番号は調査時点(main, commit `69ab758`)のアンカー。ドリフトしていたら宣言名で探すこと。

### D1. golden フレームベクトルのコマンド網羅不足 【実装可 / 主作業】

- **根拠**: `HostLinkClient` の公開コマンド面(`src/client.rs`)に対し、
  `hostlink_frame_vectors.json` は 12 コマンド種しか収録していない。未収録の例:
  `forced_set` / `forced_reset`(+ `_consecutive`)、`write_consecutive_legacy`、
  `write_set_value_consecutive`、`switch_bank`、`read_expansion_unit_buffer` /
  `write_expansion_unit_buffer`、`check_error_no`、`query_model`、
  `read_monitor_bits` / `read_monitor_words`(読出側)、`read_comments`、
  `confirm_operating_mode`、`set_time(None)` 系のバリエーション。
- **なぜ負債か**: フレーム文字列が本クレート最大の契約なのに、契約テストが面の一部しか
  押さえていない。将来の変更(意図的・事故とも)を検出できない。
- **改善案**: `tests/frame_vectors.rs` の既存方式(モックサーバが受信ボディを記録)を使い、
  未収録コマンドの**現在の送信フレームを採取して**ベクトルを追加する。
  - 追加は JSON への追記 + 必要ならディスパッチ部(`frame_vectors.rs` の command 分岐)の拡張
  - 既存 20 ベクトルは 1 文字も変更しない
  - 採取値が文書と食い違って見えたら Stop And Ask(Non-Negotiables 参照)
  - `read_comments` は応答の Shift-JIS 復号が絡むため、送信フレームの検証のみで良い
- **検証**: 追加後に全テストが通ること。追加ベクトル一覧を報告書に記載。

### D2. `helpers.rs` 内の read-plan 最適化機構の同居 【実装可(任意・小)】

- **根拠**: `read_named` のバッチ最適化を担う private 機構(`ReadPlanValueKind` /
  `ReadPlanSegmentMode` / `ReadPlanRequest` / `ReadPlanSegment` 35–100 行、
  `try_parse_optimizable_read_named_request` 788 行〜 `resolve_direct_bit_value` 904 行)が、
  公開ヘルパ群と同じ 935 行のファイルに同居している。
- **なぜ負債か(軽度)**: `read_named` の最適化規則は本ファイルで最も複雑なロジックだが、
  公開 API 部分と混ざっており単独で読めない。挙動は `tests/high_level.rs` が押さえている。
- **改善案**: private モジュール(例: `src/read_plan.rs`、`lib.rs` には `pub` の無い `mod` 追加)
  へ move-only 分離し、`pub(crate)` 化する。シグネチャ・ロジックは変えない。
  **D1 が完了してから着手し、時間や確信が足りなければ実施せず提案として報告するだけでよい。**
- **検証**: `tests/high_level.rs` が無修正で通ること。

### D3. その他(現状維持 / 報告のみ)

- `client.rs`(1,099 行)のフラットなコマンド面は `.NET` 基準実装との対応を保つための
  **意図的な構造**。分割しない。
- `address.rs` / `device_ranges.rs` / `model.rs` は inline test 付きで健全。触らない。
- `protocol.rs`(75 行)は小さく明確。触らない。
- CI は完備。変更不要。

---

## Implementation Phases

### Phase 0: 現状確認

1. `git status` 確認(クリーンでなければ停止・報告)
2. Baseline Commands を実行し、結果を記録

### Phase 1: golden フレームベクトルの拡充(D1)

1. `HostLinkClient` の公開コマンドと既存ベクトルを突き合わせ、未収録コマンドの一覧を作る
2. 1 コマンドずつ: モックサーバで現在の送信フレームを採取 → ベクトル追加 → テスト実行
3. 文書との食い違いを見つけたら、そのコマンドはベクトル追加を保留して Stop And Ask に記録し、
   他のコマンドの作業は続行する

### Phase 2: read-plan 機構の分離(D2、任意)

1. D1 完了後に着手。move-only でモジュール分離、全テスト実行
2. 確信が持てなければ実施せず、提案として報告

### Phase 3: 検証と報告

1. 全 Verification Requirements を最終実行(依存アプリのビルド確認を含む)
2. Reporting Format に従って報告書を作成

---

## Verification Requirements

各フェーズ完了時に最低限:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --features cli --bin hostlink_verify_client
```

最終フェーズでは追加で:

```bash
cargo doc --no-deps
# 依存アプリ(兄弟リポジトリがある場合)
cargo test --manifest-path ../PlcIoChecker_Android/rust-core/Cargo.toml --all-targets
cargo check --manifest-path ../PlcIoChecker_iOS/rust/melsec-io-core-ffi/Cargo.toml
```

- baseline で通っていたテストがすべて通り、件数が増えていること(D1 の追加分)
- 既存ベクトル 20 件が無変更であること(`git diff` で JSON の既存行に変更が無いことを確認)
- `src/lib.rs` の diff が(D2 実施時の)`pub` の無い `mod` 行追加のみであること
- 実機 PLC への接続を行っていないこと

---

## Reporting Format

作業完了時(または中断時)に以下を Markdown で報告する:

1. **Baseline 結果**: 実行コマンドと結果(テスト件数)
2. **D1 の網羅表**: 公開コマンド一覧 × ベクトル有無(作業前 / 作業後)
3. **追加したベクトル**: コマンドごとの採取フレーム文字列
4. **文書との食い違い**: 見つけた場合はコマンド名・実装の出力・文書の記述を併記(修正はしない)
5. **D2 の実施有無**: 実施した場合は移動した宣言一覧、見送った場合はその理由
6. **各フェーズの検証結果**: 最後に実行したコマンドと結果(失敗を隠さない)
7. **Stop And Ask**: 発生した質問と停止範囲
8. **未実施事項**: 依存アプリのビルド確認ができなかった等の明記

---

## Out-of-scope Items(やらないこと)

- 公開 API の変更・追加・整理
- 送信フレーム文字列・エラーメッセージ・Shift-JIS 処理の変更(食い違いを見つけても報告のみ)
- `client.rs` のコマンド面の再構成(`.NET` 基準との対応を崩すため)
- バージョン番号変更、`CHANGELOG.md` 更新、`cargo publish`
- 依存クレートの追加・更新、edition / MSRV 変更、CI 変更(完備のため不要)
- `docs/`(実機検証記録)・`examples/` の変更
- 実機 PLC を使う検証
- 兄弟リポジトリ(`PlcIoChecker_Android` / `PlcIoChecker_iOS` / `plc-comm-slmp-rust` /
  dotnet・python・nodered 一族)の変更
- 「死コード」と思われるものの削除(検証ラッパ / クロススタック互換用の可能性が高い)
