//! Minimal KEYENCE KV Host Link example.
//!
//! Connects to `192.168.250.100:8501`, reads `DM0`, prints the value, and
//! disconnects.

use plc_comm_hostlink::{HostLinkClient, HostLinkConnectionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut options = HostLinkConnectionOptions::new("192.168.250.100");
    options.port = 8501;

    let client = HostLinkClient::connect(options).await?;

    // `DM` is the safest first read family; special devices can be model-dependent.
    let dm0 = client.read_typed("DM0", "U").await?;
    println!("{:?}", dm0);

    client.close().await?;
    Ok(())
}
