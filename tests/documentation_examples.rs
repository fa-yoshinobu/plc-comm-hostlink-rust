const GETTING_STARTED: &str = include_str!("../docs/GETTING_STARTED.md");
const USAGE_GUIDE: &str = include_str!("../docs/USAGE_GUIDE.md");

use plc_comm_kv_hostlink::{HostLinkClient, HostLinkMonitorWord};

#[allow(dead_code)]
async fn compile_cleanup_control_flow(
    client: &HostLinkClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let original = client.read_typed("DM120", "U").await?;
    client.write_typed("DM120", "U", 1234_u16).await?;
    let readback_result = client.read_typed("DM120", "U").await;
    let restore_result = client.write_typed("DM120", "U", original).await;
    restore_result?;
    let _ = readback_result?;

    let original = client
        .read_consecutive("DM200", 2, Some("D"))
        .await?
        .into_iter()
        .map(|value| value.parse::<u32>())
        .collect::<Result<Vec<_>, _>>()?;
    client
        .write_consecutive("DM200", &[1_u32, 2_u32], Some("D"))
        .await?;
    let readback_result = client.read_consecutive("DM200", 2, Some("D")).await;
    let restore_result = client
        .write_consecutive("DM200", &original, Some("D"))
        .await;
    restore_result?;
    let _ = readback_result?;

    let original = client
        .read_expansion_unit_buffer(2, 200, 2, "S")
        .await?
        .into_iter()
        .map(|value| value.parse::<i16>())
        .collect::<Result<Vec<_>, _>>()?;
    client
        .write_expansion_unit_buffer(2, 200, &[7_i16, 8_i16], "S")
        .await?;
    let readback_result = client.read_expansion_unit_buffer(2, 200, 2, "S").await;
    let restore_result = client
        .write_expansion_unit_buffer(2, 200, &original, "S")
        .await;
    restore_result?;
    let _ = readback_result?;
    Ok(())
}

#[allow(dead_code)]
async fn compile_packed_monitor_example(
    client: &HostLinkClient,
) -> Result<(), Box<dyn std::error::Error>> {
    client
        .register_monitor_words(&[
            HostLinkMonitorWord::numeric("DM120", "U"),
            HostLinkMonitorWord::packed_direct_bits_u16("R5000"),
        ])
        .await?;
    let _values = client.read_monitor_words().await?;
    Ok(())
}

fn section<'a>(document: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = document.find(start).expect("section start");
    let tail = &document[start_index..];
    let end_index = tail.find(end).expect("section end");
    &tail[..end_index]
}

fn assert_ordered(text: &str, needles: &[&str]) {
    let mut offset = 0;
    for needle in needles {
        let relative = text[offset..]
            .find(needle)
            .unwrap_or_else(|| panic!("missing {needle:?}"));
        offset += relative + needle.len();
    }
}

#[test]
fn controlled_writes_restore_before_propagating_readback_errors() {
    assert_ordered(
        GETTING_STARTED,
        &[
            "let readback_result = client.read_typed",
            "let restore_result = client.write_typed",
            "restore_result?;",
            "let readback = readback_result?;",
        ],
    );

    let low_level = section(
        USAGE_GUIDE,
        "Low-level numeric APIs",
        "Custom low-level values",
    );
    assert_ordered(
        low_level,
        &[
            ".map(|value| value.parse::<u32>())",
            "let readback_result = client.read_consecutive",
            "let restore_result = client.write_consecutive",
            "restore_result?;",
            "let values = readback_result?;",
        ],
    );

    let expansion = section(
        USAGE_GUIDE,
        "## Expansion-unit buffer access",
        "## Comments",
    );
    assert_ordered(
        expansion,
        &[
            ".map(|value| value.parse::<i16>())",
            "let readback_result = client",
            "let restore_result = client",
            "restore_result?;",
            "let values = readback_result?;",
        ],
    );
}

#[test]
fn clock_example_declares_non_restorable_state_change() {
    let clock = section(USAGE_GUIDE, "## PLC clock", "## Shared clients");
    assert!(clock.contains("changes PLC state"));
    assert!(clock.contains("exact automatic restore impossible"));
    assert!(clock.contains("controlled PLC"));
}

#[test]
fn packed_monitor_example_names_wire_and_response_semantics() {
    let monitor = section(
        USAGE_GUIDE,
        "## Word monitor registration",
        "## Expansion-unit buffer access",
    );
    assert!(monitor.contains("packed_direct_bits_u16"));
    assert!(monitor.contains("MWS R5000"));
    assert!(monitor.contains("1-5 ASCII decimal"));
    assert!(monitor.contains("00013"));
    assert!(monitor.contains("six or more digits"));
    assert!(monitor.contains("register_monitor_bits"));
}
