//! Read-only JSON-driven Host Link polling recipe.
//!
//! Dry-run validation:
//!   cargo run --features cli --example config_polling -- --config examples/config_polling.example.json --dry-run

mod operational_common;

use operational_common::{
    CsvWriter, MonitorResult, PlcEndpoint, TagSpec, format_endpoint, format_tags, monitor_endpoint,
    parse_positive_duration, parse_transport, positive_duration,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::task::JoinSet;

#[derive(Debug)]
struct Args {
    config: PathBuf,
    dry_run: bool,
    once: bool,
    cycles: Option<usize>,
    initial_backoff: Option<Duration>,
    max_backoff: Option<Duration>,
}

struct PollingPlan {
    endpoints: Vec<PlcEndpoint>,
    tags_by_plc: Vec<Vec<TagSpec>>,
    csv_path: Option<PathBuf>,
    cycles: Option<usize>,
    initial_backoff: Duration,
    max_backoff: Duration,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args().map_err(std::io::Error::other)?;
    let plan = build_plan(&args).map_err(std::io::Error::other)?;

    if args.dry_run {
        for (endpoint, tags) in plan.endpoints.iter().zip(plan.tags_by_plc.iter()) {
            println!("{}", format_endpoint(endpoint));
            println!("  tags: {}", format_tags(tags));
        }
        println!(
            "cycles: {}",
            plan.cycles
                .map(|value| value.to_string())
                .unwrap_or_else(|| "forever".to_string())
        );
        if let Some(path) = &plan.csv_path {
            println!("csv: {}", path.display());
        }
        return Ok(());
    }

    let writer = plan.csv_path.map(CsvWriter::new);
    let mut tasks = JoinSet::new();
    for (endpoint, tags) in plan.endpoints.into_iter().zip(plan.tags_by_plc.into_iter()) {
        let writer = writer.clone();
        let cycles = plan.cycles;
        let initial_backoff = plan.initial_backoff;
        let max_backoff = plan.max_backoff;
        tasks.spawn(async move {
            monitor_endpoint(endpoint, tags, cycles, initial_backoff, max_backoff, writer).await
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
    let mut config = None;
    let mut dry_run = false;
    let mut once = false;
    let mut cycles = None;
    let mut initial_backoff = None;
    let mut max_backoff = None;

    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--config" => config = Some(PathBuf::from(require_value(&mut iter, "--config")?)),
            "--dry-run" => dry_run = true,
            "--once" => once = true,
            "--cycles" => {
                cycles = Some(
                    require_value(&mut iter, "--cycles")?
                        .parse()
                        .map_err(|e| format!("{e}"))?,
                );
                if cycles == Some(0) {
                    return Err("--cycles must be greater than zero".to_string());
                }
            }
            "--initial-backoff" => {
                initial_backoff = Some(parse_positive_duration(
                    &require_value(&mut iter, "--initial-backoff")?,
                    "--initial-backoff",
                )?);
            }
            "--max-backoff" => {
                max_backoff = Some(parse_positive_duration(
                    &require_value(&mut iter, "--max-backoff")?,
                    "--max-backoff",
                )?);
            }
            "--help" | "-h" => return Err(usage()),
            other => return Err(format!("unexpected argument: {other}\n{}", usage())),
        }
    }

    Ok(Args {
        config: config.ok_or_else(usage)?,
        dry_run,
        once,
        cycles,
        initial_backoff,
        max_backoff,
    })
}

fn build_plan(args: &Args) -> MonitorResult<PollingPlan> {
    let data = std::fs::read_to_string(&args.config).map_err(|error| error.to_string())?;
    let root: Value = serde_json::from_str(&data).map_err(|error| error.to_string())?;
    let defaults = root.get("defaults").and_then(Value::as_object);
    let default_timeout_ms = u64_field(defaults, "timeout_ms").unwrap_or(3000);
    let default_interval = f64_field(defaults, "interval").unwrap_or(1.0);
    if default_timeout_ms == 0 {
        return Err("defaults.timeout_ms must be greater than zero".to_string());
    }
    let default_interval = positive_duration(default_interval, "defaults.interval")?;

    let mut endpoints = Vec::new();
    let mut tags_by_plc = Vec::new();
    for (index, plc) in array_field(&root, "plcs")?.iter().enumerate() {
        let object = plc
            .as_object()
            .ok_or_else(|| format!("plcs[{index}] must be an object"))?;
        let name = required_str(object, "name", index)?;
        let host = required_str(object, "host", index)?;
        let profile = object
            .get("plc_profile")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("plcs[{index}] requires plc_profile"))?;
        let port_value = object
            .get("port")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("plcs[{index}] requires port"))?;
        let port = u16::try_from(port_value)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| format!("plcs[{index}].port must be in 1..=65535"))?;
        let transport = object
            .get("transport")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("plcs[{index}] requires transport"))
            .and_then(parse_transport)?;
        let timeout_ms = object
            .get("timeout_ms")
            .and_then(Value::as_u64)
            .unwrap_or(default_timeout_ms);
        if timeout_ms == 0 {
            return Err(format!(
                "plcs[{index}].timeout_ms must be greater than zero"
            ));
        }
        let interval = if let Some(value) = object.get("interval") {
            let seconds = value
                .as_f64()
                .ok_or_else(|| format!("plcs[{index}].interval must be a number"))?;
            positive_duration(seconds, &format!("plcs[{index}].interval"))?
        } else {
            default_interval
        };
        endpoints.push(PlcEndpoint {
            name: name.to_string(),
            host: host.to_string(),
            plc_profile: profile.to_string(),
            port,
            transport,
            timeout_ms,
            interval,
        });
        tags_by_plc.push(parse_tags(object.get("tags"), name)?);
    }

    let cycles = if args.once {
        Some(1)
    } else {
        args.cycles.or_else(|| {
            root.get("cycles")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value > 0)
        })
    };
    let initial_backoff = match args.initial_backoff {
        Some(value) => value,
        None => positive_duration(
            root.get("initial_backoff")
                .and_then(Value::as_f64)
                .unwrap_or(1.0),
            "initial_backoff",
        )?,
    };
    let max_backoff = match args.max_backoff {
        Some(value) => value,
        None => positive_duration(
            root.get("max_backoff")
                .and_then(Value::as_f64)
                .unwrap_or(30.0),
            "max_backoff",
        )?,
    };
    if max_backoff < initial_backoff {
        return Err("max_backoff must be greater than or equal to initial_backoff".to_string());
    }

    Ok(PollingPlan {
        endpoints,
        tags_by_plc,
        csv_path: output_csv_path(&args.config, &root)?,
        cycles,
        initial_backoff,
        max_backoff,
    })
}

fn parse_tags(raw_tags: Option<&Value>, plc_name: &str) -> MonitorResult<Vec<TagSpec>> {
    let raw_tags = raw_tags
        .and_then(Value::as_array)
        .ok_or_else(|| format!("plcs[{plc_name}].tags must be a list"))?;
    let mut tags = Vec::new();
    for (index, raw) in raw_tags.iter().enumerate() {
        let object = raw
            .as_object()
            .ok_or_else(|| format!("plcs[{plc_name}].tags[{index}] must be an object"))?;
        let name = object
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("plcs[{plc_name}].tags[{index}] requires name"))?;
        let address = object
            .get("address")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("plcs[{plc_name}].tags[{index}] requires address"))?;
        tags.push(TagSpec {
            name: name.to_string(),
            address: address.to_string(),
        });
    }
    if tags.is_empty() {
        return Err(format!("plcs[{plc_name}].tags must not be empty"));
    }
    Ok(tags)
}

fn output_csv_path(config_path: &Path, root: &Value) -> MonitorResult<Option<PathBuf>> {
    let Some(raw_csv) = root
        .get("output")
        .and_then(Value::as_object)
        .and_then(|output| output.get("csv"))
    else {
        return Ok(None);
    };
    let Some(raw_csv) = raw_csv.as_str() else {
        return Err("output.csv must be a string".to_string());
    };
    let path = PathBuf::from(raw_csv);
    if path.is_absolute() {
        Ok(Some(path))
    } else {
        Ok(Some(
            config_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(path),
        ))
    }
}

fn u64_field(object: Option<&serde_json::Map<String, Value>>, key: &str) -> Option<u64> {
    object
        .and_then(|value| value.get(key))
        .and_then(Value::as_u64)
}

fn f64_field(object: Option<&serde_json::Map<String, Value>>, key: &str) -> Option<f64> {
    object
        .and_then(|value| value.get(key))
        .and_then(Value::as_f64)
}

fn array_field<'a>(root: &'a Value, key: &str) -> MonitorResult<&'a Vec<Value>> {
    root.get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} must be a list"))
}

fn required_str<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    index: usize,
) -> MonitorResult<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("plcs[{index}] requires {key}"))
}

fn require_value(iter: &mut impl Iterator<Item = String>, name: &str) -> MonitorResult<String> {
    iter.next()
        .ok_or_else(|| format!("{name} requires a value"))
}

fn usage() -> String {
    "Usage: cargo run --features cli --example config_polling -- --config examples/config_polling.example.json [--dry-run]".to_string()
}
