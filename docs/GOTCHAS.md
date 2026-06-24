# Gotchas

## Timer/counter preset write returns E1

| Field | Detail |
| --- | --- |
| Symptom | A timer or counter preset write returns `E1`. |
| Root cause | Host Link preset writes through `WS` and `WSS` are supported on KV-8000/7000-series, not on KV-3000, KV-5000, or KV-NANO. |
| Fix | Do not write timer/counter presets on unsupported models; use `read_timer`, `read_counter`, or `read_timer_counter` for safe reads. |

```rust
use plc_comm_hostlink::{HostLinkClient, HostLinkConnectionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client =
        HostLinkClient::connect(HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?).await?;

    let value = client.read_timer_counter("T0").await?;
    println!("{:?}", value);

    client.close().await?;
    Ok(())
}
```

## AT device fails on KV-X500

| Field | Detail |
| --- | --- |
| Symptom | Reading `AT0` fails or the range catalog reports no `AT` entry for KV-X500. |
| Root cause | `AT` is not available in `keyence:kv-x500` or `keyence:kv-x500-xym`. |
| Fix | Check the selected profile before using `AT`, and avoid `AT` on KV-X500 projects. |

```rust
use plc_comm_hostlink::device_range_catalog_for_plc_profile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = device_range_catalog_for_plc_profile("keyence:kv-x500")?;
    println!("{:?}", catalog.entry("AT"));
    Ok(())
}
```

## X or Y address rejected

| Field | Detail |
| --- | --- |
| Symptom | `X` or `Y` raises an address parse or PLC device-number error. |
| Root cause | `X` and `Y` use decimal-bank plus hex-bit notation. `X10F` means bank 10, bit F. |
| Fix | Use `X10F` or `Y10F`, and select an `-xym` profile when you want XYM aliases. |

```rust
use plc_comm_hostlink::HostLinkAddress;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = HostLinkAddress::normalize("X10F")?;
    println!("{address}");
    Ok(())
}
```

## R, MR, LR, or CR address rejected

| Field | Detail |
| --- | --- |
| Symptom | `R`, `MR`, `LR`, or `CR` raises an address parse or PLC device-number error. |
| Root cause | These families use KEYENCE two-digit bit notation. |
| Fix | Use forms such as `R200` or `MR100`; do not treat the full suffix as one hexadecimal number. |

```rust
use plc_comm_hostlink::HostLinkAddress;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = HostLinkAddress::normalize("MR100")?;
    println!("{address}");
    Ok(())
}
```

## Wrong port causes an immediate timeout

| Field | Detail |
| --- | --- |
| Symptom | The connection times out immediately even though the PLC responds on the network. |
| Root cause | KV Host Link uses port `8501`, not the SLMP/Computerlink port `1025`. |
| Fix | Set `options.port = 8501` or leave the default from `HostLinkConnectionOptions::new`. |

```rust
use plc_comm_hostlink::HostLinkConnectionOptions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut options = HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?;
    options.port = 8501;
    println!("{}", options.port);
    Ok(())
}
```

## Non-canonical profile string rejected

| Field | Detail |
| --- | --- |
| Symptom | A display label, uppercase variant, or old alias returns `HostLinkError`. |
| Root cause | The range catalog accepts only exact canonical profile strings. It does not infer the profile from a PLC model response. |
| Fix | Store the canonical string from [PROFILES.md](PROFILES.md) in your project settings or UI value. |

```rust
use plc_comm_hostlink::available_plc_profiles;

fn main() {
    for profile in available_plc_profiles() {
        println!("{profile}");
    }
}
```

## DM100.D reads the wrong thing

| Field | Detail |
| --- | --- |
| Symptom | `DM100.D` reads a boolean-like value instead of a 32-bit value. |
| Root cause | Dot notation means bit-in-word, so `DM100.D` is bit 13, not a double-word read. |
| Fix | Use the dtype argument or colon notation for data types. |

```rust
use plc_comm_hostlink::{HostLinkClient, HostLinkConnectionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client =
        HostLinkClient::connect(HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?).await?;

    let value = client.read_typed("DM100", "D").await?;
    println!("{:?}", value);

    client.close().await?;
    Ok(())
}
```

## CTH or CTC address rejected

| Field | Detail |
| --- | --- |
| Symptom | `CTH` or `CTC` appears in the catalog but fails as an input address. |
| Root cause | `CTH` and `CTC` are catalog metadata rows for supported profiles, while the current address parser does not accept them as input addresses. |
| Fix | Treat them as catalog metadata only. |

```rust
use plc_comm_hostlink::device_range_catalog_for_plc_profile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = device_range_catalog_for_plc_profile("keyence:kv-5000")?;
    println!("{:?}", catalog.entry("CTH"));
    Ok(())
}
```
