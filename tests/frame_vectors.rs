use plc_comm_kv_hostlink::{
    HostLinkClient, HostLinkClock, HostLinkConnectionOptions, HostLinkMonitorWord,
    HostLinkTransportMode, KvPlcMode, parse_device,
};
use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Deserialize)]
struct VectorFile {
    vectors: Vec<FrameVector>,
}

#[derive(Debug, Clone, Deserialize)]
struct FrameVector {
    id: String,
    command: String,
    device: Option<String>,
    devices: Option<Vec<String>>,
    count: Option<usize>,
    value: Option<i32>,
    values: Option<Vec<i32>>,
    mode: Option<String>,
    response: Option<String>,
    bank_no: Option<u8>,
    unit_no: Option<u8>,
    buffer_address: Option<u32>,
    expected_body: String,
    data_format: Option<String>,
}

#[tokio::test]
async fn frame_vectors_send_expected_bodies() {
    let vectors =
        serde_json::from_str::<VectorFile>(include_str!("vectors/hostlink_frame_vectors.json"))
            .unwrap()
            .vectors;

    for vector in vectors {
        let (port, received) = start_echo_server(vector.response.clone()).await;
        let mut options = HostLinkConnectionOptions::new(
            "127.0.0.1",
            8501,
            HostLinkTransportMode::Tcp,
            "keyence:kv-8000",
        )
        .unwrap();
        options.port = port;
        let client = HostLinkClient::connect(options).await.unwrap();
        let _ = run_vector(&client, &vector).await;
        client.close().await.unwrap();

        let actual = received
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| panic!("[{}] no frame was captured", vector.id));
        assert_eq!(actual, vector.expected_body, "vector {}", vector.id);
    }
}

async fn start_echo_server(response: Option<String>) -> (u16, Arc<Mutex<VecDeque<String>>>) {
    let received = Arc::new(Mutex::new(VecDeque::new()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let queue = Arc::clone(&received);
    let response = response.unwrap_or_else(|| "OK".to_owned());
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buffer = [0u8; 4096];
        let mut partial = Vec::new();
        loop {
            let read = stream.read(&mut buffer).await.unwrap();
            if read == 0 {
                break;
            }
            for byte in &buffer[..read] {
                if matches!(byte, b'\r' | b'\n') {
                    if !partial.is_empty() {
                        queue
                            .lock()
                            .unwrap()
                            .push_back(String::from_utf8(partial.clone()).unwrap());
                        partial.clear();
                    }
                } else {
                    partial.push(*byte);
                }
            }
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.write_all(b"\r\n").await.unwrap();
        }
    });
    (port, received)
}

async fn run_vector(
    client: &HostLinkClient,
    vector: &FrameVector,
) -> Result<(), plc_comm_kv_hostlink::HostLinkError> {
    match vector.command.as_str() {
        "check_error_no" => {
            let _ = client.check_error_no().await?;
        }
        "query_model" => {
            let _ = client.query_model().await?;
        }
        "confirm_operating_mode" => {
            let _ = client.confirm_operating_mode().await?;
        }
        "forced_set" => {
            client.forced_set(vector.device.as_deref().unwrap()).await?;
        }
        "forced_reset" => {
            client
                .forced_reset(vector.device.as_deref().unwrap())
                .await?;
        }
        "read" => {
            let (device, format) = device_and_format(vector.device.as_deref().unwrap())?;
            client.read(&device, format.as_deref()).await?;
        }
        "read_consecutive" => {
            let (device, format) = device_and_format(vector.device.as_deref().unwrap())?;
            client
                .read_consecutive(&device, vector.count.unwrap(), format.as_deref())
                .await?;
        }
        "register_monitor_bits" => {
            let devices = vector.devices.as_ref().unwrap();
            client.register_monitor_bits(devices).await?;
        }
        "register_monitor_words" => {
            let devices = vector.devices.as_ref().unwrap();
            let entries = devices
                .iter()
                .map(|device| monitor_entry(device))
                .collect::<Result<Vec<_>, _>>()?;
            client.register_monitor_words(&entries).await?;
        }
        "read_monitor_bits" => {
            let _ = client.read_monitor_bits().await?;
        }
        "read_monitor_words" => {
            let _ = client.read_monitor_words().await?;
        }
        "forced_set_consecutive" => {
            client
                .forced_set_consecutive(vector.device.as_deref().unwrap(), vector.count.unwrap())
                .await?;
        }
        "forced_reset_consecutive" => {
            client
                .forced_reset_consecutive(vector.device.as_deref().unwrap(), vector.count.unwrap())
                .await?;
        }
        "write" => {
            let (device, format) = device_and_format(vector.device.as_deref().unwrap())?;
            client
                .write(&device, vector.value.unwrap(), format.as_deref())
                .await?;
        }
        "write_consecutive" => {
            let values = vector.values.clone().unwrap();
            let (device, format) = device_and_format(vector.device.as_deref().unwrap())?;
            client
                .write_consecutive(&device, &values, format.as_deref())
                .await?;
        }
        "change_mode" => {
            let mode = match vector.mode.as_deref().unwrap() {
                "RUN" => KvPlcMode::Run,
                _ => KvPlcMode::Program,
            };
            client.change_mode(mode).await?;
        }
        "clear_error" => {
            client.clear_error().await?;
        }
        "set_time" => {
            client
                .set_time(HostLinkClock {
                    year: 26,
                    month: 3,
                    day: 13,
                    hour: 22,
                    minute: 5,
                    second: 9,
                    week: 5,
                })
                .await?;
        }
        "read_format" => {
            client
                .read(
                    vector.device.as_deref().unwrap(),
                    vector.data_format.as_deref(),
                )
                .await?;
        }
        "read_consecutive_legacy" => {
            let (device, format) = device_and_format(vector.device.as_deref().unwrap())?;
            client
                .read_consecutive_legacy(&device, vector.count.unwrap(), format.as_deref())
                .await?;
        }
        "write_consecutive_legacy" => {
            let values = vector.values.clone().unwrap();
            let (device, format) = device_and_format(vector.device.as_deref().unwrap())?;
            client
                .write_consecutive_legacy(&device, &values, format.as_deref())
                .await?;
        }
        "write_set_value" => {
            let (device, format) = device_and_format(vector.device.as_deref().unwrap())?;
            client
                .write_set_value(&device, vector.value.unwrap(), format.as_deref())
                .await?;
        }
        "write_set_value_consecutive" => {
            let values = vector.values.clone().unwrap();
            let (device, format) = device_and_format(vector.device.as_deref().unwrap())?;
            client
                .write_set_value_consecutive(&device, &values, format.as_deref())
                .await?;
        }
        "switch_bank" => {
            client.switch_bank(vector.bank_no.unwrap()).await?;
        }
        "read_expansion_unit_buffer" => {
            let _ = client
                .read_expansion_unit_buffer(
                    vector.unit_no.unwrap(),
                    vector.buffer_address.unwrap(),
                    vector.count.unwrap(),
                    vector.data_format.as_deref().unwrap(),
                )
                .await?;
        }
        "write_expansion_unit_buffer" => {
            let values = vector.values.clone().unwrap();
            client
                .write_expansion_unit_buffer(
                    vector.unit_no.unwrap(),
                    vector.buffer_address.unwrap(),
                    &values,
                    vector.data_format.as_deref().unwrap(),
                )
                .await?;
        }
        "read_comments" => {
            let _ = client
                .read_comments(vector.device.as_deref().unwrap())
                .await?;
        }
        other => panic!("unsupported vector command: {other}"),
    }
    Ok(())
}

fn device_and_format(
    device: &str,
) -> Result<(String, Option<String>), plc_comm_kv_hostlink::HostLinkError> {
    let mut parsed = parse_device(device)?;
    let format = (!parsed.suffix.is_empty()).then(|| parsed.suffix.clone());
    parsed.suffix.clear();
    Ok((parsed.to_text()?, format))
}

fn monitor_entry(device: &str) -> Result<HostLinkMonitorWord, plc_comm_kv_hostlink::HostLinkError> {
    let (device, format) = device_and_format(device)?;
    Ok(match format {
        Some(format) => HostLinkMonitorWord::numeric(device, format),
        None => HostLinkMonitorWord::direct_bit(device),
    })
}
