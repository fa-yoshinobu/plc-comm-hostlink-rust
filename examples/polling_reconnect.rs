//! Read-only polling loop with automatic reconnect.
//!
//! Usage:
//!   cargo run --features cli --example polling_reconnect -- <host> <port> <transport> <plc-profile> [device] [dtype] [interval-seconds]

use plc_comm_kv_hostlink::{HostLinkClient, HostLinkConnectionOptions, HostLinkError};
use std::error::Error;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

fn positive_duration(seconds: f64, name: &str) -> Result<Duration, Box<dyn Error>> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(format!("{name} must be a finite number greater than zero").into());
    }
    let duration = Duration::try_from_secs_f64(seconds)?;
    if duration.is_zero() {
        return Err(format!("{name} must be greater than zero").into());
    }
    Ok(duration)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;
    let initial_backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(30);
    let mut state = PollState {
        client: None,
        backoff: initial_backoff,
        connected_once: false,
    };

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                if let Some(client) = state.client.take() {
                    let _ = client.close().await;
                }
                log_state("closed", "interrupted by Ctrl+C");
                break;
            }
            result = poll_step(&config, &mut state, initial_backoff, max_backoff) => {
                result?;
            }
        }
    }

    Ok(())
}

struct PollConfig {
    host: String,
    port: u16,
    plc_profile: String,
    transport: plc_comm_kv_hostlink::HostLinkTransportMode,
    device: String,
    dtype: String,
    interval: Duration,
}

struct PollState {
    client: Option<HostLinkClient>,
    backoff: Duration,
    connected_once: bool,
}

async fn poll_step(
    config: &PollConfig,
    state: &mut PollState,
    initial_backoff: Duration,
    max_backoff: Duration,
) -> Result<(), Box<dyn Error>> {
    if state.client.is_none() {
        log_state(
            "reconnecting",
            &format!(
                "{:?} {}:{} profile={}",
                config.transport, config.host, config.port, config.plc_profile
            ),
        );
        let options = HostLinkConnectionOptions::new(
            config.host.clone(),
            config.port,
            config.transport,
            config.plc_profile.as_str(),
        )?;
        match HostLinkClient::connect(options).await {
            Ok(new_client) => {
                state.client = Some(new_client);
                log_state(
                    if state.connected_once {
                        "recovered"
                    } else {
                        "connected"
                    },
                    &format!("{}:{}", config.device, config.dtype),
                );
                state.connected_once = true;
                state.backoff = initial_backoff;
            }
            Err(error) if is_retryable_hostlink(&error) => {
                log_state(
                    "reconnecting",
                    &format!(
                        "connect failed: {error}; retry in {:.1}s",
                        state.backoff.as_secs_f64()
                    ),
                );
                sleep(state.backoff).await;
                state.backoff = next_backoff(state.backoff, max_backoff);
                return Ok(());
            }
            Err(error) => return Err(Box::new(error)),
        }
    }

    let active = state.client.as_ref().expect("client was just connected");
    match active.read_typed(&config.device, &config.dtype).await {
        Ok(value) => {
            log_state(
                "read",
                &format!("{}:{}={value:?}", config.device, config.dtype),
            );
            sleep(config.interval).await;
        }
        Err(error) if is_retryable_hostlink(&error) => {
            log_state("lost", &error.to_string());
            if let Some(client) = state.client.take() {
                let _ = client.close().await;
            }
            log_state(
                "reconnecting",
                &format!("retry in {:.1}s", state.backoff.as_secs_f64()),
            );
            sleep(state.backoff).await;
            state.backoff = next_backoff(state.backoff, max_backoff);
        }
        Err(error) => return Err(Box::new(error)),
    }
    Ok(())
}

fn parse_args() -> Result<PollConfig, Box<dyn Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 5 {
        return Err("Usage: cargo run --features cli --example polling_reconnect -- <host> <port> <transport> <plc-profile> [device] [dtype] [interval-seconds]".into());
    }
    let interval = args
        .get(7)
        .map(|value| value.parse::<f64>())
        .transpose()?
        .unwrap_or(1.0);
    Ok(PollConfig {
        host: args[1].clone(),
        port: args[2].parse()?,
        transport: match args[3].as_str() {
            "tcp" => plc_comm_kv_hostlink::HostLinkTransportMode::Tcp,
            "udp" => plc_comm_kv_hostlink::HostLinkTransportMode::Udp,
            _ => return Err("transport must be exactly 'tcp' or 'udp'".into()),
        },
        plc_profile: args[4].clone(),
        device: args.get(5).cloned().unwrap_or_else(|| "DM100".to_string()),
        dtype: args.get(6).cloned().unwrap_or_else(|| "U".to_string()),
        interval: positive_duration(interval, "interval-seconds")?,
    })
}

fn is_retryable_hostlink(error: &HostLinkError) -> bool {
    matches!(
        error,
        HostLinkError::Transport { .. } | HostLinkError::Timeout(_) | HostLinkError::Closed
    )
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
