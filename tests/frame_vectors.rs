use plc_comm_hostlink::{HostLinkClient, HostLinkClock, HostLinkConnectionOptions, KvPlcMode};
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
        let mut options = HostLinkConnectionOptions::new("127.0.0.1");
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
) -> Result<(), plc_comm_hostlink::HostLinkError> {
    match vector.command.as_str() {
        "check_error_no" => {
            let _ = client.check_error_no().await?;
        }
        "query_model" => {
            let _ = client.query_model().await?;
        }
        "read_device_range_catalog" => {
            let _ = client.read_device_range_catalog().await?;
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
            client.read(vector.device.as_deref().unwrap(), None).await?;
        }
        "read_consecutive" => {
            client
                .read_consecutive(
                    vector.device.as_deref().unwrap(),
                    vector.count.unwrap(),
                    None,
                )
                .await?;
        }
        "register_monitor_bits" => {
            let devices = vector.devices.as_ref().unwrap();
            client.register_monitor_bits(devices).await?;
        }
        "register_monitor_words" => {
            let devices = vector.devices.as_ref().unwrap();
            client.register_monitor_words(devices).await?;
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
            client
                .write(
                    vector.device.as_deref().unwrap(),
                    vector.value.unwrap(),
                    None,
                )
                .await?;
        }
        "write_consecutive" => {
            let values = vector.values.clone().unwrap();
            client
                .write_consecutive(vector.device.as_deref().unwrap(), &values, None)
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
                .set_time(Some(HostLinkClock {
                    year: 26,
                    month: 3,
                    day: 13,
                    hour: 22,
                    minute: 5,
                    second: 9,
                    week: 5,
                }))
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
            client
                .read_consecutive_legacy(
                    vector.device.as_deref().unwrap(),
                    vector.count.unwrap(),
                    None,
                )
                .await?;
        }
        "write_consecutive_legacy" => {
            let values = vector.values.clone().unwrap();
            client
                .write_consecutive_legacy(
                    vector.device.as_deref().unwrap(),
                    &values,
                    vector.data_format.as_deref(),
                )
                .await?;
        }
        "write_set_value" => {
            client
                .write_set_value(
                    vector.device.as_deref().unwrap(),
                    vector.value.unwrap(),
                    None,
                )
                .await?;
        }
        "write_set_value_consecutive" => {
            let values = vector.values.clone().unwrap();
            client
                .write_set_value_consecutive(
                    vector.device.as_deref().unwrap(),
                    &values,
                    vector.data_format.as_deref(),
                )
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
                    vector.data_format.as_deref(),
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
                    vector.data_format.as_deref(),
                )
                .await?;
        }
        "read_comments" => {
            let _ = client
                .read_comments(vector.device.as_deref().unwrap(), true)
                .await?;
        }
        other => panic!("unsupported vector command: {other}"),
    }
    Ok(())
}
