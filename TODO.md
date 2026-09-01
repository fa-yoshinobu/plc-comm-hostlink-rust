# TODO: Host Link Communication Rust

Current active TODOs only.

## Current Status

The items below include one required packaging defect and separate cross-library API review candidates. The API candidates are not approved for implementation.

## HL-REQ-002: Ship the unit-test device-range fixture (`required`)

Target state: the published crate contains the existing JSON fixture required by library unit tests, and the extracted-crate package check runs those tests. Runtime behavior, the public API, and Host Link wire traffic remain unchanged.

Observed defect: `src/device_ranges.rs` uses `include_str!("../tests/fixtures/kv_device_ranges.json")` in two unit tests, but the current package include omits that fixture. An extracted crate therefore fails `cargo test --lib --all-features --no-run --offline` with two missing-file compile errors and exit code `101`. The current extracted-crate check runs `cargo check --lib`, which does not compile `cfg(test)` and does not detect the omission.

Machine-verifiable acceptance criteria:

1. `cargo package --list` contains exactly the required `tests/fixtures/kv_device_ranges.json` fixture.
2. A newly packaged and freshly extracted crate passes `cargo test --lib --all-features`.
3. `scripts/check_package_contents.ps1` executes the extracted-crate library test and passes.
4. No runtime source, public API, error contract, or Host Link wire behavior changes for this fix.

- [ ] Implementation completed in `plc-comm-hostlink-rust`.
- [ ] Tests added or updated for every acceptance criterion.
- [ ] Relevant unit tests and package/build checks passed.
- [ ] Codex self-review completed against this packaging contract.
- [ ] Live PLC verification confirmed not required because runtime and wire behavior do not change.
- [ ] Maintainer documentation, changelog, and package definition agree.
- [ ] Final acceptance criteria verified and the item marked complete.

## HL-CROSS-API-001: Public API naming candidates (`decision_pending`)

Target state: the four Host Link implementations use the same concept names while retaining Rust snake_case. Each candidate must be approved separately before implementation.

| Current Rust API | Candidate canonical API | Reason |
|---|---|---|
| `read_dwords` | `read_dwords_single_request` | The operation is one Host Link request; the explicit canonical API already exists. |
| `read_comments` | `read_comment` | One device produces one comment string, and `read_comment_bytes` is already singular. |
| `check_error_no` | `read_error_number` | The API returns the PLC error number rather than a Boolean check result. |
| `write_set_value` | `write_timer_counter_preset` | The operation is the T/C-only `WS` preset write. |
| `write_set_value_consecutive` | `write_timer_counter_preset_consecutive` | The operation is the T/C-only `WSS` consecutive preset write. |

Migration candidate: add an approved canonical name in the next version and keep the old name as a direct forwarding alias for an independently decided transition period. Input, result, exception, and wire command must not diverge. Deprecated `read_words` APIs are outside this review.

## HL-CROSS-API-002: High-level API parity candidates (`decision_pending`)

- [ ] Decide whether to add `write_named` with the same one-request-only contract already implemented and live-verified by Node-RED `writeNamed`.
- [ ] If approved, reject the complete update set before transport when it cannot fit one compatible `WR`, `WRS`, or `WSS` request; do not synthesize multiple state-changing requests.
- [ ] Add implementation, exact public-identity tests, command/device/response/result live verification, API reference, migration note, and changelog only after the cross-library contract is approved.
