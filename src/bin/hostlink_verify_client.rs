use futures_util::{StreamExt, pin_mut};
use plc_comm_hostlink::{
    HostLinkConnectionOptions, HostLinkTransportMode, HostLinkValue, KvDeviceRangeCatalog,
    KvDeviceRangeEntry, KvDeviceRangeSegment, KvPlcMode, TimerCounterValue, open_and_connect,
    read_comments, read_counter, read_dwords, read_named, read_timer, read_timer_counter,
    read_words, write_bit_in_word,
};
use serde_json::{Value, json};

#[tokio::main]
async fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 4 {
        println!(
            "{}",
            json!({"status": "error", "message": "Not enough arguments"})
        );
        return;
    }

    let result = run(&args).await.unwrap_or_else(|error| {
        json!({
            "status": "error",
            "message": error.to_string(),
        })
    });
    println!("{result}");
}

async fn run(args: &[String]) -> Result<Value, Box<dyn std::error::Error>> {
    let host = &args[1];
    let port = args[2].parse::<u16>()?;
    let command = args[3].to_ascii_lowercase();
    let address = args.get(4).cloned().unwrap_or_default();

    let mut dtype = String::new();
    let mut count = 1usize;
    let mut interval_ms = 10u64;
    let mut transport = String::from("tcp");
    let mut extra = Vec::new();
    let mut index = 5usize;
    while index < args.len() {
        match args[index].as_str() {
            "--dtype" if index + 1 < args.len() => {
                dtype = args[index + 1].clone();
                index += 2;
            }
            "--count" if index + 1 < args.len() => {
                count = args[index + 1].parse()?;
                index += 2;
            }
            "--interval-ms" if index + 1 < args.len() => {
                interval_ms = args[index + 1].parse()?;
                index += 2;
            }
            "--transport" if index + 1 < args.len() => {
                transport = args[index + 1].clone();
                index += 2;
            }
            _ => {
                extra.push(args[index].clone());
                index += 1;
            }
        }
    }

    let mut options = HostLinkConnectionOptions::new(host.clone());
    options.port = port;
    options.transport = parse_transport(&transport)?;
    let client = open_and_connect(options).await?;

    let result = match command.as_str() {
        "query-model" => {
            let model = client.inner_client().query_model().await?;
            json!({"status": "success", "code": model.code, "model": model.model})
        }
        "confirm-mode" => {
            let mode = client.inner_client().confirm_operating_mode().await?;
            json!({"status": "success", "mode": (mode as u8).to_string()})
        }
        "check-error" => {
            let value = client.inner_client().check_error_no().await?;
            json!({"status": "success", "value": value})
        }
        "clear-error" => {
            client.inner_client().clear_error().await?;
            json!({"status": "success"})
        }
        "set-time-now" => {
            client.inner_client().set_time(None).await?;
            json!({"status": "success"})
        }
        "change-mode" => {
            let mode_text = if address.trim().is_empty() {
                extra.first().cloned().unwrap_or_default()
            } else {
                address.clone()
            };
            if mode_text.trim().is_empty() {
                json!({"status": "error", "message": "change-mode requires RUN or PROGRAM"})
            } else {
                client
                    .inner_client()
                    .change_mode(parse_mode(&mode_text)?)
                    .await?;
                json!({"status": "success"})
            }
        }
        "switch-bank" => {
            let bank_text = if address.trim().is_empty() {
                extra.first().cloned().unwrap_or_default()
            } else {
                address.clone()
            };
            if bank_text.trim().is_empty() {
                json!({"status": "error", "message": "switch-bank requires bank number"})
            } else {
                client
                    .inner_client()
                    .switch_bank(bank_text.parse()?)
                    .await?;
                json!({"status": "success"})
            }
        }
        "read-bit" => {
            let values = client.inner_client().read(&address, None).await?;
            json!({"status": "success", "value": values.first().cloned().unwrap_or_else(|| "0".to_owned())})
        }
        "write-bit" => {
            if extra.is_empty() {
                json!({"status": "error", "message": "write-bit requires a bool value"})
            } else {
                client
                    .inner_client()
                    .write(&address, parse_bool(&extra[0]), None)
                    .await?;
                json!({"status": "success"})
            }
        }
        "forced-set" => {
            client.inner_client().forced_set(&address).await?;
            json!({"status": "success"})
        }
        "forced-reset" => {
            client.inner_client().forced_reset(&address).await?;
            json!({"status": "success"})
        }
        "forced-set-consecutive" => {
            if extra.is_empty() {
                json!({"status": "error", "message": "forced-set-consecutive requires count"})
            } else {
                client
                    .inner_client()
                    .forced_set_consecutive(&address, extra[0].parse()?)
                    .await?;
                json!({"status": "success"})
            }
        }
        "forced-reset-consecutive" => {
            if extra.is_empty() {
                json!({"status": "error", "message": "forced-reset-consecutive requires count"})
            } else {
                client
                    .inner_client()
                    .forced_reset_consecutive(&address, extra[0].parse()?)
                    .await?;
                json!({"status": "success"})
            }
        }
        "monitor-bits" => {
            let devices = ([address.clone()]
                .into_iter()
                .filter(|item| !item.is_empty()))
            .chain(extra.iter().cloned())
            .collect::<Vec<_>>();
            if devices.is_empty() {
                json!({"status": "error", "message": "monitor-bits requires at least one address"})
            } else {
                client
                    .inner_client()
                    .register_monitor_bits(&devices)
                    .await?;
                let values = client.inner_client().read_monitor_bits().await?;
                json!({"status": "success", "values": values})
            }
        }
        "monitor-words" => {
            let devices = ([address.clone()]
                .into_iter()
                .filter(|item| !item.is_empty()))
            .chain(extra.iter().cloned())
            .collect::<Vec<_>>();
            if devices.is_empty() {
                json!({"status": "error", "message": "monitor-words requires at least one address"})
            } else {
                client
                    .inner_client()
                    .register_monitor_words(&devices)
                    .await?;
                let values = client.inner_client().read_monitor_words().await?;
                json!({"status": "success", "values": values})
            }
        }
        "range-catalog" | "read-device-range-catalog" => {
            let catalog = client.inner_client().read_device_range_catalog().await?;
            json!({"status": "success", "catalog": normalize_catalog(&catalog)})
        }
        "write-typed" => {
            if dtype.trim().is_empty() || extra.is_empty() {
                json!({"status": "error", "message": "write-typed requires --dtype and one value"})
            } else {
                let value = parse_typed_value(&dtype, &extra[0])?;
                client.write_typed(&address, &dtype, value).await?;
                json!({"status": "success"})
            }
        }
        "write-set-value" => {
            if dtype.trim().is_empty() || extra.is_empty() {
                json!({"status": "error", "message": "write-set-value requires --dtype and one value"})
            } else {
                client
                    .inner_client()
                    .write_set_value(&address, extra[0].parse::<i64>()?, Some(&dtype))
                    .await?;
                json!({"status": "success"})
            }
        }
        "write-set-values" => {
            if dtype.trim().is_empty() || extra.is_empty() {
                json!({"status": "error", "message": "write-set-values requires --dtype and at least one value"})
            } else {
                let values = extra
                    .iter()
                    .map(|item| item.parse::<i64>())
                    .collect::<Result<Vec<_>, _>>()?;
                client
                    .inner_client()
                    .write_set_value_consecutive(&address, &values, Some(&dtype))
                    .await?;
                json!({"status": "success"})
            }
        }
        "read-typed" => {
            if dtype.trim().is_empty() {
                json!({"status": "error", "message": "read-typed requires --dtype"})
            } else {
                let value = client.read_typed(&address, &dtype).await?;
                json!({"status": "success", "value": normalize_value(&value)})
            }
        }
        "read-timer-counter" => {
            let value = read_timer_counter(client.inner_client(), &address).await?;
            json!({"status": "success", "value": normalize_timer_counter(&value)})
        }
        "read-timer" => {
            let value = read_timer(client.inner_client(), &address).await?;
            json!({"status": "success", "value": normalize_timer_counter(&value)})
        }
        "read-counter" => {
            let value = read_counter(client.inner_client(), &address).await?;
            json!({"status": "success", "value": normalize_timer_counter(&value)})
        }
        "read-comments" => {
            let value = read_comments(client.inner_client(), &address, true).await?;
            json!({"status": "success", "value": value})
        }
        "write-bit-in-word" => {
            if extra.len() < 2 {
                json!({"status": "error", "message": "write-bit-in-word requires bit-index and bool value"})
            } else {
                let bit_index = extra[0].parse::<u8>()?;
                let value = parse_bool(&extra[1]);
                write_bit_in_word(client.inner_client(), &address, bit_index, value).await?;
                json!({"status": "success"})
            }
        }
        "read-named" => {
            let addresses = ([address.clone()]
                .into_iter()
                .filter(|item| !item.is_empty()))
            .chain(extra.iter().cloned())
            .collect::<Vec<_>>();
            if addresses.is_empty() {
                json!({"status": "error", "message": "read-named requires at least one address"})
            } else {
                let values = read_named(client.inner_client(), &addresses).await?;
                json!({"status": "success", "values": normalize_named(&values)})
            }
        }
        "poll" => {
            let addresses = ([address.clone()]
                .into_iter()
                .filter(|item| !item.is_empty()))
            .chain(extra.iter().cloned())
            .collect::<Vec<_>>();
            if addresses.is_empty() {
                json!({"status": "error", "message": "poll requires at least one address"})
            } else {
                let stream = client.poll(&addresses, std::time::Duration::from_millis(interval_ms));
                pin_mut!(stream);
                let mut snapshots = Vec::new();
                while let Some(snapshot) = stream.next().await {
                    snapshots.push(normalize_named(&snapshot?));
                    if snapshots.len() >= count {
                        break;
                    }
                }
                json!({"status": "success", "snapshots": snapshots})
            }
        }
        "read-words" => {
            if extra.is_empty() {
                json!({"status": "error", "message": "read-words requires count"})
            } else {
                let values = read_words(client.inner_client(), &address, extra[0].parse()?).await?;
                json!({"status": "success", "values": values.into_iter().map(|value| value.to_string()).collect::<Vec<_>>()})
            }
        }
        "read-dwords" => {
            if extra.is_empty() {
                json!({"status": "error", "message": "read-dwords requires count"})
            } else {
                let values =
                    read_dwords(client.inner_client(), &address, extra[0].parse()?).await?;
                json!({"status": "success", "values": values.into_iter().map(|value| value.to_string()).collect::<Vec<_>>()})
            }
        }
        "read-expansion" => {
            if extra.len() < 2 {
                json!({"status": "error", "message": "read-expansion requires buffer address and count"})
            } else {
                let format = if dtype.trim().is_empty() {
                    None
                } else {
                    Some(dtype.as_str())
                };
                let values = client
                    .inner_client()
                    .read_expansion_unit_buffer(
                        address.parse()?,
                        extra[0].parse()?,
                        extra[1].parse()?,
                        format,
                    )
                    .await?;
                json!({"status": "success", "values": values})
            }
        }
        "write-expansion" => {
            if extra.len() < 2 {
                json!({"status": "error", "message": "write-expansion requires buffer address and one or more values"})
            } else {
                let format = if dtype.trim().is_empty() {
                    None
                } else {
                    Some(dtype.as_str())
                };
                let values = extra[1..]
                    .iter()
                    .map(|item| {
                        parse_typed_value(if dtype.trim().is_empty() { "U" } else { &dtype }, item)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                client
                    .inner_client()
                    .write_expansion_unit_buffer(
                        address.parse()?,
                        extra[0].parse()?,
                        &values,
                        format,
                    )
                    .await?;
                json!({"status": "success"})
            }
        }
        _ => json!({"status": "error", "message": format!("Unknown command: {command}")}),
    };

    let _ = client.close().await;
    Ok(result)
}

fn normalize_catalog(catalog: &KvDeviceRangeCatalog) -> Value {
    json!({
        "model": catalog.model,
        "modelCode": catalog.model_code,
        "hasModelCode": catalog.has_model_code,
        "requestedModel": catalog.requested_model,
        "resolvedModel": catalog.resolved_model,
        "entries": catalog.entries.iter().map(normalize_entry).collect::<Vec<_>>(),
    })
}

fn normalize_entry(entry: &KvDeviceRangeEntry) -> Value {
    json!({
        "device": entry.device,
        "deviceType": entry.device_type,
        "category": format!("{:?}", entry.category),
        "isBitDevice": entry.is_bit_device,
        "notation": format!("{:?}", entry.notation),
        "supported": entry.supported,
        "lowerBound": entry.lower_bound,
        "upperBound": entry.upper_bound,
        "pointCount": entry.point_count,
        "addressRange": entry.address_range,
        "source": entry.source,
        "notes": entry.notes,
        "segments": entry.segments.iter().map(normalize_segment).collect::<Vec<_>>(),
    })
}

fn normalize_segment(segment: &KvDeviceRangeSegment) -> Value {
    json!({
        "device": segment.device,
        "category": format!("{:?}", segment.category),
        "isBitDevice": segment.is_bit_device,
        "notation": format!("{:?}", segment.notation),
        "lowerBound": segment.lower_bound,
        "upperBound": segment.upper_bound,
        "pointCount": segment.point_count,
        "addressRange": segment.address_range,
    })
}

fn parse_typed_value(dtype: &str, raw: &str) -> Result<HostLinkValue, Box<dyn std::error::Error>> {
    let key = dtype.trim_start_matches('.').to_ascii_uppercase();
    Ok(match key.as_str() {
        "F" => HostLinkValue::F32(raw.parse()?),
        "S" => HostLinkValue::I16(raw.parse()?),
        "D" => HostLinkValue::U32(raw.parse()?),
        "L" => HostLinkValue::I32(raw.parse()?),
        _ => HostLinkValue::U16(raw.parse()?),
    })
}

fn parse_bool(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on" | "yes"
    )
}

fn parse_mode(raw: &str) -> Result<KvPlcMode, Box<dyn std::error::Error>> {
    match raw.trim().to_ascii_uppercase().as_str() {
        "1" | "RUN" => Ok(KvPlcMode::Run),
        "0" | "PROGRAM" | "PROG" | "STOP" => Ok(KvPlcMode::Program),
        _ => Err(format!("Unsupported mode: {raw}").into()),
    }
}

fn parse_transport(raw: &str) -> Result<HostLinkTransportMode, Box<dyn std::error::Error>> {
    match raw.trim().to_ascii_uppercase().as_str() {
        "" | "TCP" => Ok(HostLinkTransportMode::Tcp),
        "UDP" => Ok(HostLinkTransportMode::Udp),
        _ => Err(format!("Unsupported transport: {raw}").into()),
    }
}

fn normalize_value(value: &HostLinkValue) -> Value {
    match value {
        HostLinkValue::Bool(value) => json!(value),
        HostLinkValue::F32(value) => json!(
            format!("{value:.9}")
                .trim_end_matches('0')
                .trim_end_matches('.')
        ),
        HostLinkValue::Text(value) => json!(value),
        HostLinkValue::U16(value) => json!(value.to_string()),
        HostLinkValue::I16(value) => json!(value.to_string()),
        HostLinkValue::U32(value) => json!(value.to_string()),
        HostLinkValue::I32(value) => json!(value.to_string()),
    }
}

fn normalize_timer_counter(value: &TimerCounterValue) -> Value {
    json!({
        "status": value.status.to_string(),
        "current": value.current.to_string(),
        "preset": value.preset.to_string(),
    })
}

fn normalize_named(values: &plc_comm_hostlink::NamedSnapshot) -> Value {
    let mut map = serde_json::Map::new();
    for (key, value) in values {
        map.insert(key.clone(), normalize_value(value));
    }
    Value::Object(map)
}
