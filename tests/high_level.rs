use encoding_rs::SHIFT_JIS;
use futures_util::{StreamExt, pin_mut};
use plc_comm_hostlink::{
    HostLinkClient, HostLinkConnectionOptions, HostLinkValue, open_and_connect, read_comments,
    read_dwords_chunked, read_typed, write_dwords_chunked,
};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn read_named_batches_contiguous_word_reads() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDS DM100.U 8" => "1025 65535 2 1 57920 1 0 16712".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let result = client
        .read_named(&[
            "DM100", "DM100.0", "DM100.A", "DM101:S", "DM102:D", "DM104:L", "DM106:F",
        ])
        .await
        .unwrap();

    assert_eq!(result["DM100"], HostLinkValue::U16(1025));
    assert_eq!(result["DM100.0"], HostLinkValue::Bool(true));
    assert_eq!(result["DM100.A"], HostLinkValue::Bool(true));
    assert_eq!(result["DM101:S"], HostLinkValue::I16(-1));
    assert_eq!(result["DM102:D"], HostLinkValue::U32(65_538));
    assert_eq!(result["DM104:L"], HostLinkValue::I32(123_456));
    assert_eq!(result["DM106:F"], HostLinkValue::F32(12.5));

    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDS DM100.U 8"]
    );
}

#[tokio::test]
async fn read_typed_and_write_typed_support_float_suffix() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDS DM200.U 2" => "0 16712".to_owned(),
        "WRS DM200.U 2 0 16712" => "OK".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let value = read_typed(&client, "DM200", "F").await.unwrap();
    client.write_typed("DM200", "F", 12.5f32).await.unwrap();

    assert_eq!(value, HostLinkValue::F32(12.5));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDS DM200.U 2", "WRS DM200.U 2 0 16712"]
    );
}

#[tokio::test]
async fn read_named_direct_bits_use_unsuffixed_rd_commands() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDS R000 1" => "1".to_owned(),
        "RDS CR000 1" => "0".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let result = client.read_named(&["R0", "CR0"]).await.unwrap();
    assert_eq!(result["R0"], HostLinkValue::Bool(true));
    assert_eq!(result["CR0"], HostLinkValue::Bool(false));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDS R000 1", "RDS CR000 1"]
    );
}

#[tokio::test]
async fn read_named_batches_contiguous_direct_bit_reads() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDS R000 4" => "1 0 1 0".to_owned(),
        "RDS CR000 4" => "0 1 0 1".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let result = client
        .read_named(&["R0", "R1", "R2", "R3", "CR0", "CR1", "CR2", "CR3"])
        .await
        .unwrap();

    assert_eq!(result["R0"], HostLinkValue::Bool(true));
    assert_eq!(result["R1"], HostLinkValue::Bool(false));
    assert_eq!(result["R2"], HostLinkValue::Bool(true));
    assert_eq!(result["R3"], HostLinkValue::Bool(false));
    assert_eq!(result["CR0"], HostLinkValue::Bool(false));
    assert_eq!(result["CR1"], HostLinkValue::Bool(true));
    assert_eq!(result["CR2"], HostLinkValue::Bool(false));
    assert_eq!(result["CR3"], HostLinkValue::Bool(true));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDS R000 4", "RDS CR000 4"]
    );
}

#[tokio::test]
async fn read_typed_empty_dtype_uses_device_default_format() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RD CR000" => "1".to_owned(),
        "RD DM200.S" => "-12".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    assert_eq!(
        read_typed(&client, "CR0", "").await.unwrap(),
        HostLinkValue::Bool(true)
    );
    assert_eq!(
        read_typed(&client, "DM200.S", "").await.unwrap(),
        HostLinkValue::I16(-12)
    );
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RD CR000", "RD DM200.S"]
    );
}

#[tokio::test]
async fn read_comments_helper_and_named_snapshot_support_comment_values() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDC DM150" => "MAIN COMMENT                    ".to_owned(),
        "RD DM100.U" => "321".to_owned(),
        "RDC DM101" => "ALARM COMMENT                   ".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let comment = read_comments(&client, "DM150", true).await.unwrap();
    assert_eq!(comment, "MAIN COMMENT");

    let result = client
        .read_named(&["DM100", "DM101:COMMENT"])
        .await
        .unwrap();
    assert_eq!(result["DM100"], HostLinkValue::U16(321));
    assert_eq!(
        result["DM101:COMMENT"],
        HostLinkValue::Text("ALARM COMMENT".to_owned())
    );
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDC DM150", "RD DM100.U", "RDC DM101"]
    );
}

#[tokio::test]
async fn read_comments_decodes_shift_jis_payloads() {
    let (port, received) = start_scripted_server_bytes(|command| match command.as_str() {
        "RDC DM20" => {
            let (encoded, _, _) = SHIFT_JIS.encode("運転許可");
            let mut bytes = encoded.into_owned();
            bytes.extend_from_slice(b"                    ");
            bytes
        }
        _ => b"E1".to_vec(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let comment = read_comments(&client, "DM20", true).await.unwrap();

    assert_eq!(comment, "運転許可");
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDC DM20"]
    );
}

#[tokio::test]
async fn open_and_connect_returns_queued_client_that_uses_helper_api() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RD DM10.U" => "123".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = open_and_connect(options).await.unwrap();
    let value = client.read_typed("DM10", "U").await.unwrap();

    assert!(client.is_open().await);
    assert_eq!(value, HostLinkValue::U16(123));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RD DM10.U"]
    );
}

#[tokio::test]
async fn queued_client_supports_read_comments() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDC DM10" => "ALARM TEXT                      ".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = open_and_connect(options).await.unwrap();
    let comment = client.read_comments("DM10", true).await.unwrap();

    assert_eq!(comment, "ALARM TEXT");
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDC DM10"]
    );
}

#[tokio::test]
async fn read_comments_accepts_xym_alias_device_types() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDC D10" => "DM COMMENT                      ".to_owned(),
        "RDC M20" => "MR COMMENT                      ".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();
    let data_memory_comment = client.read_comments("D10", true).await.unwrap();
    let auxiliary_relay_comment = client.read_comments("M20", true).await.unwrap();

    assert_eq!(data_memory_comment, "DM COMMENT");
    assert_eq!(auxiliary_relay_comment, "MR COMMENT");
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDC D10", "RDC M20"]
    );
}

#[tokio::test]
async fn poll_reuses_compiled_plan_for_each_cycle() {
    let responses = Arc::new(Mutex::new(0usize));
    let state = Arc::clone(&responses);
    let (port, received) = start_scripted_server(move |command| {
        assert_eq!(command, "RDS DM100.U 3");
        let mut counter = state.lock().unwrap();
        let response = if *counter == 0 {
            "1 0 16320"
        } else {
            "3 0 16416"
        };
        *counter += 1;
        response.to_owned()
    })
    .await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let stream = plc_comm_hostlink::poll(
        &client,
        &["DM100", "DM100.0", "DM101:F"],
        std::time::Duration::from_millis(1),
    );
    pin_mut!(stream);
    let first = stream.next().await.unwrap().unwrap();
    let second = stream.next().await.unwrap().unwrap();

    assert_eq!(first["DM100"], HostLinkValue::U16(1));
    assert_eq!(first["DM100.0"], HostLinkValue::Bool(true));
    assert_eq!(first["DM101:F"], HostLinkValue::F32(1.5));
    assert_eq!(second["DM100"], HostLinkValue::U16(3));
    assert_eq!(second["DM100.0"], HostLinkValue::Bool(true));
    assert_eq!(second["DM101:F"], HostLinkValue::F32(2.5));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDS DM100.U 3", "RDS DM100.U 3"]
    );
}

#[tokio::test]
async fn read_dwords_chunked_advances_by_whole_dword_boundaries() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDS DM200.U 2" => "1 1".to_owned(),
        "RDS DM202.U 2" => "2 2".to_owned(),
        "RDS DM204.U 2" => "3 3".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();
    let values = read_dwords_chunked(&client, "DM200", 3, 1).await.unwrap();

    assert_eq!(values, vec![65_537, 131_074, 196_611]);
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDS DM200.U 2", "RDS DM202.U 2", "RDS DM204.U 2"]
    );
}

#[tokio::test]
async fn write_dwords_chunked_advances_by_whole_dword_boundaries() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "WRS DM200.U 2 1 1" => "OK".to_owned(),
        "WRS DM202.U 2 2 2" => "OK".to_owned(),
        "WRS DM204.U 2 3 3" => "OK".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();
    write_dwords_chunked(&client, "DM200", &[65_537, 131_074, 196_611], 1)
        .await
        .unwrap();

    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec![
            "WRS DM200.U 2 1 1",
            "WRS DM202.U 2 2 2",
            "WRS DM204.U 2 3 3"
        ]
    );
}

#[tokio::test]
async fn read_rejects_32_bit_device_end_crossing_before_send() {
    let (port, received) = start_scripted_server(|_| "OK".to_owned()).await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();
    let error = read_typed(&client, "DM65534", "D").await.unwrap_err();

    assert!(error.to_string().contains("Device span out of range"));
    assert!(received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn read_expansion_unit_buffer_rejects_32_bit_buffer_end_crossing_before_send() {
    let (port, received) = start_scripted_server(|_| "OK".to_owned()).await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();
    let error = client
        .read_expansion_unit_buffer(1, 59_999, 1, Some("D"))
        .await
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("Expansion buffer span out of range")
    );
    assert!(received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn read_device_range_catalog_resolves_query_model_into_range_catalog() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "?K" => "58".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new("127.0.0.1");
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();
    let catalog = client.read_device_range_catalog().await.unwrap();

    assert_eq!(catalog.model, "KV-8000");
    assert_eq!(catalog.model_code, "58");
    assert!(catalog.has_model_code);
    assert_eq!(catalog.requested_model, "KV-8000A");
    assert_eq!(catalog.resolved_model, "KV-8000");
    assert_eq!(
        catalog.entry("DM").unwrap().address_range.as_deref(),
        Some("DM00000-DM65534")
    );
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["?K"]
    );
}

async fn start_scripted_server<F>(response_factory: F) -> (u16, Arc<Mutex<Vec<String>>>)
where
    F: Fn(String) -> String + Send + Sync + 'static,
{
    start_scripted_server_bytes(move |command| response_factory(command).into_bytes()).await
}

async fn start_scripted_server_bytes<F>(response_factory: F) -> (u16, Arc<Mutex<Vec<String>>>)
where
    F: Fn(String) -> Vec<u8> + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let received = Arc::new(Mutex::new(Vec::new()));
    let queue = Arc::clone(&received);
    let response_factory = Arc::new(response_factory);

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
                    if partial.is_empty() {
                        continue;
                    }
                    let command = String::from_utf8(std::mem::take(&mut partial)).unwrap();
                    queue.lock().unwrap().push(command.clone());
                    let response = response_factory(command);
                    let mut frame = response;
                    frame.extend_from_slice(b"\r\n");
                    stream.write_all(&frame).await.unwrap();
                } else {
                    partial.push(*byte);
                }
            }
        }
    });
    (port, received)
}
