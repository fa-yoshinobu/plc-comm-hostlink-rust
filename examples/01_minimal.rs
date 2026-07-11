//! Minimal KEYENCE KV Host Link example.
//!
//! Usage:
//!   cargo run --example 01_minimal -- <host> <port> <transport> <plc-profile>

use plc_comm_kv_hostlink::{HostLinkClient, HostLinkConnectionOptions, HostLinkTransportMode};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (host, port, transport, plc_profile) = parse_args()?;
    let options = HostLinkConnectionOptions::new(host, port, transport, plc_profile)?;

    let client = HostLinkClient::connect(options).await?;

    // `DM` is the safest first read family; special devices can be model-dependent.
    let dm0 = client.read_typed("DM0", "U").await?;
    println!("{:?}", dm0);

    client.close().await?;
    Ok(())
}

fn parse_args() -> Result<(String, u16, HostLinkTransportMode, String), Box<dyn Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 5 {
        return Err(
            "Usage: cargo run --example 01_minimal -- <host> <port> <transport> <plc-profile>"
                .into(),
        );
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
