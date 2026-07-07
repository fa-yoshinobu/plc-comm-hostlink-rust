# PLC profiles

PLC profiles select one embedded device-range catalog. They are useful when you build UI validation, address pickers, or model-specific checks before talking to the PLC. The library accepts only the exact canonical strings in this table.
Use crate-root `display_name(plc_profile)` for UI labels. Store the canonical profile
string, not the display name.

## Device families and ranges

Device-family notation, type suffixes, XYM aliases, and static range tables are shared across the KV Host Link libraries. Use the common [KV Host Link Device Ranges](https://fa-yoshinobu.github.io/plc-comm-docs-site/plc-setup/kv/device-ranges/) page for those details.

The tables below only identify the canonical profile names and the intended KV families.

## Supported PLC profiles

| Canonical profile | Display name | Runtime model |
| --- | --- | --- |
| `keyence:kv-nano` | `KEYENCE KV-NANO` | `KV-N24nn`, `KV-N40nn`, `KV-N60nn`, `KV-NC32T` |
| `keyence:kv-nano-xym` | `KEYENCE KV-NANO (XYM)` | KV-NANO family with XYM aliases. |
| `keyence:kv-3000` | `KEYENCE KV-3000` | `KV-3000` |
| `keyence:kv-3000-xym` | `KEYENCE KV-3000 (XYM)` | KV-3000 with XYM aliases. |
| `keyence:kv-5000` | `KEYENCE KV-5000` | `KV-5000`, `KV-5500` |
| `keyence:kv-5000-xym` | `KEYENCE KV-5000 (XYM)` | KV-5000 family with XYM aliases. |
| `keyence:kv-7000` | `KEYENCE KV-7000` | `KV-7000`, `KV-7300`, `KV-7500` |
| `keyence:kv-7000-xym` | `KEYENCE KV-7000 (XYM)` | KV-7000 family with XYM aliases. |
| `keyence:kv-8000` | `KEYENCE KV-8000` | `KV-8000`, `KV-8000A` |
| `keyence:kv-8000-xym` | `KEYENCE KV-8000 (XYM)` | KV-8000 family with XYM aliases. |
| `keyence:kv-x500` | `KEYENCE KV-X500` | `KV-X310`, `KV-X500`, `KV-X520`, `KV-X530`, `KV-X550` |
| `keyence:kv-x500-xym` | `KEYENCE KV-X500 (XYM)` | KV-X500 family with XYM aliases. |

## How to select

```rust
use plc_comm_kv_hostlink::device_range_catalog_for_plc_profile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = device_range_catalog_for_plc_profile("keyence:kv-7000")?;
    println!("{}", catalog.plc_profile);
    Ok(())
}
```

Connection setup is separate from catalog selection:

```rust
use plc_comm_kv_hostlink::{HostLinkClient, HostLinkConnectionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut options = HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?;
    options.port = 8501;
    let client = HostLinkClient::connect(options).await?;

    let dm0 = client.read_typed("DM0", "U").await?;
    println!("{:?}", dm0);

    client.close().await?;
    Ok(())
}
```

## Model-specific cautions

| Scope | Caution |
| --- | --- |
| KV-NANO | `EM`, `FM`, `ZF`, and `AT` are unsupported in the embedded catalog. |
| KV-3000 | `AT` is readable, but write helpers reject `AT` because it is not in the Host Link write-device table. |
| KV-5000 | `AT` is readable, but write helpers reject `AT` because it is not in the Host Link write-device table. |
| KV-7000 | Timer/counter preset writes are supported by the KV-7000/8000 class only; use care when changing preset values. |
| KV-8000 | Timer/counter preset writes are supported by the KV-7000/8000 class only; use care when changing preset values. |
| KV-X500 | `VM`, `VB`, `CTH`, `CTC`, and `AT` are unsupported in the embedded catalog. |
| Any `-xym` profile | `X` and `Y` use decimal bank digits plus one hexadecimal bit digit, such as `X10F`. |
