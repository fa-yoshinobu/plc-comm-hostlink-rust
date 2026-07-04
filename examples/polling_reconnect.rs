//! Read-only polling loop with automatic reconnect.
//!
//! Usage:
//!   cargo run --features cli --example polling_reconnect -- <host> <port> <plc-profile> [device] [dtype] [interval-seconds]

use plc_comm_hostlink::{HostLinkClient, HostLinkConnectionOptions, HostLinkError};
use std::error::Error;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (host, port, plc_profile, device, dtype, interval) = parse_args()?;
    let initial_backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(30);
    let mut backoff = initial_backoff;
    let mut client: Option<HostLinkClient> = None;
    let mut connected_once = false;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                if let Some(client) = client.take() {
                    let _ = client.close().await;
                }
                log_state("closed", "interrupted by Ctrl+C");
                break;
            }
            result = poll_step(
                &host,
                port,
                &plc_profile,
                &device,
                &dtype,
                interval,
                &mut client,
                &mut backoff,
                initial_backoff,
                max_backoff,
                &mut connected_once,
            ) => {
                if let Err(error) = result {
                    return Err(error);
                }
            }
        }
    }

    Ok(())
}

async fn poll_step(
    host: &str,
    port: u16,
    plc_profile: &str,
    device: &str,
    dtype: &str,
    interval: Duration,
    client: &mut Option<HostLinkClient>,
    backoff: &mut Duration,
    initial_backoff: Duration,
    max_backoff: Duration,
    connected_once: &mut bool,
) -> Result<(), Box<dyn Error>> {
    if client.is_none() {
        log_state(
            "reconnecting",
            &format!("tcp {host}:{port} profile={plc_profile}"),
        );
        let mut options = HostLinkConnectionOptions::new(host.to_string(), plc_profile)?;
        options.port = port;
        match HostLinkClient::connect(options).await {
            Ok(new_client) => {
                *client = Some(new_client);
                log_state(
                    if *connected_once { "recovered" } else { "connected" },
                    &format!("{device}:{dtype}"),
                );
                *connected_once = true;
                *backoff = initial_backoff;
            }
            Err(error) if is_retryable_hostlink(&error) => {
                log_state(
                    "reconnecting",
                    &format!("connect failed: {error}; retry in {:.1}s", backoff.as_secs_f64()),
                );
                sleep(*backoff).await;
                *backoff = next_backoff(*backoff, max_backoff);
                return Ok(());
            }
            Err(error) => return Err(Box::new(error)),
        }
    }

    let active = client.as_ref().expect("client was just connected");
    match active.read_typed(device, dtype).await {
        Ok(value) => {
            log_state("read", &format!("{device}:{dtype}={value:?}"));
            sleep(interval).await;
        }
        Err(error) if is_retryable_hostlink(&error) => {
            log_state("lost", &error.to_string());
            if let Some(client) = client.take() {
                let _ = client.close().await;
            }
            log_state(
                "reconnecting",
                &format!("retry in {:.1}s", backoff.as_secs_f64()),
            );
            sleep(*backoff).await;
            *backoff = next_backoff(*backoff, max_backoff);
        }
        Err(error) => return Err(Box::new(error)),
    }
    Ok(())
}

fn parse_args() -> Result<(String, u16, String, String, String, Duration), Box<dyn Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 4 {
        return Err("Usage: cargo run --features cli --example polling_reconnect -- <host> <port> <plc-profile> [device] [dtype] [interval-seconds]".into());
    }
    let interval = args
        .get(6)
        .map(|value| value.parse::<f64>())
        .transpose()?
        .unwrap_or(1.0);
    Ok((
        args[1].clone(),
        args[2].parse()?,
        args[3].clone(),
        args.get(4).cloned().unwrap_or_else(|| "DM100".to_string()),
        args.get(5).cloned().unwrap_or_else(|| "U".to_string()),
        Duration::from_secs_f64(interval),
    ))
}

fn is_retryable_hostlink(error: &HostLinkError) -> bool {
    matches!(error, HostLinkError::Connection(_))
}

fn next_backoff(current: Duration, max: Duration) -> Duration {
    Duration::from_secs_f64((current.as_secs_f64() * 2.0).min(max.as_secs_f64()))
}

fn log_state(state: &str, message: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    println!(
        "{}.{:03} [{state}] {message}",
        now.as_secs(),
        now.subsec_millis()
    );
}
