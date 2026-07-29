# PLC profiles

PLC profiles select one embedded device-range catalog. They are useful when you build UI validation, address pickers, or model-specific checks before talking to the PLC. The library accepts only the exact canonical strings in this table.
Models not represented below, including KV-700 and KV-1000, do not currently
have a canonical profile.
Use crate-root `plc_profile_descriptors()` to enumerate canonical names, display labels,
connection eligibility, and XYM base profiles for a UI. Store the canonical profile string,
not the display name.

Verified hardware available for focused validation is maintained once in the
shared [KEYENCE KV Host Link profile catalog](https://github.com/fa-yoshinobu/plc-comm-hostlink-profiles#verified-hardware-available-for-validation).

## Device families and ranges

Device-family notation, type suffixes, XYM aliases, and static range tables are shared across the KV Host Link libraries. Use the common [KV Host Link Device Ranges](https://fa-yoshinobu.github.io/plc-comm-docs-site/plc-setup/kv/device-ranges/) page for those details.

The table below identifies the canonical profile names, intended hardware, and
address notation. Device ranges remain in the shared reference above.

## Supported PLC profiles

| Canonical profile | Display name | Intended hardware | Address notation |
| --- | --- | --- | --- |
| `keyence:kv-nano` | KEYENCE KV-NANO | `KV-N24nn`, `KV-N40nn`, `KV-N60nn`, `KV-NC32T` | Native KV notation. |
| `keyence:kv-nano-xym` | KEYENCE KV-NANO (XYM) | Same KV-NANO family | XYM aliases over `keyence:kv-nano`. |
| `keyence:kv-3000` | KEYENCE KV-3000 | `KV-3000` | Native KV notation. |
| `keyence:kv-3000-xym` | KEYENCE KV-3000 (XYM) | Same KV-3000 family | XYM aliases over `keyence:kv-3000`. |
| `keyence:kv-5000` | KEYENCE KV-5000 | `KV-5000`, `KV-5500` | Native KV notation. |
| `keyence:kv-5000-xym` | KEYENCE KV-5000 (XYM) | Same KV-5000 family | XYM aliases over `keyence:kv-5000`. |
| `keyence:kv-7000` | KEYENCE KV-7000 | `KV-7000`, `KV-7300`, `KV-7500` | Native KV notation. |
| `keyence:kv-7000-xym` | KEYENCE KV-7000 (XYM) | Same KV-7000 family | XYM aliases over `keyence:kv-7000`. |
| `keyence:kv-8000` | KEYENCE KV-8000 | `KV-8000`, `KV-8000A` | Native KV notation. |
| `keyence:kv-8000-xym` | KEYENCE KV-8000 (XYM) | Same KV-8000 family | XYM aliases over `keyence:kv-8000`. |
| `keyence:kv-x500` | KEYENCE KV-X500 | `KV-X310`, `KV-X500`, `KV-X520`, `KV-X530`, `KV-X550` | Native KV notation. |
| `keyence:kv-x500-xym` | KEYENCE KV-X500 (XYM) | Same KV-X500 family | XYM aliases over `keyence:kv-x500`. |

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
use plc_comm_kv_hostlink::{HostLinkClient, HostLinkConnectionOptions, HostLinkTransportMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = HostLinkConnectionOptions::new(
        "192.168.250.100",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )?;
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
