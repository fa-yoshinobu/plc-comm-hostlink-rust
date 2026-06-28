//! Typed read/write example for the high-level Host Link API.
//!
//! The example writes test values to `DM120` and nearby registers on a PLC at
//! the endpoint you pass on the command line. Change these addresses before
//! running on production equipment. The original values are restored before
//! the example exits.
//!
//! Usage:
//!   cargo run --example 02_typed_read_write -- <host> <port> <plc-profile>

use plc_comm_hostlink::{HostLinkClient, HostLinkConnectionOptions};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (host, port, plc_profile) = parse_args()?;
    let mut options = HostLinkConnectionOptions::new(host, plc_profile)?;
    options.port = port;

    let client = HostLinkClient::connect(options).await?;

    let original_unsigned_word = client.read_typed("DM120", "U").await?;
    let original_signed_word = client.read_typed("DM121", "S").await?;
    let original_unsigned_dword = client.read_typed("DM122", "D").await?;
    let original_signed_dword = client.read_typed("DM124", "L").await?;
    let original_float_value = client.read_typed("DM126", "F").await?;

    let demo_result: Result<(), Box<dyn Error>> = async {
        // Use the dtype argument for word formats. Dot notation is bit-in-word.
        client.write_typed("DM120", "U", 1234_u16).await?;
        let unsigned_word = client.read_typed("DM120", "U").await?;

        client.write_typed("DM121", "S", -123_i16).await?;
        let signed_word = client.read_typed("DM121", "S").await?;

        client.write_typed("DM122", "D", 123_456_u32).await?;
        let unsigned_dword = client.read_typed("DM122", "D").await?;

        client.write_typed("DM124", "L", -123_456_i32).await?;
        let signed_dword = client.read_typed("DM124", "L").await?;

        client.write_typed("DM126", "F", 12.5_f32).await?;
        let float_value = client.read_typed("DM126", "F").await?;

        println!("{:?}", unsigned_word);
        println!("{:?}", signed_word);
        println!("{:?}", unsigned_dword);
        println!("{:?}", signed_dword);
        println!("{:?}", float_value);
        Ok(())
    }
    .await;

    let restore_result: Result<(), Box<dyn Error>> = async {
        client
            .write_typed("DM120", "U", original_unsigned_word)
            .await?;
        client
            .write_typed("DM121", "S", original_signed_word)
            .await?;
        client
            .write_typed("DM122", "D", original_unsigned_dword)
            .await?;
        client
            .write_typed("DM124", "L", original_signed_dword)
            .await?;
        client
            .write_typed("DM126", "F", original_float_value)
            .await?;
        println!("Restored DM120/DM121/DM122/DM124/DM126");
        Ok(())
    }
    .await;

    client.close().await?;
    demo_result?;
    restore_result?;
    Ok(())
}

fn parse_args() -> Result<(String, u16, String), Box<dyn Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 4 {
        return Err(
            "Usage: cargo run --example 02_typed_read_write -- <host> <port> <plc-profile>".into(),
        );
    }
    Ok((args[1].clone(), args[2].parse()?, args[3].clone()))
}
