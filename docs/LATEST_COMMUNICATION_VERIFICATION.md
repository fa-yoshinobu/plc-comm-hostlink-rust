# Latest communication verification

This page is the public index for retained live-device validation records.

| Date | PLC / CPU | Canonical profile | Transport | Verified scope | Limitations | Record |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-05-03 | KEYENCE KV-7500 | `keyence:kv-7000` | TCP `8501`, UDP `8501` | Runtime model query, range catalog resolution, sampled read/write/readback/restore checks. | `T` and `C` reads, `AT` writes, and unavailable `CTH`/`CTC` entries are recorded as target/catalog limitations. | Retained maintainer note |
| 2026-05-03 | KEYENCE KV-5000 | `keyence:kv-5000` | TCP `8501` | Runtime model query, named reads, word write/readback/restore, app bridge checks, and sampled range checks. | Low real-I/O relay addresses and ladder-controlled data registers are kept out of write/readback smoke checks. | Retained maintainer note |

Update this page when a new live-device validation result becomes the public summary. Keep detailed raw notes as maintainer records and keep this page focused on the public summary.
