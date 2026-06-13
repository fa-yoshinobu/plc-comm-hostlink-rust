//! Basic high-level Host Link example using the queued client.
//!
//! The default target is `192.168.250.100:8501`. The write uses a DM test
//! address; change it before running against a PLC program that owns that range.

use plc_comm_hostlink::{
    HostLinkConnectionOptions, device_range_catalog_for_plc_profile, open_and_connect,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("HOSTLINK_HOST").unwrap_or_else(|_| "192.168.250.100".to_owned());
    let mut options = HostLinkConnectionOptions::new(host);
    if let Ok(port) = std::env::var("HOSTLINK_PORT") {
        options.port = port.parse()?;
    }

    // `HostLinkConnectionOptions::new` defaults to Host Link port 8501.
    let client = open_and_connect(options).await?;

    let plc_profile =
        std::env::var("HOSTLINK_PLC_PROFILE").unwrap_or_else(|_| "keyence:kv-8000".to_owned());
    let catalog = device_range_catalog_for_plc_profile(&plc_profile)?;
    println!("{:?}", catalog.plc_profile);

    // Start with DM reads before model-dependent devices; see docs/GOTCHAS.md.
    let dm0 = client.read_typed("DM0", "U").await?;
    client.write_typed("DM120", "U", dm0).await?;

    let snapshot = client
        .read_named(&["DM0", "DM1:S", "DM2:D", "DM4:F", "DM120.0"])
        .await?;
    println!("{snapshot:?}");
    Ok(())
}
