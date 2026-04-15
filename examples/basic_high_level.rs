use plc_comm_hostlink::{HostLinkConnectionOptions, open_and_connect, read_named, read_typed};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("HOSTLINK_HOST").unwrap_or_else(|_| "192.168.250.100".to_owned());
    let mut options = HostLinkConnectionOptions::new(host);
    if let Ok(port) = std::env::var("HOSTLINK_PORT") {
        options.port = port.parse()?;
    }

    let client = open_and_connect(options).await?;

    let dm0 = read_typed(client.inner_client(), "DM0", "U").await?;
    client.write_typed("DM10", "U", dm0).await?;

    let snapshot = read_named(
        client.inner_client(),
        &["DM0", "DM1:S", "DM2:D", "DM4:F", "DM10.0"],
    )
    .await?;
    println!("{snapshot:?}");
    Ok(())
}
