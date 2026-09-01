# TODO: Host Link Communication Rust

Current active TODOs only.

## Current Status

The following items are cross-library API review candidates. They are not approved for implementation.

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
