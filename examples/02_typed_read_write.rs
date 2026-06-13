//! Typed read/write example for the high-level Host Link API.
//!
//! The example writes test values to `DM120` and nearby registers on a PLC at
//! `192.168.250.100:8501`. Change these addresses before running on production
//! equipment.

use plc_comm_hostlink::{HostLinkClient, HostLinkConnectionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut options = HostLinkConnectionOptions::new("192.168.250.100");
    options.port = 8501;

    let client = HostLinkClient::connect(options).await?;

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

    client.close().await?;
    Ok(())
}
