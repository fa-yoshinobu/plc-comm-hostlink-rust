//! Basic high-level Host Link example using the queued client.
//!
//! Usage:
//!   cargo run --features cli --example basic_high_level -- <host> <port> <transport> <plc-profile>
//!
//! The write uses a DM test address; change it before running against a PLC
//! program that owns that range. The original value is restored before exit.

use plc_comm_kv_hostlink::{
    HostLinkConnectionOptions, HostLinkTransportMode, device_range_catalog_for_plc_profile,
    open_and_connect,
};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (host, port, transport, plc_profile) = parse_args()?;
    let options = HostLinkConnectionOptions::new(host, port, transport, &plc_profile)?;
    let client = open_and_connect(options).await?;

    let catalog = device_range_catalog_for_plc_profile(&plc_profile)?;
    println!("{:?}", catalog.plc_profile);

    // Start with DM reads before model-dependent devices; see docs/GOTCHAS.md.
    let dm0 = client.read_typed("DM0", "U").await?;
    let original_dm120 = client.read_typed("DM120", "U").await?;

    let demo_result: Result<(), Box<dyn Error>> = async {
        client.write_typed("DM120", "U", dm0).await?;

        let snapshot = client
            .read_named(&["DM0:U", "DM1:S", "DM2:D", "DM4:F", "DM120.0"])
            .await?;
        println!("{snapshot:?}");
        Ok(())
    }
    .await;

    let restore_result: Result<(), Box<dyn Error>> = async {
        client.write_typed("DM120", "U", original_dm120).await?;
        println!("Restored DM120");
        Ok(())
    }
    .await;

    demo_result?;
    restore_result?;
    Ok(())
}

fn parse_args() -> Result<(String, u16, HostLinkTransportMode, String), Box<dyn Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 5 {
        return Err("Usage: cargo run --features cli --example basic_high_level -- <host> <port> <transport> <plc-profile>".into());
    }
    Ok((
        args[1].clone(),
        args[2].parse()?,
        parse_transport(&args[3])?,
        args[4].clone(),
    ))
}

fn parse_transport(value: &str) -> Result<HostLinkTransportMode, Box<dyn Error>> {
    match value {
        "tcp" => Ok(HostLinkTransportMode::Tcp),
        "udp" => Ok(HostLinkTransportMode::Udp),
        _ => Err("transport must be exactly 'tcp' or 'udp'".into()),
    }
}
