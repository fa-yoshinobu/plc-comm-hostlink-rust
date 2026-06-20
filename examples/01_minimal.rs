//! Minimal KEYENCE KV Host Link example.
//!
//! Usage:
//!   cargo run --example 01_minimal -- <host> <port> <plc-profile>

use plc_comm_hostlink::{HostLinkClient, HostLinkConnectionOptions};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (host, port, plc_profile) = parse_args()?;
    let mut options = HostLinkConnectionOptions::new(host, plc_profile)?;
    options.port = port;

    let client = HostLinkClient::connect(options).await?;

    // `DM` is the safest first read family; special devices can be model-dependent.
    let dm0 = client.read_typed("DM0", "U").await?;
    println!("{:?}", dm0);

    client.close().await?;
    Ok(())
}

fn parse_args() -> Result<(String, u16, String), Box<dyn Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 4 {
        return Err("Usage: cargo run --example 01_minimal -- <host> <port> <plc-profile>".into());
    }
    Ok((args[1].clone(), args[2].parse()?, args[3].clone()))
}
