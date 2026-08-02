use futures_util::{StreamExt, pin_mut};
use plc_comm_kv_hostlink::{
    HostLinkClient, HostLinkConnectionOptions, HostLinkError, HostLinkMonitorWord,
    HostLinkOutcomeUnknownReason, HostLinkTransportMode, HostLinkValue,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

fn options(port: u16, transport: HostLinkTransportMode) -> HostLinkConnectionOptions {
    let mut options =
        HostLinkConnectionOptions::new("127.0.0.1", port, transport, "keyence:kv-8000").unwrap();
    options.timeout = Duration::from_secs(2);
    options
}

async fn read_command(stream: &mut TcpStream) -> String {
    let mut command = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if stream.read_exact(&mut byte).await.is_err() {
            return String::new();
        }
        if matches!(byte[0], b'\r' | b'\n') {
            if !command.is_empty() {
                return String::from_utf8(command).unwrap();
            }
        } else {
            command.push(byte[0]);
        }
    }
}

async fn received_more_data(stream: &mut TcpStream, wait: Duration) -> bool {
    let mut byte = [0u8; 1];
    matches!(
        tokio::time::timeout(wait, stream.read(&mut byte)).await,
        Ok(Ok(read)) if read > 0
    )
}

async fn scripted_tcp<F>(response: F) -> (u16, Arc<Mutex<Vec<String>>>)
where
    F: Fn(&str) -> Vec<u8> + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let commands = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&commands);
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        loop {
            let command =
                match tokio::time::timeout(Duration::from_secs(2), read_command(&mut stream)).await
                {
                    Ok(command) => command,
                    Err(_) => return,
                };
            if command.is_empty() {
                return;
            }
            recorded.lock().unwrap().push(command.clone());
            let mut frame = response(&command);
            frame.extend_from_slice(b"\r\n");
            if stream.write_all(&frame).await.is_err() {
                return;
            }
        }
    });
    (port, commands)
}

#[tokio::test]
async fn tcp_rejects_a_second_nonempty_response_and_never_reuses_it() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        assert_eq!(read_command(&mut stream).await, "RD DM0.U");
        stream.write_all(b"1\r\n2\r\n").await.unwrap();
        received_more_data(&mut stream, Duration::from_millis(100)).await
    });
    let client = HostLinkClient::connect(options(port, HostLinkTransportMode::Tcp))
        .await
        .unwrap();

    let error = client.read("DM0", Some("U")).await.unwrap_err();
    assert!(matches!(error, HostLinkError::Protocol(_)));
    assert!(!client.is_open().await);
    assert!(matches!(
        client.read("DM1", Some("U")).await,
        Err(HostLinkError::NotConnected)
    ));
    assert!(!server.await.unwrap());
}

#[tokio::test]
async fn tcp_ignores_only_empty_separators_and_rejects_delayed_unowned_data_before_send() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        assert_eq!(read_command(&mut stream).await, "RD DM0.U");
        stream.write_all(b"1\r\n\r\n").await.unwrap();
        assert_eq!(read_command(&mut stream).await, "RD DM1.U");
        stream.write_all(b"2\r").await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        stream.write_all(b"999\r").await.unwrap();
        received_more_data(&mut stream, Duration::from_millis(150)).await
    });
    let client = HostLinkClient::connect(options(port, HostLinkTransportMode::Tcp))
        .await
        .unwrap();

    assert_eq!(client.read("DM0", Some("U")).await.unwrap(), ["1"]);
    assert_eq!(client.read("DM1", Some("U")).await.unwrap(), ["2"]);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let error = client.write("DM2", 3_u16, Some("U")).await.unwrap_err();
    assert!(matches!(error, HostLinkError::Protocol(_)));
    assert!(!client.is_open().await);
    assert!(!server.await.unwrap());
}

#[tokio::test]
async fn state_changing_tcp_surplus_response_is_never_false_success() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        assert_eq!(read_command(&mut stream).await, "WR DM0.U 1");
        stream.write_all(b"OK\rEXTRA\r").await.unwrap();
    });
    let client = HostLinkClient::connect(options(port, HostLinkTransportMode::Tcp))
        .await
        .unwrap();

    assert!(matches!(
        client.write("DM0", 1_u16, Some("U")).await,
        Err(HostLinkError::OutcomeUnknown {
            reason: HostLinkOutcomeUnknownReason::MalformedResponse,
            ..
        })
    ));
    assert!(!client.is_open().await);
}

#[tokio::test]
async fn monitor_words_validate_each_registered_format_and_normalize_hex() {
    let (port, _) = scripted_tcp(|command| match command {
        command if command.starts_with("MWS ") => b"OK".to_vec(),
        "MWR" => b"65535 -32768 a 4294967295 -2147483648 ON".to_vec(),
        _ => b"E1".to_vec(),
    })
    .await;
    let client = HostLinkClient::connect(options(port, HostLinkTransportMode::Tcp))
        .await
        .unwrap();
    client
        .register_monitor_words(&[
            HostLinkMonitorWord::numeric("DM0", "U"),
            HostLinkMonitorWord::numeric("DM1", "S"),
            HostLinkMonitorWord::numeric("DM2", "H"),
            HostLinkMonitorWord::numeric("DM3", "D"),
            HostLinkMonitorWord::numeric("DM5", "L"),
            HostLinkMonitorWord::direct_bit("MR0"),
        ])
        .await
        .unwrap();

    assert_eq!(
        client.read_monitor_words().await.unwrap(),
        ["65535", "-32768", "000A", "4294967295", "-2147483648", "ON"]
    );
}

#[tokio::test]
async fn monitor_words_reject_malformed_tokens_for_every_registered_format() {
    for (entry, response) in [
        (HostLinkMonitorWord::numeric("DM0", "U"), "-1"),
        (HostLinkMonitorWord::numeric("DM0", "S"), "32768"),
        (HostLinkMonitorWord::numeric("DM0", "H"), "10000"),
        (HostLinkMonitorWord::numeric("DM0", "D"), "-1"),
        (HostLinkMonitorWord::numeric("DM0", "L"), "2147483648"),
        (HostLinkMonitorWord::direct_bit("MR0"), "2"),
    ] {
        let (port, _) = scripted_tcp(move |command| match command {
            command if command.starts_with("MWS ") => b"OK".to_vec(),
            "MWR" => response.as_bytes().to_vec(),
            _ => b"E1".to_vec(),
        })
        .await;
        let client = HostLinkClient::connect(options(port, HostLinkTransportMode::Tcp))
            .await
            .unwrap();
        client.register_monitor_words(&[entry]).await.unwrap();
        assert!(matches!(
            client.read_monitor_words().await,
            Err(HostLinkError::Protocol(_))
        ));
        assert!(!client.is_open().await, "response={response}");
    }
}

#[tokio::test]
async fn all_semantic_hex_paths_return_four_uppercase_digits_while_raw_is_exact() {
    let (port, _) = scripted_tcp(|command| match command {
        "RD DM0.H" => b"0".to_vec(),
        "RDS DM1.H 4" => b"a ff FFFF 0000".to_vec(),
        "RD DM5.H" => b"a".to_vec(),
        "RDS DM6.U 1" => b"15".to_vec(),
        "MWS DM7.H" => b"OK".to_vec(),
        "MWR" => b"ff".to_vec(),
        "RDS DM8.U 1" => b"10".to_vec(),
        "RDE DM9.H 1" => b"f".to_vec(),
        "URD 01 10.H 1" => b"a".to_vec(),
        "RD R000.H" => b"0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 0".to_vec(),
        "RD T0.H" => b"0 1 a".to_vec(),
        "RAW" => b"a".to_vec(),
        _ => b"E1".to_vec(),
    })
    .await;
    let client = HostLinkClient::connect(options(port, HostLinkTransportMode::Tcp))
        .await
        .unwrap();

    assert_eq!(client.read("DM0", Some("H")).await.unwrap(), ["0000"]);
    assert_eq!(
        client.read_consecutive("DM1", 4, Some("H")).await.unwrap(),
        ["000A", "00FF", "FFFF", "0000"]
    );
    assert_eq!(
        client.read_typed("DM5", "H").await.unwrap(),
        HostLinkValue::Text("000A".to_owned())
    );
    assert_eq!(
        client.read_named(&["DM6:H"]).await.unwrap()["DM6:H"],
        HostLinkValue::Text("000F".to_owned())
    );
    client
        .register_monitor_words(&[HostLinkMonitorWord::numeric("DM7", "H")])
        .await
        .unwrap();
    assert_eq!(client.read_monitor_words().await.unwrap(), ["00FF"]);
    let poll_addresses = ["DM8:H"];
    {
        let poll = client.poll(&poll_addresses, Duration::from_millis(10));
        pin_mut!(poll);
        assert_eq!(
            poll.next().await.unwrap().unwrap()["DM8:H"],
            HostLinkValue::Text("000A".to_owned())
        );
    }
    assert_eq!(
        client
            .read_consecutive_legacy("DM9", 1, Some("H"))
            .await
            .unwrap(),
        ["000F"]
    );
    assert_eq!(
        client
            .read_expansion_unit_buffer(1, 10, 1, "H")
            .await
            .unwrap(),
        ["000A"]
    );
    assert_eq!(
        client.read_typed("R0", "H").await.unwrap(),
        HostLinkValue::Text("000A".to_owned())
    );
    assert_eq!(
        client.read("T0", Some("H")).await.unwrap(),
        ["0000", "0001", "000A"]
    );
    assert_eq!(
        client.read_typed("T0", "H").await.unwrap(),
        HostLinkValue::Text("000A".to_owned())
    );
    assert_eq!(client.send_raw("RAW").await.unwrap(), b"a");
}

#[tokio::test]
async fn descending_named_read_boundary_keeps_compilation_and_poll_reuses_the_plan() {
    let (port, commands) = scripted_tcp(|command| match command {
        "RDS DM100.U 1" => b"100".to_vec(),
        "RDS DM0.U 2" => b"0 1".to_vec(),
        _ => b"E1".to_vec(),
    })
    .await;
    let client = HostLinkClient::connect(options(port, HostLinkTransportMode::Tcp))
        .await
        .unwrap();
    let addresses = ["DM100:U", "DM0:U", "DM1:U"];

    let named = client.read_named(&addresses).await.unwrap();
    assert_eq!(named["DM100:U"], HostLinkValue::U16(100));
    assert_eq!(named["DM0:U"], HostLinkValue::U16(0));
    assert_eq!(named["DM1:U"], HostLinkValue::U16(1));

    {
        let poll = client.poll(&addresses, Duration::from_millis(10));
        pin_mut!(poll);
        let polled = poll.next().await.unwrap().unwrap();
        assert_eq!(polled["DM100:U"], HostLinkValue::U16(100));
        assert_eq!(polled["DM0:U"], HostLinkValue::U16(0));
        assert_eq!(polled["DM1:U"], HostLinkValue::U16(1));
    }

    assert_eq!(
        commands.lock().unwrap().as_slice(),
        [
            "RDS DM0.U 2",
            "RDS DM100.U 1",
            "RDS DM0.U 2",
            "RDS DM100.U 1"
        ]
    );
}

#[tokio::test]
async fn malformed_hex_semantic_responses_retire_transport() {
    for response in ["-1", "10000", "GG", "0x1"] {
        let (port, _) = scripted_tcp(move |_| response.as_bytes().to_vec()).await;
        let client = HostLinkClient::connect(options(port, HostLinkTransportMode::Tcp))
            .await
            .unwrap();
        assert!(matches!(
            client.read("DM0", Some("H")).await,
            Err(HostLinkError::Protocol(_))
        ));
        assert!(!client.is_open().await, "response={response}");
    }
}

#[tokio::test]
async fn every_plc_error_code_keeps_the_semantic_tcp_connection_open() {
    for number in 0..=9 {
        let code = format!("E{number}");
        let response_code = code.clone();
        let (port, commands) = scripted_tcp(move |command| {
            if command.starts_with("WR ") {
                response_code.as_bytes().to_vec()
            } else {
                b"7".to_vec()
            }
        })
        .await;
        let client = HostLinkClient::connect(options(port, HostLinkTransportMode::Tcp))
            .await
            .unwrap();

        let error = client.write("DM0", 1_u16, Some("U")).await.unwrap_err();
        assert!(
            matches!(error, HostLinkError::Plc { ref code, .. } if code == &format!("E{number}"))
        );
        assert!(client.is_open().await);
        assert_eq!(client.read("DM1", Some("U")).await.unwrap(), ["7"]);
        assert_eq!(commands.lock().unwrap().len(), 2);
    }
}

#[tokio::test]
async fn normal_udp_requests_reuse_one_endpoint_and_ignore_foreign_datagrams() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let mut buffer = [0u8; 256];
        let (_, first_peer) = socket.recv_from(&mut buffer).await.unwrap();
        socket.send_to(b"1\r", first_peer).await.unwrap();
        let (_, second_peer) = socket.recv_from(&mut buffer).await.unwrap();
        assert_eq!(first_peer, second_peer);
        let foreign = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        foreign.send_to(b"999\r", second_peer).await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        socket.send_to(b"2\r", second_peer).await.unwrap();
        (first_peer, second_peer)
    });
    let client = HostLinkClient::connect(options(port, HostLinkTransportMode::Udp))
        .await
        .unwrap();

    assert_eq!(client.read("DM0", Some("U")).await.unwrap(), ["1"]);
    assert_eq!(client.read("DM1", Some("U")).await.unwrap(), ["2"]);
    assert!(client.is_open().await);
    let (first, second) = server.await.unwrap();
    assert_eq!(first, second);
    assert_eq!(client.traffic_stats().await.request_count, 2);
}

#[tokio::test]
async fn unowned_udp_datagram_before_send_retires_socket_without_sending() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let (inject_tx, inject_rx) = tokio::sync::oneshot::channel();
    let (injected_tx, injected_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut buffer = [0u8; 256];
        let (_, first_peer) = socket.recv_from(&mut buffer).await.unwrap();
        socket.send_to(b"1\r", first_peer).await.unwrap();
        inject_rx.await.unwrap();
        socket.send_to(b"LATE\r", first_peer).await.unwrap();
        injected_tx.send(()).unwrap();
        let (_, replacement_peer) = socket.recv_from(&mut buffer).await.unwrap();
        socket.send_to(b"2\r", replacement_peer).await.unwrap();
        (first_peer, replacement_peer)
    });
    let client = HostLinkClient::connect(options(port, HostLinkTransportMode::Udp))
        .await
        .unwrap();

    assert_eq!(client.send_raw("FIRST").await.unwrap(), b"1");
    inject_tx.send(()).unwrap();
    injected_rx.await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(matches!(
        client.send_raw("BLOCKED").await,
        Err(HostLinkError::Protocol(_))
    ));
    assert_eq!(client.traffic_stats().await.request_count, 1);
    assert!(client.is_open().await);
    assert_eq!(client.send_raw("REPLACEMENT").await.unwrap(), b"2");
    let (first_peer, replacement_peer) = server.await.unwrap();
    assert_ne!(first_peer, replacement_peer);
}

#[tokio::test]
async fn udp_plc_error_keeps_logical_session_and_reuses_the_valid_socket() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let mut buffer = [0u8; 256];
        let (_, first_peer) = socket.recv_from(&mut buffer).await.unwrap();
        socket.send_to(b"E9\r", first_peer).await.unwrap();
        let (_, second_peer) = socket.recv_from(&mut buffer).await.unwrap();
        socket.send_to(b"7\r", second_peer).await.unwrap();
        (first_peer, second_peer)
    });
    let client = HostLinkClient::connect(options(port, HostLinkTransportMode::Udp))
        .await
        .unwrap();

    assert!(matches!(
        client.write("DM0", 1_u16, Some("U")).await,
        Err(HostLinkError::Plc { ref code, .. }) if code == "E9"
    ));
    assert!(client.is_open().await);
    assert_eq!(client.read("DM1", Some("U")).await.unwrap(), ["7"]);
    let (first, second) = server.await.unwrap();
    assert_eq!(first, second);
}

#[tokio::test]
async fn closing_an_active_udp_write_retires_that_generation_and_reports_unknown() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let (seen_tx, seen_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let mut buffer = [0u8; 256];
        let _ = socket.recv_from(&mut buffer).await.unwrap();
        seen_tx.send(()).unwrap();
    });
    let client = HostLinkClient::connect(options(port, HostLinkTransportMode::Udp))
        .await
        .unwrap();
    let request_client = client.clone();
    let request = tokio::spawn(async move { request_client.write("DM0", 1_u16, Some("U")).await });
    seen_rx.await.unwrap();
    client.close().await.unwrap();

    assert!(matches!(
        request.await.unwrap(),
        Err(HostLinkError::OutcomeUnknown {
            reason: HostLinkOutcomeUnknownReason::Closed,
            ..
        })
    ));
    assert!(!client.is_open().await);
}
