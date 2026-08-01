use encoding_rs::SHIFT_JIS;
use futures_util::{StreamExt, pin_mut};
use plc_comm_kv_hostlink::{
    HostLinkAddress, HostLinkClient, HostLinkCommentEncoding, HostLinkConnectionOptions,
    HostLinkError, HostLinkMonitorWord, HostLinkOutcomeUnknownReason, HostLinkPayloadValue,
    HostLinkTransportMode, HostLinkValue, KvDeviceAddress, KvLogicalAddress, open_and_connect,
    read_comment_bytes, read_comments, read_dwords, read_typed, read_words,
    write_dwords_single_request,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::Notify;

#[tokio::test]
async fn refused_tcp_connection_is_transport_failure_and_not_adopted() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let error = match HostLinkClient::connect(options).await {
        Ok(_) => panic!("connection to a released listener port unexpectedly succeeded"),
        Err(error) => error,
    };

    assert!(matches!(error, HostLinkError::Transport { .. }));
}

#[tokio::test]
async fn read_named_batches_contiguous_word_reads() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDS DM100.U 8" => "1025 65535 2 1 57920 1 0 16712".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let result = client
        .read_named(&[
            "DM100:U", "DM100.0", "DM100.A", "DM101:S", "DM102:D", "DM104:L", "DM106:F",
        ])
        .await
        .unwrap();

    assert_eq!(result["DM100:U"], HostLinkValue::U16(1025));
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
async fn read_named_preserves_descending_input_wire_order() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDS DM10.U 1" => "10".to_owned(),
        "RDS DM9.U 1" => "9".to_owned(),
        "RDS DM11.U 1" => "11".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let result = client
        .read_named(&["DM10:U", "DM9:U", "DM11:U"])
        .await
        .unwrap();

    assert_eq!(result["DM10:U"], HostLinkValue::U16(10));
    assert_eq!(result["DM9:U"], HostLinkValue::U16(9));
    assert_eq!(result["DM11:U"], HostLinkValue::U16(11));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDS DM10.U 1", "RDS DM9.U 1", "RDS DM11.U 1"]
    );
}

#[tokio::test]
async fn read_named_prevalidates_every_address_before_first_send() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDS DM0.U 3" => "1 2 3".to_owned(),
        _ => "1".to_owned(),
    })
    .await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let error = client
        .read_named(&["DM0:U", "not-a-device:U"])
        .await
        .unwrap_err();

    assert!(matches!(error, HostLinkError::Protocol(_)));
    assert!(received.lock().unwrap().is_empty());
    assert_eq!(client.traffic_stats().await.request_count, 0);

    for duplicates in [
        ["DM0:U", "DM0:U"],
        ["dm0:u", "DM0000:U"],
        ["R0:BIT", "R000:BIT"],
    ] {
        let error = client.read_named(&duplicates).await.unwrap_err();
        assert!(matches!(&error, HostLinkError::Protocol(_)));
        assert!(error.to_string().contains("semantically duplicated"));
        assert!(received.lock().unwrap().is_empty());
        assert_eq!(client.traffic_stats().await.request_count, 0);
    }

    let values = client
        .read_named(&["dm0:u", "DM0:S", "DM0.0", "DM0.1", "DM0:D", "DM1:D"])
        .await
        .unwrap();
    assert_eq!(
        values.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["dm0:u", "DM0:S", "DM0.0", "DM0.1", "DM0:D", "DM1:D"]
    );
    assert_eq!(received.lock().unwrap().as_slice(), ["RDS DM0.U 3"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_named_owns_one_fifo_turn_for_its_complete_result() {
    let (port, received) = start_scripted_server(|command| {
        if command == "RDS DM0.U 1" {
            std::thread::sleep(Duration::from_millis(60));
            return "1".to_owned();
        }
        match command.as_str() {
            "RDS TM0.U 1" => "2".to_owned(),
            "?K" => "01".to_owned(),
            _ => "E1".to_owned(),
        }
    })
    .await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let named_client = client.clone();
    let named = tokio::spawn(async move { named_client.read_named(&["DM0:U", "TM0:U"]).await });
    for _ in 0..100 {
        if !received.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert_eq!(received.lock().unwrap().as_slice(), ["RDS DM0.U 1"]);

    let query_client = client.clone();
    let query = tokio::spawn(async move { query_client.query_model().await });
    named.await.unwrap().unwrap();
    query.await.unwrap().unwrap();

    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDS DM0.U 1", "RDS TM0.U 1", "?K"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_rejects_active_and_waiting_generation_then_reopen_accepts_only_new_work() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let first_seen = Arc::new(Notify::new());
    let server_seen = Arc::new(Mutex::new(Vec::new()));
    let server_notify = Arc::clone(&first_seen);
    let server_records = Arc::clone(&server_seen);
    tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.unwrap();
        let first_command = read_test_command(&mut first).await;
        server_records.lock().unwrap().push(first_command);
        server_notify.notify_one();
        let mut eof = [0u8; 1];
        while first.read(&mut eof).await.unwrap_or(0) != 0 {}

        let (mut second, _) = listener.accept().await.unwrap();
        let second_command = read_test_command(&mut second).await;
        server_records.lock().unwrap().push(second_command);
        second.write_all(b"01\r").await.unwrap();
    });

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let active_client = client.clone();
    let active = tokio::spawn(async move { active_client.send_raw("WRITE").await });
    first_seen.notified().await;
    let waiting_client = client.clone();
    let waiting = tokio::spawn(async move { waiting_client.query_model().await });
    tokio::time::sleep(Duration::from_millis(10)).await;

    client.close().await.unwrap();
    let active_error = active.await.unwrap().unwrap_err();
    assert!(matches!(
        active_error,
        HostLinkError::OutcomeUnknown {
            reason: HostLinkOutcomeUnknownReason::Closed,
            ..
        }
    ));
    assert!(matches!(waiting.await.unwrap(), Err(HostLinkError::Closed)));

    client.open().await.unwrap();
    client.query_model().await.unwrap();
    assert_eq!(server_seen.lock().unwrap().as_slice(), ["WRITE", "?K"]);
}

#[tokio::test]
async fn malformed_state_changing_response_is_outcome_unknown_without_retry() {
    let (port, received) = start_scripted_server(|_| "MAYBE".to_owned()).await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let error = client
        .change_mode(plc_comm_kv_hostlink::KvPlcMode::Run)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        HostLinkError::OutcomeUnknown {
            reason: HostLinkOutcomeUnknownReason::MalformedResponse,
            ..
        }
    ));
    assert!(!client.is_open().await);
    assert_eq!(received.lock().unwrap().as_slice(), ["M1"]);
}

#[tokio::test]
async fn direct_bit_writes_accept_only_bool_values_before_transport() {
    let (port, received) = start_scripted_server(|_| "OK".to_owned()).await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    assert!(matches!(
        client.write("R0", 1_u8, None).await,
        Err(HostLinkError::Protocol(_))
    ));
    assert!(matches!(
        client.write_typed("R0", "BIT", 1_u8).await,
        Err(HostLinkError::Protocol(_))
    ));
    assert!(matches!(
        client.write_consecutive("R0", &[true, false], None).await,
        Ok(())
    ));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["WRS R000 2 1 0"]
    );
}

#[tokio::test]
async fn raw_request_accepts_exact_capacity_and_rejects_one_over_without_state_change() {
    let (port, received) = start_scripted_server(|_| "OK".to_owned()).await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let maximum = "A".repeat(65_536);
    assert_eq!(client.send_raw(&maximum).await.unwrap(), b"OK");
    let before = client.traffic_stats().await;
    let error = client.send_raw(&"A".repeat(65_537)).await.unwrap_err();
    assert!(matches!(error, HostLinkError::Protocol(_)));
    assert!(client.is_open().await);
    assert_eq!(client.traffic_stats().await, before);
    assert_eq!(before.request_count, 1);
    assert_eq!(before.tx_bytes, 65_537);
    assert_eq!(received.lock().unwrap().len(), 1);
    assert_eq!(received.lock().unwrap()[0].len(), 65_536);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_a_waiting_fifo_operation_sends_nothing() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let received = Arc::new(Mutex::new(Vec::new()));
    let first_seen = Arc::new(Notify::new());
    let release_first = Arc::new(Notify::new());
    let server_received = Arc::clone(&received);
    let server_seen = Arc::clone(&first_seen);
    let server_release = Arc::clone(&release_first);
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let first = read_test_command(&mut stream).await;
        server_received.lock().unwrap().push(first);
        server_seen.notify_one();
        server_release.notified().await;
        stream.write_all(b"01\r").await.unwrap();
        let second = read_test_command(&mut stream).await;
        server_received.lock().unwrap().push(second);
        stream.write_all(b"1\r").await.unwrap();
    });
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let active_client = client.clone();
    let active = tokio::spawn(async move { active_client.query_model().await });
    first_seen.notified().await;
    let mut waiting = Box::pin(client.read("DM99", Some("U")));
    let waiting_poll = tokio::time::timeout(Duration::from_millis(5), waiting.as_mut()).await;
    assert!(waiting_poll.is_err(), "poll={waiting_poll:?}");
    drop(waiting);
    release_first.notify_one();
    active.await.unwrap().unwrap();
    client.read("DM0", Some("U")).await.unwrap();

    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["?K", "RD DM0.U"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn timeout_is_snapshotted_when_an_operation_enters_fifo() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let first_seen = Arc::new(Notify::new());
    let release_first = Arc::new(Notify::new());
    let server_seen = Arc::clone(&first_seen);
    let server_release = Arc::clone(&release_first);
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        assert_eq!(read_test_command(&mut stream).await, "?K");
        server_seen.notify_one();
        server_release.notified().await;
        stream.write_all(b"01\r").await.unwrap();

        assert_eq!(read_test_command(&mut stream).await, "RD DM0.U");
        tokio::time::sleep(Duration::from_millis(45)).await;
        stream.write_all(b"1\r").await.unwrap();

        assert_eq!(read_test_command(&mut stream).await, "RD DM1.U");
        tokio::time::sleep(Duration::from_millis(45)).await;
        let _ = stream.write_all(b"1\r").await;
    });
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    options.timeout = Duration::from_millis(200);
    let client = HostLinkClient::connect(options).await.unwrap();

    let active_client = client.clone();
    let active = tokio::spawn(async move { active_client.query_model().await });
    first_seen.notified().await;
    let mut setter = Box::pin(client.set_timeout(Duration::from_millis(20)));
    let setter_poll = tokio::time::timeout(Duration::from_millis(5), setter.as_mut()).await;
    assert!(setter_poll.is_err(), "poll={setter_poll:?}");
    let mut admitted = Box::pin(client.read("DM0", Some("U")));
    assert!(
        tokio::time::timeout(Duration::from_millis(5), admitted.as_mut())
            .await
            .is_err()
    );

    release_first.notify_one();
    active.await.unwrap().unwrap();
    setter.await.unwrap();
    admitted.await.unwrap();
    assert_eq!(client.timeout().await, Duration::from_millis(20));

    let error = client.read("DM1", Some("U")).await.unwrap_err();
    assert!(matches!(error, HostLinkError::Timeout(_)));
}

#[tokio::test]
async fn post_send_transport_failure_on_write_is_outcome_unknown() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        assert_eq!(read_test_command(&mut stream).await, "WR DM0.U 1");
    });
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let error = client.write("DM0", 1_u16, Some("U")).await.unwrap_err();
    assert!(matches!(
        error,
        HostLinkError::OutcomeUnknown {
            reason: HostLinkOutcomeUnknownReason::Transport,
            ..
        }
    ));
    assert!(!client.is_open().await);
}

#[tokio::test]
async fn udp_send_raw_accepts_large_datagram_response() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut request = vec![0u8; 1024];
        let (_read, peer) = socket.recv_from(&mut request).await.unwrap();
        let mut response = "7".repeat(5000).into_bytes();
        response.extend_from_slice(b"\r\n");
        socket.send_to(&response, peer).await.unwrap();
    });

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.transport = HostLinkTransportMode::Udp;
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();
    assert_eq!(client.traffic_stats().await, Default::default());

    let response = client.send_raw("LARGE").await.unwrap();

    assert_eq!(response.len(), 5000);
    assert!(response.iter().all(|byte| *byte == b'7'));
    let stats = client.traffic_stats().await;
    assert_eq!(stats.request_count, 1);
    assert_eq!(stats.tx_bytes, b"LARGE\r".len() as u64);
    assert_eq!(stats.rx_bytes, 5002);
    client.close().await.unwrap();
    assert_eq!(client.traffic_stats().await, stats);
}

#[tokio::test]
async fn udp_missing_terminator_is_rejected_and_transport_is_closed() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let mut request = vec![0u8; 1024];
        let (_, peer) = socket.recv_from(&mut request).await.unwrap();
        socket.send_to(b"UNTERMINATED", peer).await.unwrap();
    });

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Udp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let error = client.send_raw("READ").await.unwrap_err();
    assert!(error.to_string().contains("terminator"));
    assert!(!client.is_open().await);
    let stats = client.traffic_stats().await;
    assert_eq!(stats.request_count, 1);
    assert_eq!(stats.tx_bytes, b"READ\r".len() as u64);
    assert_eq!(stats.rx_bytes, 0);
    server.await.unwrap();
}

#[tokio::test]
async fn timeout_contract_has_three_second_default_rejects_zero_and_closes_timed_out_tcp() {
    let default_options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    assert_eq!(default_options.timeout, Duration::from_secs(3));
    let unconnected = HostLinkClient::new(default_options);
    assert!(unconnected.set_timeout(Duration::ZERO).await.is_err());
    assert!(unconnected.set_timeout(Duration::MAX).await.is_err());
    assert_eq!(unconnected.timeout().await, Duration::from_secs(3));

    let mut oversized_options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    oversized_options.timeout = Duration::MAX;
    let oversized = HostLinkClient::new(oversized_options);
    let oversized_error = oversized.open().await.unwrap_err();
    assert!(oversized_error.to_string().contains("too large"));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 5];
        stream.read_exact(&mut request).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    });
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    options.timeout = Duration::from_millis(50);
    let client = HostLinkClient::connect(options).await.unwrap();
    let error = client.send_raw("READ").await.unwrap_err();
    assert!(matches!(
        error,
        HostLinkError::OutcomeUnknown {
            reason: HostLinkOutcomeUnknownReason::Timeout,
            ..
        }
    ));
    assert!(!client.is_open().await);
    let stats = client.traffic_stats().await;
    assert_eq!(stats.request_count, 1);
    assert_eq!(stats.tx_bytes, b"READ\r".len() as u64);
    assert_eq!(stats.rx_bytes, 0);
    server.await.unwrap();
}

#[tokio::test]
async fn read_named_direct_bit_word_views_use_independent_sixteen_bit_reads() {
    let first = (0..16)
        .map(|bit| if matches!(bit, 0 | 3 | 15) { "1" } else { "0" })
        .collect::<Vec<_>>()
        .join(" ");
    let second = (0..16)
        .map(|bit| if matches!(bit, 1 | 15) { "1" } else { "0" })
        .collect::<Vec<_>>()
        .join(" ");
    let (port, received) = start_scripted_server(move |command| match command.as_str() {
        "RD M0.U" => first.clone(),
        "RD M1.U" => second.clone(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let result = client.read_named(&["M0:U", "M1:U"]).await.unwrap();

    assert_eq!(result["M0:U"], HostLinkValue::U16(0x8009));
    assert_eq!(result["M1:U"], HostLinkValue::U16(0x8002));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RD M0.U", "RD M1.U"]
    );
}

#[tokio::test]
async fn tcp_timeout_is_one_deadline_for_a_trickled_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 5];
        stream.read_exact(&mut request).await.unwrap();
        for _ in 0..5 {
            tokio::time::sleep(Duration::from_millis(30)).await;
            if stream.write_all(b"A").await.is_err() {
                break;
            }
        }
    });
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    options.timeout = Duration::from_millis(80);
    let client = HostLinkClient::connect(options).await.unwrap();

    let started = tokio::time::Instant::now();
    let error = client.send_raw("READ").await.unwrap_err();
    assert!(matches!(
        error,
        HostLinkError::OutcomeUnknown {
            reason: HostLinkOutcomeUnknownReason::Timeout,
            ..
        }
    ));
    assert!(started.elapsed() < Duration::from_millis(200));
    assert!(!client.is_open().await);
    server.await.unwrap();
}

#[tokio::test]
async fn suffixes_are_rejected_by_non_format_commands_before_transport() {
    let client = HostLinkClient::new(
        HostLinkConnectionOptions::new(
            "127.0.0.1",
            8501,
            HostLinkTransportMode::Tcp,
            "keyence:kv-8000",
        )
        .unwrap(),
    );

    assert!(
        client
            .forced_set("R0.U")
            .await
            .unwrap_err()
            .to_string()
            .contains("suffix")
    );
    assert!(
        client
            .forced_reset("R0.U")
            .await
            .unwrap_err()
            .to_string()
            .contains("suffix")
    );
    assert!(
        client
            .forced_set_consecutive("R0.U", 2)
            .await
            .unwrap_err()
            .to_string()
            .contains("suffix")
    );
    assert!(
        client
            .forced_reset_consecutive("R0.U", 2)
            .await
            .unwrap_err()
            .to_string()
            .contains("suffix")
    );
    assert!(
        client
            .register_monitor_bits(&["R0.U"])
            .await
            .unwrap_err()
            .to_string()
            .contains("suffix")
    );
    assert!(
        client
            .read_comments("DM0.U", HostLinkCommentEncoding::Utf8)
            .await
            .unwrap_err()
            .to_string()
            .contains("suffix")
    );
    assert!(
        client
            .read_timer_counter("T0.U")
            .await
            .unwrap_err()
            .to_string()
            .contains("suffix")
    );
    assert!(!client.is_open().await);
}

#[test]
fn host_link_value_u16_conversion_is_fallible() {
    assert_eq!(u16::try_from(HostLinkValue::U16(42)).unwrap(), 42);
    assert!(u16::try_from(HostLinkValue::U32(42)).is_err());
    assert!(u16::try_from(HostLinkValue::I16(42)).is_err());
    assert!(u16::try_from(HostLinkValue::Bool(true)).is_err());
}

#[tokio::test]
async fn hexadecimal_typed_read_requires_exactly_one_valid_token() {
    for (device, response, expected) in [
        ("DM210", "ABCD EEEE", "Expected 1"),
        ("DM211", "WXYZ", "hexadecimal digits"),
    ] {
        let (port, _) = start_scripted_server(move |_| response.to_owned()).await;
        let mut options = HostLinkConnectionOptions::new(
            "127.0.0.1",
            8501,
            HostLinkTransportMode::Tcp,
            "keyence:kv-8000",
        )
        .unwrap();
        options.port = port;
        let client = HostLinkClient::connect(options).await.unwrap();

        assert!(
            read_typed(&client, device, "H")
                .await
                .unwrap_err()
                .to_string()
                .contains(expected)
        );
        assert!(!client.is_open().await);
    }
}

#[tokio::test]
async fn single_read_token_count_follows_device_and_format() {
    let (port, _) = start_scripted_server(|command| {
        let count = match command.as_str() {
            "RD R000.U" => 16,
            "RD R000.D" => 32,
            "RD DM0.U" => 1,
            _ => return "E1".to_owned(),
        };
        (0..count).map(|_| "0").collect::<Vec<_>>().join(" ")
    })
    .await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    assert_eq!(client.read("R0", Some("U")).await.unwrap().len(), 16);
    assert_eq!(client.read("R0", Some("D")).await.unwrap().len(), 32);
    assert_eq!(client.read("DM0", Some("U")).await.unwrap().len(), 1);
}

#[tokio::test]
async fn direct_bit_response_accepts_only_documented_tokens() {
    let (port, _) = start_scripted_server(|_| "ON".to_owned()).await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    assert_eq!(client.read("R0", None).await.unwrap(), vec!["ON"]);

    let (invalid_port, _) = start_scripted_server(|_| "GARBAGE".to_owned()).await;
    let mut invalid_options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    invalid_options.port = invalid_port;
    let invalid_client = HostLinkClient::connect(invalid_options).await.unwrap();
    let error = invalid_client.read("R0", None).await.unwrap_err();
    assert!(error.to_string().contains("Invalid response token"));
    assert!(!invalid_client.is_open().await);
}

#[tokio::test]
async fn direct_bit_response_rejects_case_folding_and_surrounding_whitespace() {
    for response in ["on", "TRUE", " ON", "ON "] {
        let (port, _) = start_scripted_server(move |_| response.to_owned()).await;
        let mut options = HostLinkConnectionOptions::new(
            "127.0.0.1",
            8501,
            HostLinkTransportMode::Tcp,
            "keyence:kv-8000",
        )
        .unwrap();
        options.port = port;
        let client = HostLinkClient::connect(options).await.unwrap();

        assert!(
            client.read("R0", None).await.is_err(),
            "response={response:?}"
        );
        assert!(!client.is_open().await, "response={response:?}");
    }
}

#[tokio::test]
async fn monitor_reads_require_registration_and_exact_registered_count() {
    let unconnected = HostLinkClient::new(
        HostLinkConnectionOptions::new(
            "127.0.0.1",
            8501,
            HostLinkTransportMode::Tcp,
            "keyence:kv-8000",
        )
        .unwrap(),
    );
    assert!(
        unconnected
            .read_monitor_bits()
            .await
            .unwrap_err()
            .to_string()
            .contains("registered")
    );
    assert!(
        unconnected
            .read_monitor_words()
            .await
            .unwrap_err()
            .to_string()
            .contains("registered")
    );

    let (port, _) = start_scripted_server(|command| match command.as_str() {
        "MBS R000 R001" => "OK".to_owned(),
        "MBR" => "1".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();
    client.register_monitor_bits(&["R0", "R1"]).await.unwrap();
    let error = client.read_monitor_bits().await.unwrap_err();
    assert!(error.to_string().contains("expected 2"));
    assert!(!client.is_open().await);
}

#[tokio::test]
async fn unconnected_commands_return_typed_not_connected_without_opening_transport() {
    let client = HostLinkClient::new(
        HostLinkConnectionOptions::new(
            "203.0.113.1",
            8501,
            HostLinkTransportMode::Tcp,
            "keyence:kv-8000",
        )
        .unwrap(),
    );

    assert!(matches!(
        client.send_raw("RD DM0.U").await,
        Err(HostLinkError::NotConnected)
    ));
    assert!(matches!(
        client.read("DM0", Some("U")).await,
        Err(HostLinkError::NotConnected)
    ));
    assert!(matches!(
        client.write("DM0", 1_u16, Some("U")).await,
        Err(HostLinkError::NotConnected)
    ));
    assert!(matches!(
        client.read("DM100", None).await,
        Err(HostLinkError::Protocol(_))
    ));
    assert!(matches!(
        client.read("DM100.D", Some("D")).await,
        Err(HostLinkError::Protocol(_))
    ));
    assert!(matches!(
        client.read("R100", None).await,
        Err(HostLinkError::NotConnected)
    ));
    assert!(!client.is_open().await);
    assert_eq!(client.traffic_stats().await, Default::default());
}

#[tokio::test]
async fn raw_exchange_preserves_plc_error_and_non_ascii_response_bytes() {
    let (port, received) = start_scripted_server_bytes(|command| match command.as_str() {
        "ERROR" => b"E1".to_vec(),
        "BYTES" => vec![0x82, 0xA0, 0x09],
        _ => Vec::new(),
    })
    .await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();
    assert_eq!(client.traffic_stats().await, Default::default());

    assert_eq!(client.send_raw("ERROR").await.unwrap(), b"E1");
    assert_eq!(
        client.send_raw("BYTES").await.unwrap(),
        vec![0x82, 0xA0, 0x09]
    );
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["ERROR", "BYTES"]
    );
    let stats = client.traffic_stats().await;
    assert_eq!(stats.request_count, 2);
    assert_eq!(stats.tx_bytes, 12);
    assert_eq!(stats.rx_bytes, 7);
}

#[tokio::test]
async fn tcp_traffic_stats_are_independent_of_crlf_segmentation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut first = [0_u8; 6];
        stream.read_exact(&mut first).await.unwrap();
        assert_eq!(&first, b"FIRST\r");
        stream.write_all(b"FIRST\r").await.unwrap();

        let mut second = [0_u8; 7];
        stream.read_exact(&mut second).await.unwrap();
        assert_eq!(&second, b"SECOND\r");
        stream.write_all(b"\nSECOND\n\r").await.unwrap();
    });
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    assert_eq!(client.send_raw("FIRST").await.unwrap(), b"FIRST");
    assert_eq!(client.send_raw("SECOND").await.unwrap(), b"SECOND");
    assert_eq!(client.traffic_stats().await.rx_bytes, 13);
    server.await.unwrap();
}

#[tokio::test]
async fn tcp_absolute_response_cap_accepts_maximum_and_rejects_one_byte_over() {
    let (port, _) = start_scripted_server_bytes(|command| match command.as_str() {
        "MAX" => vec![b'7'; 65_536],
        "OVER" => vec![b'8'; 65_537],
        _ => Vec::new(),
    })
    .await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    assert_eq!(client.send_raw("MAX").await.unwrap().len(), 65_536);
    assert!(client.send_raw("OVER").await.is_err());
    assert!(!client.is_open().await);
}

#[tokio::test]
async fn dword_helpers_use_one_native_dword_request_and_enforce_the_limit() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDS DM100.D 2" => "1 4294967295".to_owned(),
        "WRS DM200.D 2 1 4294967295" => "OK".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    assert_eq!(
        read_dwords(&client, "DM100", 2).await.unwrap(),
        vec![1, u32::MAX]
    );
    write_dwords_single_request(&client, "DM200", &[1, u32::MAX])
        .await
        .unwrap();
    assert!(read_dwords(&client, "DM0", 501).await.is_err());
    assert!(read_words(&client, "DM0", 1001).await.is_err());
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDS DM100.D 2", "WRS DM200.D 2 1 4294967295"]
    );
}

#[tokio::test]
async fn unexpected_numeric_response_count_invalidates_transport() {
    let (port, _) = start_scripted_server(|command| match command.as_str() {
        "RDS DM100.U 2" => "1 2 3".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let error = client
        .read_consecutive("DM100", 2, Some("U"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("expected 2"));
    assert!(!client.is_open().await);
}

#[tokio::test]
async fn tcp_eof_before_terminator_rejects_partial_response_and_closes_transport() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 5];
        stream.read_exact(&mut request).await.unwrap();
        stream.write_all(b"PARTIAL").await.unwrap();
    });

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    options.timeout = Duration::from_secs(2);
    let client = HostLinkClient::connect(options).await.unwrap();

    let error = client.send_raw("READ").await.unwrap_err();
    assert!(error.to_string().contains("before the response terminator"));
    assert!(!client.is_open().await);
    assert_eq!(client.traffic_stats().await.rx_bytes, 0);
    server.await.unwrap();
}

#[tokio::test]
async fn tcp_oversize_partial_response_does_not_increment_receive_stats() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 5];
        stream.read_exact(&mut request).await.unwrap();
        stream.write_all(&vec![b'A'; 65_537]).await.unwrap();
    });

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    options.timeout = Duration::from_secs(2);
    let client = HostLinkClient::connect(options).await.unwrap();

    let error = client.send_raw("READ").await.unwrap_err();
    assert!(error.to_string().contains("exceeds"));
    assert_eq!(client.traffic_stats().await.rx_bytes, 0);
    assert!(!client.is_open().await);
    server.await.unwrap();
}

#[tokio::test]
async fn complete_plc_error_line_is_counted_before_semantic_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 3];
        stream.read_exact(&mut request).await.unwrap();
        stream.write_all(b"E1\r").await.unwrap();
    });

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    assert!(client.clear_error().await.is_err());
    assert_eq!(client.traffic_stats().await.rx_bytes, 3);
    server.await.unwrap();
}

#[tokio::test]
async fn udp_timeout_discards_delayed_response_before_next_request() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let mut request = vec![0u8; 1024];
        let (_, first_peer) = socket.recv_from(&mut request).await.unwrap();
        tokio::time::sleep(Duration::from_millis(150)).await;
        socket.send_to(b"FIRST\r", first_peer).await.unwrap();

        let (_, second_peer) = socket.recv_from(&mut request).await.unwrap();
        socket.send_to(b"SECOND\r", second_peer).await.unwrap();
    });

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.transport = HostLinkTransportMode::Udp;
    options.port = port;
    options.timeout = Duration::from_millis(50);
    let client = HostLinkClient::connect(options).await.unwrap();

    assert!(client.send_raw("FIRST").await.is_err());
    assert!(!client.is_open().await);
    client.set_timeout(Duration::from_secs(2)).await.unwrap();
    assert!(matches!(
        client.send_raw("SECOND").await,
        Err(HostLinkError::NotConnected)
    ));
    client.open().await.unwrap();
    assert_eq!(client.send_raw("SECOND").await.unwrap(), b"SECOND");
    server.await.unwrap();
}

#[tokio::test]
async fn dropped_udp_request_poisons_transport_and_discards_delayed_response() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let (first_seen_tx, first_seen_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut request = vec![0u8; 1024];
        let (_, first_peer) = socket.recv_from(&mut request).await.unwrap();
        first_seen_tx.send(()).unwrap();
        release_rx.await.unwrap();
        let (_, second_peer) = socket.recv_from(&mut request).await.unwrap();
        socket.send_to(b"FIRST\r", first_peer).await.unwrap();
        socket.send_to(b"SECOND\r", second_peer).await.unwrap();
    });

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.transport = HostLinkTransportMode::Udp;
    options.port = port;
    options.timeout = Duration::from_secs(2);
    let client = HostLinkClient::connect(options).await.unwrap();

    let request_client = client.clone();
    let request = tokio::spawn(async move { request_client.send_raw("FIRST").await });
    first_seen_rx.await.unwrap();
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    assert!(!client.is_open().await);

    release_tx.send(()).unwrap();
    assert!(matches!(
        client.send_raw("SECOND").await,
        Err(HostLinkError::NotConnected)
    ));
    client.open().await.unwrap();
    assert_eq!(client.send_raw("SECOND").await.unwrap(), b"SECOND");
    server.await.unwrap();
}

#[tokio::test]
async fn read_typed_and_write_typed_support_float_suffix() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDS DM200.U 2" => "0 16712".to_owned(),
        "WRS DM200.U 2 0 16712" => "OK".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
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
async fn float_write_to_every_direct_bit_family_is_rejected_before_transport() {
    let options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    let client = HostLinkClient::new(options);

    for device in [
        "Y0", "R0", "B0", "MR0", "LR0", "CR0", "VB0", "X0", "M0", "L0",
    ] {
        let error = client.write_typed(device, "F", 1.0_f32).await.unwrap_err();
        assert!(
            matches!(error, HostLinkError::Protocol(_)),
            "device={device} error={error}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn special_family_float32_rejects_before_fifo_and_transport() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let first_seen = Arc::new(Notify::new());
    let release_first = Arc::new(Notify::new());
    let server_seen = Arc::clone(&first_seen);
    let server_release = Arc::clone(&release_first);
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        assert_eq!(read_test_command(&mut stream).await, "?K");
        server_seen.notify_one();
        server_release.notified().await;
        stream.write_all(b"01\r").await.unwrap();
    });

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let active_client = client.clone();
    let active = tokio::spawn(async move { active_client.query_model().await });
    first_seen.notified().await;

    for device in ["R0", "T0", "C0", "AT0", "Z0"] {
        let read_error =
            tokio::time::timeout(Duration::from_millis(50), client.read_typed(device, "F"))
                .await
                .expect("invalid Float32 read queued behind the active FIFO turn")
                .unwrap_err();
        assert!(matches!(read_error, HostLinkError::Protocol(_)));

        let write_error = tokio::time::timeout(
            Duration::from_millis(50),
            client.write_typed(device, "F", 1.0_f32),
        )
        .await
        .expect("invalid Float32 write queued behind the active FIFO turn")
        .unwrap_err();
        assert!(matches!(write_error, HostLinkError::Protocol(_)));

        let address = format!("{device}:F");
        let addresses = [address.as_str()];
        let named_error =
            tokio::time::timeout(Duration::from_millis(50), client.read_named(&addresses))
                .await
                .expect("invalid named Float32 read queued behind the active FIFO turn")
                .unwrap_err();
        assert!(matches!(named_error, HostLinkError::Protocol(_)));

        let mut stream = Box::pin(client.poll(&addresses, Duration::from_millis(1)));
        let poll_error = tokio::time::timeout(Duration::from_millis(50), stream.next())
            .await
            .expect("invalid Float32 poll queued behind the active FIFO turn")
            .expect("poll ended without its validation error")
            .unwrap_err();
        assert!(matches!(poll_error, HostLinkError::Protocol(_)));
    }

    release_first.notify_one();
    active.await.unwrap().unwrap();
    assert_eq!(client.traffic_stats().await.request_count, 1);
}

#[test]
fn float32_parser_normalizer_and_hand_built_formatter_share_family_validation() {
    assert_eq!(
        HostLinkAddress::normalize_logical("dm0:f").unwrap(),
        "DM0:F"
    );
    for device in ["R0", "T0", "C0", "AT0", "Z0"] {
        let text = format!("{device}:F");
        assert!(HostLinkAddress::parse_logical(&text).is_err());
        assert!(HostLinkAddress::normalize_logical(&text).is_err());
        assert!(HostLinkAddress::try_parse_logical(&text).is_none());

        let hand_built = KvLogicalAddress {
            base_address: KvDeviceAddress {
                device_type: device.trim_end_matches('0').to_owned(),
                number: 0,
                suffix: String::new(),
            },
            data_type: "F".to_owned(),
            bit_index: None,
        };
        assert!(hand_built.to_text().is_err());

        let low_level = KvDeviceAddress {
            device_type: device.trim_end_matches('0').to_owned(),
            number: 0,
            suffix: ".F".to_owned(),
        };
        assert!(HostLinkAddress::format(&low_level).is_err());
    }

    for text in ["Z1:F", "z1:f", "Z1:.F"] {
        assert!(HostLinkAddress::parse_logical(text).is_err(), "{text}");
        assert!(HostLinkAddress::normalize_logical(text).is_err(), "{text}");
        assert!(HostLinkAddress::try_parse_logical(text).is_none(), "{text}");
    }
}

#[tokio::test]
async fn read_typed_write_typed_and_read_named_support_hex_suffix() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RD DM210.H" => "00ff".to_owned(),
        "WR DM210.H FF" => "OK".to_owned(),
        "WR DM211.H AA" => "OK".to_owned(),
        "RD DM212.H" => "ABCD".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let value = read_typed(&client, "DM210", "H").await.unwrap();
    client.write_typed("DM210", "H", 0x00FF_u16).await.unwrap();
    client.write_typed("DM211", "H", "00AA").await.unwrap();
    let named = client.read_named(&["DM212:H"]).await.unwrap();

    assert_eq!(value, HostLinkValue::Text("00FF".to_owned()));
    assert_eq!(named["DM212:H"], HostLinkValue::Text("ABCD".to_owned()));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RD DM210.H", "WR DM210.H FF", "WR DM211.H AA", "RD DM212.H"]
    );
}

#[tokio::test]
async fn read_typed_timer_counter_composite_read_returns_set_value() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RD T0.D" => "0,0000000010,0000000020".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let value = read_typed(&client, "T0", "D").await.unwrap();

    assert_eq!(value, HostLinkValue::U32(20));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RD T0.D"]
    );
}

#[tokio::test]
async fn read_typed_timer_counter_16bit_composite_read_returns_set_value() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RD T0.U" => "0,00010,00020".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let value = read_typed(&client, "T0", "U").await.unwrap();

    assert_eq!(value, HostLinkValue::U16(20));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RD T0.U"]
    );
}

#[tokio::test]
async fn read_named_timer_counter_composite_read_returns_set_value() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RD T10.D" => "0,0000000010,0000000020".to_owned(),
        "RD C10.D" => "0,0000000000,0000000030".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let result = client.read_named(&["T10:D", "C10:D"]).await.unwrap();

    assert_eq!(result["T10:D"], HostLinkValue::U32(20));
    assert_eq!(result["C10:D"], HostLinkValue::U32(30));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RD T10.D", "RD C10.D"]
    );
}

#[tokio::test]
async fn read_named_native_32bit_z_uses_native_dword_read() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RD Z1.D" => "0000070000".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let result = client.read_named(&["Z1:D"]).await.unwrap();

    assert_eq!(result["Z1:D"], HostLinkValue::U32(70_000));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RD Z1.D"]
    );
}

#[tokio::test]
async fn read_timer_counter_returns_status_current_and_preset() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RD T10.D" => "1,0000000010,0000000020".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let result = client.read_timer_counter("T10").await.unwrap();

    assert_eq!(result.status, 1);
    assert_eq!(result.current, 10);
    assert_eq!(result.preset, 20);
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RD T10.D"]
    );
}

#[tokio::test]
async fn timer_counter_composites_validate_status_current_and_preset() {
    for (dtype, response) in [
        ("U", "2,00010,00020"),
        ("S", "0,invalid,20"),
        ("D", "0,10,invalid"),
        ("L", "0,invalid,20"),
        ("H", "0,00FF,10000"),
    ] {
        let (port, _) = start_scripted_server(move |_| response.to_owned()).await;
        let mut options = HostLinkConnectionOptions::new(
            "127.0.0.1",
            8501,
            HostLinkTransportMode::Tcp,
            "keyence:kv-8000",
        )
        .unwrap();
        options.port = port;
        let client = HostLinkClient::connect(options).await.unwrap();

        assert!(read_typed(&client, "T0", dtype).await.is_err());
        assert!(!client.is_open().await);
    }
}

#[tokio::test]
async fn timer_counter_composites_accept_exact_type_boundaries() {
    for (dtype, response, expected) in [
        ("U", "1,0,65535", HostLinkValue::U16(u16::MAX)),
        ("S", "0,-32768,32767", HostLinkValue::I16(i16::MAX)),
        ("D", "1,0,4294967295", HostLinkValue::U32(u32::MAX)),
        (
            "L",
            "0,-2147483648,2147483647",
            HostLinkValue::I32(i32::MAX),
        ),
        ("H", "1,0000,ffff", HostLinkValue::Text("FFFF".to_owned())),
    ] {
        let (port, _) = start_scripted_server(move |_| response.to_owned()).await;
        let mut options = HostLinkConnectionOptions::new(
            "127.0.0.1",
            8501,
            HostLinkTransportMode::Tcp,
            "keyence:kv-8000",
        )
        .unwrap();
        options.port = port;
        let client = HostLinkClient::connect(options).await.unwrap();

        assert_eq!(read_typed(&client, "T0", dtype).await.unwrap(), expected);
    }
}

#[tokio::test]
async fn timer_counter_composites_reject_overflow_missing_and_extra_fields() {
    for (dtype, response) in [
        ("U", "0,65536,1"),
        ("U", "0,1,65536"),
        ("S", "0,-32769,1"),
        ("S", "0,1,32768"),
        ("D", "0,4294967296,1"),
        ("D", "0,1,4294967296"),
        ("L", "0,-2147483649,1"),
        ("L", "0,1,2147483648"),
        ("H", "0,10000,1"),
        ("H", "0,1,10000"),
        ("U", "0,1"),
        ("U", "0,1,2,3"),
    ] {
        let (port, _) = start_scripted_server(move |_| response.to_owned()).await;
        let mut options = HostLinkConnectionOptions::new(
            "127.0.0.1",
            8501,
            HostLinkTransportMode::Tcp,
            "keyence:kv-8000",
        )
        .unwrap();
        options.port = port;
        let client = HostLinkClient::connect(options).await.unwrap();

        assert!(
            read_typed(&client, "T0", dtype).await.is_err(),
            "dtype={dtype} response={response}"
        );
        assert!(!client.is_open().await);
    }
}

#[tokio::test]
async fn timer_counter_helper_rejects_malformed_composite_and_closes_transport() {
    let (port, _) = start_scripted_server(|_| "garbage,garbage,123".to_owned()).await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    assert!(client.read_timer_counter("T0").await.is_err());
    assert!(!client.is_open().await);
}

#[tokio::test]
async fn read_named_direct_bits_use_unsuffixed_rd_commands() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDS R000 1" => "1".to_owned(),
        "RDS CR000 1" => "0".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let result = client.read_named(&["R0:BIT", "CR0:BIT"]).await.unwrap();
    assert_eq!(result["R0:BIT"], HostLinkValue::Bool(true));
    assert_eq!(result["CR0:BIT"], HostLinkValue::Bool(false));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDS R000 1", "RDS CR000 1"]
    );
}

#[tokio::test]
async fn read_named_batches_xym_direct_bits_with_hostlink_notation() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDS X100 2" => "1 0".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let result = client.read_named(&["X100:BIT", "X101:BIT"]).await.unwrap();
    assert_eq!(result["X100:BIT"], HostLinkValue::Bool(true));
    assert_eq!(result["X101:BIT"], HostLinkValue::Bool(false));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDS X100 2"]
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

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let result = client
        .read_named(&[
            "R0:BIT", "R1:BIT", "R2:BIT", "R3:BIT", "CR0:BIT", "CR1:BIT", "CR2:BIT", "CR3:BIT",
        ])
        .await
        .unwrap();

    assert_eq!(result["R0:BIT"], HostLinkValue::Bool(true));
    assert_eq!(result["R1:BIT"], HostLinkValue::Bool(false));
    assert_eq!(result["R2:BIT"], HostLinkValue::Bool(true));
    assert_eq!(result["R3:BIT"], HostLinkValue::Bool(false));
    assert_eq!(result["CR0:BIT"], HostLinkValue::Bool(false));
    assert_eq!(result["CR1:BIT"], HostLinkValue::Bool(true));
    assert_eq!(result["CR2:BIT"], HostLinkValue::Bool(false));
    assert_eq!(result["CR3:BIT"], HostLinkValue::Bool(true));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDS R000 4", "RDS CR000 4"]
    );
}

#[tokio::test]
async fn read_named_batches_bit_bank_direct_bits_across_display_bank_boundary() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDS CR3614 4" => "0 1 0 1".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let result = client
        .read_named(&["CR3614:BIT", "CR3615:BIT", "CR3700:BIT", "CR3701:BIT"])
        .await
        .unwrap();

    assert_eq!(result["CR3614:BIT"], HostLinkValue::Bool(false));
    assert_eq!(result["CR3615:BIT"], HostLinkValue::Bool(true));
    assert_eq!(result["CR3700:BIT"], HostLinkValue::Bool(false));
    assert_eq!(result["CR3701:BIT"], HostLinkValue::Bool(true));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDS CR3614 4"]
    );
}

#[tokio::test]
async fn read_typed_empty_dtype_is_rejected() {
    let (port, received) = start_scripted_server(|_| "E1".to_owned()).await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    assert!(read_typed(&client, "CR0", "").await.is_err());
    assert!(read_typed(&client, "DM200", "").await.is_err());
    assert!(received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn read_comments_helper_and_named_read_support_comment_values() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDC DM150" => "MAIN COMMENT                    ".to_owned(),
        "RD DM100.U" => "321".to_owned(),
        "RDC DM101" => "ALARM COMMENT                   ".to_owned(),
        "RDC DM102" => "POLL COMMENT                    ".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let comment = read_comments(&client, "DM150", HostLinkCommentEncoding::Utf8)
        .await
        .unwrap();
    assert_eq!(comment, "MAIN COMMENT");

    let error = client
        .read_named(&["DM100:U", "DM101:COMMENT"])
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("explicit HostLinkCommentEncoding")
    );
    assert_eq!(client.traffic_stats().await.request_count, 1);

    let result = client
        .read_named_with_comment_encoding(
            &["DM100:U", "DM101:COMMENT"],
            HostLinkCommentEncoding::Utf8,
        )
        .await
        .unwrap();
    assert_eq!(result["DM100:U"], HostLinkValue::U16(321));
    assert_eq!(
        result["DM101:COMMENT"],
        HostLinkValue::Text("ALARM COMMENT".to_owned())
    );

    let implicit_poll = client.poll(&["DM102:COMMENT"], Duration::from_millis(1));
    pin_mut!(implicit_poll);
    let error = implicit_poll.next().await.unwrap().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("explicit HostLinkCommentEncoding")
    );

    let explicit_poll = client.poll_with_comment_encoding(
        &["DM102:COMMENT"],
        Duration::from_millis(1),
        HostLinkCommentEncoding::Utf8,
    );
    pin_mut!(explicit_poll);
    let polled = explicit_poll.next().await.unwrap().unwrap();
    assert_eq!(
        polled["DM102:COMMENT"],
        HostLinkValue::Text("POLL COMMENT".to_owned())
    );
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDC DM150", "RD DM100.U", "RDC DM101", "RDC DM102"]
    );
}

#[tokio::test]
async fn read_comments_decodes_cp932_payloads_only_when_selected() {
    let (port, received) = start_scripted_server_bytes(|command| match command.as_str() {
        "RDC DM20" => {
            let expected = "\u{904b}\u{8ee2}\u{8a31}\u{53ef}";
            let (encoded, _, _) = SHIFT_JIS.encode(expected);
            let mut bytes = encoded.into_owned();
            bytes.extend_from_slice(b"                    ");
            bytes
        }
        _ => b"E1".to_vec(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let comment = read_comments(&client, "DM20", HostLinkCommentEncoding::Cp932)
        .await
        .unwrap();

    assert_eq!(comment, "\u{904b}\u{8ee2}\u{8a31}\u{53ef}");
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDC DM20"]
    );
}

#[tokio::test]
async fn read_comments_remove_only_trailing_ascii_space_padding() {
    let (port, received) = start_scripted_server_bytes(|command| match command.as_str() {
        "RDC DM20" => "A B   ".as_bytes().to_vec(),
        "RDC DM21" => "TEXT \t".as_bytes().to_vec(),
        "RDC DM22" => "FULLWIDTH\u{3000}".as_bytes().to_vec(),
        "RDC DM23" => b"    ".to_vec(),
        _ => b"E1".to_vec(),
    })
    .await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    assert_eq!(
        client
            .read_comments("DM20", HostLinkCommentEncoding::Utf8)
            .await
            .unwrap(),
        "A B"
    );
    assert_eq!(
        client
            .read_comments("DM21", HostLinkCommentEncoding::Utf8)
            .await
            .unwrap(),
        "TEXT \t"
    );
    assert_eq!(
        client
            .read_comments("DM22", HostLinkCommentEncoding::Utf8)
            .await
            .unwrap(),
        "FULLWIDTH\u{3000}"
    );
    assert_eq!(
        client
            .read_comments("DM23", HostLinkCommentEncoding::Utf8)
            .await
            .unwrap(),
        ""
    );
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDC DM20", "RDC DM21", "RDC DM22", "RDC DM23"]
    );
}

#[tokio::test]
async fn read_comment_bytes_preserves_padding_and_text_decode_is_strict() {
    let (raw_port, raw_received) = start_scripted_server_bytes(|command| match command.as_str() {
        "RDC DM20" => b"\x82\xA0   ".to_vec(),
        _ => b"E1".to_vec(),
    })
    .await;
    let mut raw_options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    raw_options.port = raw_port;
    let raw_client = HostLinkClient::connect(raw_options).await.unwrap();
    assert_eq!(
        read_comment_bytes(&raw_client, "DM20").await.unwrap(),
        b"\x82\xA0   "
    );
    assert_eq!(
        raw_received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDC DM20"]
    );

    let (invalid_port, invalid_received) =
        start_scripted_server_bytes(|_| b"\x81\x30".to_vec()).await;
    let mut invalid_options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    invalid_options.port = invalid_port;
    let invalid_client = HostLinkClient::connect(invalid_options).await.unwrap();
    let error = invalid_client
        .read_comments("DM21", HostLinkCommentEncoding::Cp932)
        .await
        .unwrap_err();
    assert!(matches!(error, HostLinkError::Protocol(_)));
    assert!(error.to_string().contains("no fallback or replacement"));
    assert!(!invalid_client.is_open().await);
    assert_eq!(
        invalid_received
            .lock()
            .unwrap()
            .drain(..)
            .collect::<Vec<_>>(),
        vec!["RDC DM21"]
    );
}

#[tokio::test]
async fn plc_comment_errors_keep_connection_for_raw_and_text_reads() {
    let (raw_port, raw_received) = start_scripted_server_bytes(|command| match command.as_str() {
        "RDC DM22" => b"E1".to_vec(),
        "RDC DM23" => b"RAW OK  ".to_vec(),
        _ => b"E2".to_vec(),
    })
    .await;
    let mut raw_options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    raw_options.port = raw_port;
    let raw_client = HostLinkClient::connect(raw_options).await.unwrap();
    let error = raw_client.read_comment_bytes("DM22").await.unwrap_err();
    assert!(matches!(
        error,
        HostLinkError::Plc { ref code, .. } if code == "E1"
    ));
    assert!(raw_client.is_open().await);
    assert_eq!(
        raw_client.read_comment_bytes("DM23").await.unwrap(),
        b"RAW OK  "
    );
    assert_eq!(
        raw_received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDC DM22", "RDC DM23"]
    );

    let (text_port, text_received) =
        start_scripted_server_bytes(|command| match command.as_str() {
            "RDC DM24" => b"E1".to_vec(),
            "RDC DM25" => b"TEXT OK  ".to_vec(),
            _ => b"E2".to_vec(),
        })
        .await;
    let mut text_options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    text_options.port = text_port;
    let text_client = HostLinkClient::connect(text_options).await.unwrap();
    let error = text_client
        .read_comments("DM24", HostLinkCommentEncoding::Utf8)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        HostLinkError::Plc { ref code, .. } if code == "E1"
    ));
    assert!(text_client.is_open().await);
    assert_eq!(
        text_client
            .read_comments("DM25", HostLinkCommentEncoding::Utf8)
            .await
            .unwrap(),
        "TEXT OK"
    );
    assert_eq!(
        text_received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDC DM24", "RDC DM25"]
    );
}

#[tokio::test]
async fn at_write_is_rejected_before_opening_connection() {
    let client = HostLinkClient::new(
        HostLinkConnectionOptions::new(
            "127.0.0.1",
            8501,
            HostLinkTransportMode::Tcp,
            "keyence:kv-8000",
        )
        .unwrap(),
    );

    assert!(client.write("AT0", 3533, Some("D")).await.is_err());
    assert!(
        client
            .write_consecutive("AT0", &[3533, 5543], Some("D"))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn open_and_connect_returns_fifo_client_that_uses_helper_api() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RD DM10.U" => "123".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
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
async fn ordinary_client_supports_read_comments() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RDC DM10" => "ALARM TEXT                      ".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = open_and_connect(options).await.unwrap();
    let comment = client
        .read_comments("DM10", HostLinkCommentEncoding::Utf8)
        .await
        .unwrap();

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

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();
    let data_memory_comment = client
        .read_comments("D10", HostLinkCommentEncoding::Utf8)
        .await
        .unwrap();
    let auxiliary_relay_comment = client
        .read_comments("M20", HostLinkCommentEncoding::Utf8)
        .await
        .unwrap();

    assert_eq!(data_memory_comment, "DM COMMENT");
    assert_eq!(auxiliary_relay_comment, "MR COMMENT");
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDC D10", "RDC M20"]
    );
}

#[tokio::test]
async fn command_device_sets_follow_manual_and_xym_aliases() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "ST X100" => "OK".to_owned(),
        "RS M100" => "OK".to_owned(),
        "STS L100 4" => "OK".to_owned(),
        "MWS D100.U E100.U F100.U MR100 LR100" => "OK".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    client.forced_set("X100").await.unwrap();
    client.forced_reset("M100").await.unwrap();
    client.forced_set_consecutive("L100", 4).await.unwrap();
    client
        .register_monitor_words(&[
            HostLinkMonitorWord::numeric("D100", "U"),
            HostLinkMonitorWord::numeric("E100", "U"),
            HostLinkMonitorWord::numeric("F100", "U"),
            HostLinkMonitorWord::direct_bit("MR100"),
            HostLinkMonitorWord::direct_bit("LR100"),
        ])
        .await
        .unwrap();
    assert!(
        client
            .register_monitor_words(&[HostLinkMonitorWord::direct_bit("M100")])
            .await
            .is_err()
    );
    assert!(
        client
            .register_monitor_words(&[HostLinkMonitorWord::direct_bit("L100")])
            .await
            .is_err()
    );
    assert!(client.forced_set_consecutive("T100", 4).await.is_err());

    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec![
            "ST X100",
            "RS M100",
            "STS L100 4",
            "MWS D100.U E100.U F100.U MR100 LR100"
        ]
    );
}

#[tokio::test]
async fn wss_timer_counter_count_limit_is_enforced_before_send() {
    let (port, received) = start_scripted_server(|_| "OK".to_owned()).await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();
    let values = vec![0_i32; 121];

    assert!(
        client
            .write_set_value_consecutive("T0", &values, None)
            .await
            .is_err()
    );
    assert!(received.lock().unwrap().is_empty());
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

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let stream = plc_comm_kv_hostlink::poll(
        &client,
        &["DM100:U", "DM100.0", "DM101:F"],
        std::time::Duration::from_millis(1),
    );
    pin_mut!(stream);
    let first = stream.next().await.unwrap().unwrap();
    let second = stream.next().await.unwrap().unwrap();

    assert_eq!(first["DM100:U"], HostLinkValue::U16(1));
    assert_eq!(first["DM100.0"], HostLinkValue::Bool(true));
    assert_eq!(first["DM101:F"], HostLinkValue::F32(1.5));
    assert_eq!(second["DM100:U"], HostLinkValue::U16(3));
    assert_eq!(second["DM100.0"], HostLinkValue::Bool(true));
    assert_eq!(second["DM101:F"], HostLinkValue::F32(2.5));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RDS DM100.U 3", "RDS DM100.U 3"]
    );
}

#[tokio::test]
async fn empty_named_reads_and_invalid_polling_are_rejected_without_transport() {
    let options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    let client = HostLinkClient::new(options.clone());
    assert!(client.read_named::<&str>(&[]).await.is_err());

    let empty: [&str; 0] = [];
    let stream = plc_comm_kv_hostlink::poll(&client, &empty, Duration::from_millis(1));
    pin_mut!(stream);
    assert!(stream.next().await.unwrap().is_err());

    let stream = client.poll(&["DM0:U"], Duration::ZERO);
    pin_mut!(stream);
    assert!(stream.next().await.unwrap().is_err());

    let (port, received) = start_scripted_server(|_| "1".to_owned()).await;
    let mut connected_options = options;
    connected_options.port = port;
    let connected = HostLinkClient::connect(connected_options).await.unwrap();

    let error = connected
        .read_named_with_comment_encoding(&["DM0:U"], HostLinkCommentEncoding::Utf8)
        .await
        .unwrap_err();
    assert!(matches!(error, HostLinkError::Protocol(_)));
    assert!(error.to_string().contains("contains no COMMENT entry"));

    let stream = connected.poll_with_comment_encoding(
        &["DM0:U"],
        Duration::from_millis(1),
        HostLinkCommentEncoding::Utf8,
    );
    pin_mut!(stream);
    let error = stream.next().await.unwrap().unwrap_err();
    assert!(matches!(error, HostLinkError::Protocol(_)));
    assert!(error.to_string().contains("contains no COMMENT entry"));
    assert_eq!(connected.traffic_stats().await.request_count, 0);
    assert!(received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn ipv6_literals_are_rejected_for_tcp_and_udp_before_transport() {
    for transport in [HostLinkTransportMode::Tcp, HostLinkTransportMode::Udp] {
        let options =
            HostLinkConnectionOptions::new("::1", 8501, transport, "keyence:kv-8000").unwrap();
        let client = HostLinkClient::new(options);

        let error = client.open().await.unwrap_err();
        assert!(matches!(error, HostLinkError::Protocol(_)));
        assert!(!client.is_open().await);
    }
}

#[tokio::test]
async fn read_sends_32_bit_device_at_catalog_upper_boundary() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "RD DM65534.D" => "1".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();
    let value = read_typed(&client, "DM65534", "D").await.unwrap();

    assert_eq!(value, HostLinkValue::U32(1));
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["RD DM65534.D"]
    );
}

#[tokio::test]
async fn expansion_unit_buffer_uses_address_suffix_command_form() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "URD 01 100.U 2" => "123 456".to_owned(),
        "UWR 02 200.S 2 7 8" => "OK".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    let values = client
        .read_expansion_unit_buffer(1, 100, 2, "U")
        .await
        .unwrap();
    client
        .write_expansion_unit_buffer(2, 200, &[7_i16, 8_i16], "S")
        .await
        .unwrap();

    assert_eq!(values, vec!["123".to_owned(), "456".to_owned()]);
    assert_eq!(
        received.lock().unwrap().drain(..).collect::<Vec<_>>(),
        vec!["URD 01 100.U 2", "UWR 02 200.S 2 7 8"]
    );
}

struct CustomPayloadValue {
    accepted: bool,
}

struct EmptyPayloadValue;

impl HostLinkPayloadValue for EmptyPayloadValue {
    fn format_for_suffix(&self, _suffix: &str) -> Result<String, HostLinkError> {
        Ok(String::new())
    }
}

impl HostLinkPayloadValue for CustomPayloadValue {
    fn format_for_suffix(&self, suffix: &str) -> Result<String, HostLinkError> {
        if self.accepted && suffix == ".U" {
            Ok("77".to_owned())
        } else {
            Err(HostLinkError::protocol(format!(
                "custom value does not support suffix '{suffix}'"
            )))
        }
    }
}

#[tokio::test]
async fn fallible_custom_formatter_propagates_through_normal_write_without_transport_fallback() {
    let (port, received) = start_scripted_server(|command| match command.as_str() {
        "WR DM0.U 77" => "OK".to_owned(),
        _ => "E1".to_owned(),
    })
    .await;
    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();

    assert_eq!(
        CustomPayloadValue { accepted: true }
            .format_for_suffix(".U")
            .unwrap(),
        "77"
    );
    let mut output = String::from("unchanged");
    let error = CustomPayloadValue { accepted: false }
        .append_to_payload(".U", &mut output)
        .unwrap_err();
    assert!(error.to_string().contains("custom value"));
    assert_eq!(output, "unchanged");

    client
        .write("DM0", CustomPayloadValue { accepted: true }, Some("U"))
        .await
        .unwrap();
    let before = client.traffic_stats().await;
    let error = client
        .write("DM1", CustomPayloadValue { accepted: false }, Some("U"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("custom value"));
    assert_eq!(client.traffic_stats().await, before);
    let error = client
        .write("DM2", EmptyPayloadValue, Some("U"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("empty token"));
    assert_eq!(client.traffic_stats().await, before);
    assert_eq!(received.lock().unwrap().as_slice(), ["WR DM0.U 77"]);
}

#[tokio::test]
async fn invalid_builtin_formatting_fails_all_normal_write_shapes_before_transport() {
    let options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    let client = HostLinkClient::new(options);

    assert!(client.write("DM0", 65_536_u32, Some("U")).await.is_err());
    assert!(
        client
            .write_consecutive("DM0", &[0_i32, 65_536_i32], Some("U"))
            .await
            .is_err()
    );
    assert!(
        client
            .write_set_value("T0", 32_768_i32, Some("S"))
            .await
            .is_err()
    );
    assert!(
        client
            .write_set_value_consecutive("T0", &[0_i64, 4_294_967_296_i64], Some("D"))
            .await
            .is_err()
    );
    assert!(
        client
            .write_expansion_unit_buffer(1, 0, &[0_i64, 2_147_483_648_i64], "L")
            .await
            .is_err()
    );
    assert!(client.write_typed("DM0", "U", 65_536_u32).await.is_err());
    assert_eq!(client.traffic_stats().await.request_count, 0);
    assert!(!client.is_open().await);
}

#[tokio::test]
async fn read_expansion_unit_buffer_rejects_32_bit_buffer_end_crossing_before_send() {
    let (port, received) = start_scripted_server(|_| "OK".to_owned()).await;

    let mut options = HostLinkConnectionOptions::new(
        "127.0.0.1",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )
    .unwrap();
    options.port = port;
    let client = HostLinkClient::connect(options).await.unwrap();
    let error = client
        .read_expansion_unit_buffer(1, 59_999, 1, "D")
        .await
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("Expansion buffer span out of range")
    );
    assert!(received.lock().unwrap().is_empty());
}

#[test]
fn device_range_catalog_uses_explicit_plc_profile() {
    let catalog =
        plc_comm_kv_hostlink::device_range_catalog_for_plc_profile("keyence:kv-8000").unwrap();
    assert_eq!(catalog.plc_profile, "keyence:kv-8000");
    assert_eq!(catalog.model_code, "");
    assert!(!catalog.has_model_code);
    assert_eq!(catalog.requested_plc_profile, "keyence:kv-8000");
    assert_eq!(catalog.resolved_plc_profile, "keyence:kv-8000");
    assert_eq!(
        catalog.entry("DM").unwrap().address_range.as_deref(),
        Some("DM00000-DM65534")
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
                    if stream.write_all(&frame).await.is_err() {
                        return;
                    }
                } else {
                    partial.push(*byte);
                }
            }
        }
    });
    (port, received)
}

async fn read_test_command(stream: &mut TcpStream) -> String {
    let mut command = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        stream.read_exact(&mut byte).await.unwrap();
        if matches!(byte[0], b'\r' | b'\n') {
            if !command.is_empty() {
                return String::from_utf8(command).unwrap();
            }
        } else {
            command.push(byte[0]);
        }
    }
}
