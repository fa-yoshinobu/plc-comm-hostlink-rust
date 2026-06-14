[![CI](https://github.com/fa-yoshinobu/plc-comm-hostlink-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/fa-yoshinobu/plc-comm-hostlink-rust/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/plc-comm-hostlink-rust.svg)](https://crates.io/crates/plc-comm-hostlink-rust)
[![docs.rs](https://img.shields.io/docsrs/plc-comm-hostlink-rust)](https://docs.rs/plc-comm-hostlink-rust)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

# KV Host Link Protocol for Rust

Rust async library for KEYENCE KV Host Link communication.

## Supported profiles

| Canonical profile | Catalog profile | Notes |
| --- | --- | --- |
| `keyence:kv-nano` | `KV-NANO` | Standard KV-NANO device ranges. |
| `keyence:kv-nano-xym` | `KV-NANO(XYM)` | KV-NANO ranges with XYM alias notation. |
| `keyence:kv-3000` | `KV-3000` | KV-3000 device ranges. |
| `keyence:kv-3000-xym` | `KV-3000(XYM)` | KV-3000 ranges with XYM alias notation. |
| `keyence:kv-5000` | `KV-5000` | KV-5000 device ranges. |
| `keyence:kv-5000-xym` | `KV-5000(XYM)` | KV-5000 ranges with XYM alias notation. |
| `keyence:kv-7000` | `KV-7000` | KV-7000, KV-7300, and KV-7500 family ranges. |
| `keyence:kv-7000-xym` | `KV-7000(XYM)` | KV-7000 ranges with XYM alias notation. |
| `keyence:kv-8000` | `KV-8000` | KV-8000 and KV-8000A family ranges. |
| `keyence:kv-8000-xym` | `KV-8000(XYM)` | KV-8000 ranges with XYM alias notation. |
| `keyence:kv-x500` | `KV-X500` | KV-X310, KV-X500, KV-X520, KV-X530, and KV-X550 family ranges. |
| `keyence:kv-x500-xym` | `KV-X500(XYM)` | KV-X500 ranges with XYM alias notation. |

## Supported device types

| Family | Common devices | Typical use |
| --- | --- | --- |
| Relay bits | `R`, `B`, `MR`, `LR`, `CR`, `VB` | Direct bit reads, writes, monitor registration, and forced set/reset. |
| Data memory | `DM`, `EM`, `FM`, `ZF` | Word, signed word, double word, long, and float reads or writes. |
| Word memory | `W`, `TM`, `CM`, `VM` | Word-oriented register access. |
| Timer/counter | `T`, `C`, `TC`, `TS`, `CC`, `CS` | Timer and counter current values, status, and preset reads. |
| Index and trimmer | `Z`, `AT` | Index registers and digital trimmer values on supported PLCs. |
| XYM bit aliases | `X`, `Y`, `M`, `L` | Alias notation exposed by XYM catalog profiles. |
| XYM word aliases | `D`, `E`, `F` | Alias notation for `DM`, `EM`, and `FM` rows. |

See [Supported registers](docs/SUPPORTED_REGISTERS.md) for ranges, suffixes, and addressing notes.

## Installation

```bash
cargo add plc-comm-hostlink-rust
```

The package name is `plc-comm-hostlink-rust`; the Rust import path is `plc_comm_hostlink`.

## Quick example

```rust
use plc_comm_hostlink::{
    HostLinkClient, HostLinkConnectionOptions, device_range_catalog_for_plc_profile,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = device_range_catalog_for_plc_profile("keyence:kv-7000")?;

    let mut options = HostLinkConnectionOptions::new("192.168.250.100");
    options.port = 8501;
    let client = HostLinkClient::connect(options).await?;

    let dm0 = client.read_typed("DM0", "U").await?;
    println!("{} DM0 = {:?}", catalog.plc_profile, dm0);

    client.close().await?;
    Ok(())
}
```

## Documentation links

| Page | Link |
| --- | --- |
| Getting started | [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) |
| Usage guide | [docs/USAGE_GUIDE.md](docs/USAGE_GUIDE.md) |
| Supported registers | [docs/SUPPORTED_REGISTERS.md](docs/SUPPORTED_REGISTERS.md) |
| PLC profiles | [docs/PROFILES.md](docs/PROFILES.md) |
| Gotchas | [docs/GOTCHAS.md](docs/GOTCHAS.md) |
| Examples | [examples/README.md](examples/README.md) |

## Hardware verified

| PLC | Runtime result | Transport | Validation record |
| --- | --- | --- | --- |
| KEYENCE KV-7500 | Model code `55`, resolved as `KV-7000` | TCP and UDP | [KV-7000 live validation](docs/KV7000_LIVE_VALIDATION_2026-05-03.md) |
| KEYENCE KV-5000 | Model code `52`, configured as `keyence:kv-5000` | TCP | [KV-5000 live validation](docs/KV5000_LIVE_VALIDATION_2026-05-03.md) |

## License and registry

| Item | Value |
| --- | --- |
| License | [MIT](LICENSE) |
| Registry | [crates.io/crates/plc-comm-hostlink-rust](https://crates.io/crates/plc-comm-hostlink-rust) |
| API docs | [docs.rs/plc-comm-hostlink-rust](https://docs.rs/plc-comm-hostlink-rust) |
