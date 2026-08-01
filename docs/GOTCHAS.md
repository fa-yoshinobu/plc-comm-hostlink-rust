# Gotchas

Use this page only for library-specific caveats.

Use the shared
[KV Host Link Troubleshooting & Codes](https://fa-yoshinobu.github.io/plc-comm-docs-site/plc-setup/kv/troubleshooting-codes/)
page for common connection, profile, address-shape, write-permission, and PLC
error-code symptoms.

## Current library-specific caveats

| Area | Symptom | Guidance |
| --- | --- | --- |
| RDC comments | A comment read returns a protocol decode error. | Select `Utf8` or `Cp932` explicitly from application knowledge, or use `read_comment_bytes`; the library never guesses or falls back. |
| Custom write formatter | A custom value is rejected before transport. | Return `Ok(token)` only for a validated supported suffix and propagate `HostLinkError` for every invalid value or suffix; empty fallback tokens are not supported. |
