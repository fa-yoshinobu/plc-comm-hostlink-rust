use futures_util::{StreamExt, pin_mut};
use plc_comm_kv_hostlink::{
    HostLinkClient, HostLinkCommentEncoding, HostLinkConnectionOptions, HostLinkError,
    HostLinkMonitorWord, HostLinkTransportMode, HostLinkValue, KvDeviceRangeCatalog,
    KvDeviceRangeEntry, KvDeviceRangeSegment, KvPlcMode, TimerCounterValue,
    device_range_catalog_for_plc_profile, poll, poll_with_comment_encoding, read_comment_bytes,
    read_comments, read_counter, read_dwords, read_named, read_named_with_comment_encoding,
    read_timer, read_timer_counter, read_words,
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
    let mut transport = String::new();
    let mut plc_profile = String::new();
    let mut comment_encoding = String::new();
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
            "--plc-profile" if index + 1 < args.len() => {
                plc_profile = args[index + 1].clone();
                index += 2;
            }
            "--comment-encoding" if index + 1 < args.len() => {
                comment_encoding = args[index + 1].clone();
                index += 2;
            }
            _ => {
                extra.push(args[index].clone());
                index += 1;
            }
        }
    }

    if plc_profile.trim().is_empty() {
        return Ok(json!({
            "status": "error",
            "message": "PLC profile is required. Pass --plc-profile with a canonical value such as keyence:kv-8000."
        }));
    }
    if transport.trim().is_empty() {
        return Ok(json!({
            "status": "error",
            "message": "Transport is required. Pass --transport tcp or --transport udp."
        }));
    }

    let options = HostLinkConnectionOptions::new(
        host.clone(),
        port,
        parse_transport(&transport)?,
        &plc_profile,
    )?;
    let client = HostLinkClient::new(options);
    client.open().await?;

    let result = match command.as_str() {
        "query-model" => {
            let model = client.query_model().await?;
            json!({"status": "success", "code": model.code, "model": model.model})
        }
        "confirm-mode" => {
            let mode = client.confirm_operating_mode().await?;
            json!({"status": "success", "mode": (mode as u8).to_string()})
        }
        "check-error" => {
            let value = client.check_error_no().await?;
            json!({"status": "success", "value": value})
        }
        "clear-error" => {
            client.clear_error().await?;
            json!({"status": "success"})
        }
        "set-time-now" => {
            client
                .set_time(plc_comm_kv_hostlink::HostLinkClock::now_local()?)
                .await?;
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
                client.change_mode(parse_mode(&mode_text)?).await?;
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
                client.switch_bank(bank_text.parse()?).await?;
                json!({"status": "success"})
            }
        }
        "read-bit" => {
            let values = client.read(&address, None).await?;
            json!({"status": "success", "value": values.first().cloned().unwrap_or_else(|| "0".to_owned())})
        }
        "write-bit" => {
            if extra.is_empty() {
                json!({"status": "error", "message": "write-bit requires a bool value"})
            } else {
                client.write(&address, parse_bool(&extra[0]), None).await?;
                json!({"status": "success"})
            }
        }
        "forced-set" => {
            client.forced_set(&address).await?;
            json!({"status": "success"})
        }
        "forced-reset" => {
            client.forced_reset(&address).await?;
            json!({"status": "success"})
        }
        "forced-set-consecutive" => {
            if extra.is_empty() {
                json!({"status": "error", "message": "forced-set-consecutive requires count"})
            } else {
                client
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
                    .forced_reset_consecutive(&address, extra[0].parse()?)
                    .await?;
                json!({"status": "success"})
            }
        }
        "register-monitor-bits" => {
            let devices = ([address.clone()]
                .into_iter()
                .filter(|item| !item.is_empty()))
            .chain(extra.iter().cloned())
            .collect::<Vec<_>>();
            if devices.is_empty() {
                json!({"status": "error", "message": "register-monitor-bits requires at least one address"})
            } else {
                client.register_monitor_bits(&devices).await?;
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
                client.register_monitor_bits(&devices).await?;
                let values = client.read_monitor_bits().await?;
                json!({"status": "success", "values": values})
            }
        }
        "register-monitor-words" => {
            let devices = ([address.clone()]
                .into_iter()
                .filter(|item| !item.is_empty()))
            .chain(extra.iter().cloned())
            .collect::<Vec<_>>();
            if devices.is_empty() {
                json!({"status": "error", "message": "register-monitor-words requires at least one address"})
            } else {
                let entries = parse_monitor_words(&devices)?;
                client.register_monitor_words(&entries).await?;
                json!({"status": "success"})
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
                let entries = parse_monitor_words(&devices)?;
                client.register_monitor_words(&entries).await?;
                let values = client.read_monitor_words().await?;
                json!({"status": "success", "values": values})
            }
        }
        "range-catalog" | "read-device-range-catalog" => {
            let plc_profile = if address.trim().is_empty() {
                extra
                    .first()
                    .cloned()
                    .unwrap_or_else(|| plc_profile.clone())
            } else {
                address.clone()
            };
            if plc_profile.trim().is_empty() {
                json!({"status": "error", "message": "range-catalog requires a PLC profile"})
            } else {
                let catalog = device_range_catalog_for_plc_profile(&plc_profile)?;
                json!({"status": "success", "catalog": normalize_catalog(&catalog)})
            }
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
            let value = read_timer_counter(&client, &address).await?;
            json!({"status": "success", "value": normalize_timer_counter(&value)})
        }
        "read-timer" => {
            let value = read_timer(&client, &address).await?;
            json!({"status": "success", "value": normalize_timer_counter(&value)})
        }
        "read-counter" => {
            let value = read_counter(&client, &address).await?;
            json!({"status": "success", "value": normalize_timer_counter(&value)})
        }
        "read-comments" => {
            if comment_encoding.is_empty() {
                json!({"status": "error", "message": "read-comments requires --comment-encoding utf-8 or cp932"})
            } else {
                let value = read_comments(
                    &client,
                    &address,
                    parse_comment_encoding(&comment_encoding)?,
                )
                .await?;
                json!({"status": "success", "value": value})
            }
        }
        "read-comment-bytes" => {
            let value = read_comment_bytes(&client, &address).await?;
            let hex = value
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<String>();
            json!({"status": "success", "bytes": value, "hex": hex})
        }
        "read-named" => {
            let addresses = ([address.clone()]
                .into_iter()
                .filter(|item| !item.is_empty()))
            .chain(extra.iter().cloned())
            .collect::<Vec<_>>();
            if addresses.is_empty() {
                json!({"status": "error", "message": "read-named requires at least one address"})
            } else if comment_encoding.is_empty() {
                let values = read_named(&client, &addresses).await?;
                json!({"status": "success", "values": normalize_named(&values)})
            } else {
                let values = read_named_with_comment_encoding(
                    &client,
                    &addresses,
                    parse_comment_encoding(&comment_encoding)?,
                )
                .await?;
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
            } else if comment_encoding.is_empty() {
                let stream = poll(
                    &client,
                    &addresses,
                    std::time::Duration::from_millis(interval_ms),
                );
                pin_mut!(stream);
                let mut results = Vec::new();
                while let Some(result) = stream.next().await {
                    results.push(normalize_named(&result?));
                    if results.len() >= count {
                        break;
                    }
                }
                json!({"status": "success", "results": results})
            } else {
                let stream = poll_with_comment_encoding(
                    &client,
                    &addresses,
                    std::time::Duration::from_millis(interval_ms),
                    parse_comment_encoding(&comment_encoding)?,
                );
                pin_mut!(stream);
                let mut results = Vec::new();
                while let Some(result) = stream.next().await {
                    results.push(normalize_named(&result?));
                    if results.len() >= count {
                        break;
                    }
                }
                json!({"status": "success", "results": results})
            }
        }
        "read-words" => {
            if extra.is_empty() {
                json!({"status": "error", "message": "read-words requires count"})
            } else {
                let values = read_words(&client, &address, extra[0].parse()?).await?;
                json!({"status": "success", "values": values.into_iter().map(|value| value.to_string()).collect::<Vec<_>>()})
            }
        }
        "read-dwords" => {
            if extra.is_empty() {
                json!({"status": "error", "message": "read-dwords requires count"})
            } else {
                let values = read_dwords(&client, &address, extra[0].parse()?).await?;
                json!({"status": "success", "values": values.into_iter().map(|value| value.to_string()).collect::<Vec<_>>()})
            }
        }
        "read-expansion" => {
            if extra.len() < 2 || dtype.trim().is_empty() {
                json!({"status": "error", "message": "read-expansion requires buffer address, count, and dtype"})
            } else {
                let values = client
                    .read_expansion_unit_buffer(
                        address.parse()?,
                        extra[0].parse()?,
                        extra[1].parse()?,
                        &dtype,
                    )
                    .await?;
                json!({"status": "success", "values": values})
            }
        }
        "write-expansion" => {
            if extra.len() < 2 || dtype.trim().is_empty() {
                json!({"status": "error", "message": "write-expansion requires buffer address, values, and dtype"})
            } else {
                let values = extra[1..]
                    .iter()
                    .map(|item| parse_typed_value(&dtype, item))
                    .collect::<Result<Vec<_>, _>>()?;
                client
                    .write_expansion_unit_buffer(
                        address.parse()?,
                        extra[0].parse()?,
                        &values,
                        &dtype,
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

fn parse_comment_encoding(value: &str) -> Result<HostLinkCommentEncoding, HostLinkError> {
    match value {
        "utf-8" => Ok(HostLinkCommentEncoding::Utf8),
        "cp932" => Ok(HostLinkCommentEncoding::Cp932),
        _ => Err(HostLinkError::protocol(
            "comment encoding must be exactly utf-8 or cp932",
        )),
    }
}

fn normalize_catalog(catalog: &KvDeviceRangeCatalog) -> Value {
    json!({
        "plcProfile": catalog.plc_profile,
        "modelCode": catalog.model_code,
        "hasModelCode": catalog.has_model_code,
        "requestedPlcProfile": catalog.requested_plc_profile,
        "resolvedPlcProfile": catalog.resolved_plc_profile,
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
        "TCP" => Ok(HostLinkTransportMode::Tcp),
        "UDP" => Ok(HostLinkTransportMode::Udp),
        _ => Err(format!("Unsupported transport: {raw}").into()),
    }
}

fn parse_monitor_words(
    devices: &[String],
) -> Result<Vec<HostLinkMonitorWord>, Box<dyn std::error::Error>> {
    devices
        .iter()
        .map(|entry| {
            let (device, data_format) = entry.split_once(':').ok_or(
                "monitor word entries require DEVICE:FORMAT, for example DM0:U or MR0:BIT",
            )?;
            if data_format.eq_ignore_ascii_case("BIT") {
                Ok(HostLinkMonitorWord::direct_bit(device))
            } else {
                Ok(HostLinkMonitorWord::numeric(device, data_format))
            }
        })
        .collect()
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

fn normalize_named(values: &plc_comm_kv_hostlink::NamedReadResult) -> Value {
    let mut map = serde_json::Map::new();
    for (key, value) in values {
        map.insert(key.clone(), normalize_value(value));
    }
    Value::Object(map)
}
