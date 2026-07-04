//! Read-only multi-PLC monitor with reconnect state transitions.
//!
//! Dry-run validation:
//!   cargo run --features cli --example multi_plc_monitor -- --plc line-a=192.168.250.100,keyence:kv-8000,8501,tcp --tag dm100=DM100:U --cycles 1 --dry-run

mod operational_common;

use operational_common::{
    MonitorResult, TagSpec, format_endpoint, format_tags, monitor_endpoint, parse_plc_spec,
    parse_tag_spec, parse_transport,
};
use std::time::Duration;
use tokio::task::JoinSet;

#[derive(Debug)]
struct Args {
    plcs: Vec<String>,
    tags: Vec<String>,
    port: u16,
    transport: String,
    timeout_ms: u64,
    interval: Duration,
    cycles: Option<usize>,
    initial_backoff: Duration,
    max_backoff: Duration,
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args().map_err(std::io::Error::other)?;
    if args.max_backoff < args.initial_backoff {
        return Err(std::io::Error::other(
            "--max-backoff must be greater than or equal to --initial-backoff",
        )
        .into());
    }

    let endpoints = args
        .plcs
        .iter()
        .map(|value| {
            parse_plc_spec(
                value,
                args.port,
                &args.transport,
                args.timeout_ms,
                args.interval,
            )
        })
        .collect::<MonitorResult<Vec<_>>>()
        .map_err(std::io::Error::other)?;
    let tags = parse_tags(&args.tags).map_err(std::io::Error::other)?;

    if args.dry_run {
        for endpoint in &endpoints {
            println!("{}", format_endpoint(endpoint));
        }
        println!("tags: {}", format_tags(&tags));
        return Ok(());
    }

    let mut tasks = JoinSet::new();
    for endpoint in endpoints {
        let tags = tags.clone();
        let initial_backoff = args.initial_backoff;
        let max_backoff = args.max_backoff;
        let cycles = args.cycles;
        tasks.spawn(async move {
            monitor_endpoint(endpoint, tags, cycles, initial_backoff, max_backoff, None).await
        });
    }
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(std::io::Error::other(error).into()),
            Err(error) => return Err(std::io::Error::other(error).into()),
        }
    }
    Ok(())
}

fn parse_args() -> MonitorResult<Args> {
    let mut args = Args {
        plcs: Vec::new(),
        tags: Vec::new(),
        port: 8501,
        transport: "tcp".to_string(),
        timeout_ms: 3000,
        interval: Duration::from_secs(1),
        cycles: None,
        initial_backoff: Duration::from_secs(1),
        max_backoff: Duration::from_secs(30),
        dry_run: false,
    };

    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--plc" => args.plcs.push(require_value(&mut iter, "--plc")?),
            "--tag" => args.tags.push(require_value(&mut iter, "--tag")?),
            "--port" => {
                args.port = require_value(&mut iter, "--port")?
                    .parse()
                    .map_err(|e| format!("{e}"))?
            }
            "--transport" => {
                args.transport = parse_transport(&require_value(&mut iter, "--transport")?)?
            }
            "--timeout-ms" => {
                args.timeout_ms = require_value(&mut iter, "--timeout-ms")?
                    .parse()
                    .map_err(|e| format!("{e}"))?;
            }
            "--interval" => {
                args.interval = Duration::from_secs_f64(
                    require_value(&mut iter, "--interval")?
                        .parse::<f64>()
                        .map_err(|e| format!("{e}"))?,
                );
            }
            "--cycles" => {
                args.cycles = Some(
                    require_value(&mut iter, "--cycles")?
                        .parse()
                        .map_err(|e| format!("{e}"))?,
                );
            }
            "--initial-backoff" => {
                args.initial_backoff = Duration::from_secs_f64(
                    require_value(&mut iter, "--initial-backoff")?
                        .parse::<f64>()
                        .map_err(|e| format!("{e}"))?,
                );
            }
            "--max-backoff" => {
                args.max_backoff = Duration::from_secs_f64(
                    require_value(&mut iter, "--max-backoff")?
                        .parse::<f64>()
                        .map_err(|e| format!("{e}"))?,
                );
            }
            "--dry-run" => args.dry_run = true,
            "--help" | "-h" => return Err(usage()),
            other => return Err(format!("unexpected argument: {other}\n{}", usage())),
        }
    }

    if args.plcs.is_empty() {
        return Err(usage());
    }
    Ok(args)
}

fn parse_tags(raw_tags: &[String]) -> MonitorResult<Vec<TagSpec>> {
    if raw_tags.is_empty() {
        return Ok(vec![parse_tag_spec("DM100:U")?]);
    }
    raw_tags.iter().map(|value| parse_tag_spec(value)).collect()
}

fn require_value(iter: &mut impl Iterator<Item = String>, name: &str) -> MonitorResult<String> {
    iter.next()
        .ok_or_else(|| format!("{name} requires a value"))
}

fn usage() -> String {
    "Usage: cargo run --features cli --example multi_plc_monitor -- --plc NAME=HOST,PROFILE[,PORT[,TRANSPORT]] [--tag NAME=ADDRESS] [--cycles N] [--dry-run]".to_string()
}
