//! Basic high-level Host Link example using the queued client.
//!
//! Usage:
//!   cargo run --features cli --example basic_high_level -- <host> <port> <plc-profile>
//!
//! The write uses a DM test address; change it before running against a PLC
//! program that owns that range.

use plc_comm_hostlink::{
    HostLinkConnectionOptions, device_range_catalog_for_plc_profile, open_and_connect,
};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (host, port, plc_profile) = parse_args()?;
    let mut options = HostLinkConnectionOptions::new(host, &plc_profile)?;
    options.port = port;
    let client = open_and_connect(options).await?;

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

fn parse_args() -> Result<(String, u16, String), Box<dyn Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 4 {
        return Err("Usage: cargo run --features cli --example basic_high_level -- <host> <port> <plc-profile>".into());
    }
    Ok((args[1].clone(), args[2].parse()?, args[3].clone()))
}
