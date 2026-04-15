use futures_util::{StreamExt, pin_mut};
use plc_comm_hostlink::{
    HostLinkConnectionOptions, HostLinkValue, open_and_connect, read_dwords, read_named,
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
            _ => {
                extra.push(args[index].clone());
                index += 1;
            }
        }
    }

    let mut options = HostLinkConnectionOptions::new(host.clone());
    options.port = port;
    let client = open_and_connect(options).await?;

    let result = match command.as_str() {
        "write-typed" => {
            if dtype.trim().is_empty() || extra.is_empty() {
                json!({"status": "error", "message": "write-typed requires --dtype and one value"})
            } else {
                let value = parse_typed_value(&dtype, &extra[0])?;
                client.write_typed(&address, &dtype, value).await?;
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
        _ => json!({"status": "error", "message": format!("Unknown command: {command}")}),
    };

    let _ = client.close().await;
    Ok(result)
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

fn normalize_value(value: &HostLinkValue) -> Value {
    match value {
        HostLinkValue::Bool(value) => json!(value),
        HostLinkValue::F32(value) => json!(
            format!("{value:.9}")
                .trim_end_matches('0')
                .trim_end_matches('.')
        ),
        HostLinkValue::U16(value) => json!(value.to_string()),
        HostLinkValue::I16(value) => json!(value.to_string()),
        HostLinkValue::U32(value) => json!(value.to_string()),
        HostLinkValue::I32(value) => json!(value.to_string()),
    }
}

fn normalize_named(values: &plc_comm_hostlink::NamedSnapshot) -> Value {
    let mut map = serde_json::Map::new();
    for (key, value) in values {
        map.insert(key.clone(), normalize_value(value));
    }
    Value::Object(map)
}
