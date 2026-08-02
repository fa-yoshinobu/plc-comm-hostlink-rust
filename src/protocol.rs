use crate::error::HostLinkError;
use crate::model::HostLinkCommentEncoding;
use encoding_rs::{SHIFT_JIS, UTF_8};

/// Maximum ASCII command-body size shared by TCP and UDP.
///
/// The terminating CR makes the complete request frame at most 65,507 bytes,
/// which fits in one IPv4 UDP payload.
pub(crate) const MAX_FRAME_BODY_BYTES: usize = 65_506;

pub fn build_frame(body: &str) -> Result<Vec<u8>, HostLinkError> {
    if body.is_empty() {
        return Err(HostLinkError::protocol(
            "Host Link command body must not be empty",
        ));
    }
    if !body.is_ascii() {
        return Err(HostLinkError::protocol(
            "Host Link command body must contain ASCII bytes only",
        ));
    }
    if body
        .as_bytes()
        .iter()
        .any(|byte| matches!(byte, b'\r' | b'\n'))
    {
        return Err(HostLinkError::protocol(
            "Host Link command body must not contain CR or LF",
        ));
    }
    let body = body.as_bytes();
    if body.len() > MAX_FRAME_BODY_BYTES {
        return Err(HostLinkError::protocol(format!(
            "Host Link command body exceeds {MAX_FRAME_BODY_BYTES} bytes"
        )));
    }
    let mut result = Vec::with_capacity(body.len() + 1);
    result.extend_from_slice(body);
    result.push(b'\r');
    Ok(result)
}

fn trim_response(raw: &[u8]) -> Result<&[u8], HostLinkError> {
    if raw.is_empty() {
        return Err(HostLinkError::protocol("Empty response"));
    }

    let mut len = raw.len();
    while len > 0 && matches!(raw[len - 1], b'\r' | b'\n') {
        len -= 1;
    }

    if len == 0 {
        return Err(HostLinkError::protocol("Malformed response frame"));
    }

    Ok(&raw[..len])
}

pub(crate) fn raw_response_body(raw: &[u8]) -> Vec<u8> {
    let mut len = raw.len();
    while len > 0 && matches!(raw[len - 1], b'\r' | b'\n') {
        len -= 1;
    }
    raw[..len].to_vec()
}

pub fn decode_response(raw: &[u8]) -> Result<String, HostLinkError> {
    let payload = trim_response(raw)?;
    let text = std::str::from_utf8(payload)
        .map_err(|_| HostLinkError::protocol("Response is not ASCII"))?;
    if !text.is_ascii() {
        return Err(HostLinkError::protocol("Response is not ASCII"));
    }
    Ok(text.to_owned())
}

pub(crate) fn comment_response_payload(raw: &[u8]) -> Result<&[u8], HostLinkError> {
    trim_response(raw)
}

pub(crate) fn ensure_comment_success(payload: &[u8]) -> Result<(), HostLinkError> {
    if payload.len() == 2 && payload[0] == b'E' && payload[1].is_ascii_digit() {
        let response =
            String::from_utf8(payload.to_vec()).expect("an E plus one ASCII digit is valid UTF-8");
        return Err(HostLinkError::plc(response.clone(), response));
    }
    Ok(())
}

pub(crate) fn decode_comment_payload(
    payload: &[u8],
    encoding: HostLinkCommentEncoding,
) -> Result<String, HostLinkError> {
    let mut end = payload.len();
    while end > 0 && payload[end - 1] == b' ' {
        end -= 1;
    }
    let payload = &payload[..end];
    let (codec, name) = match encoding {
        HostLinkCommentEncoding::Utf8 => (UTF_8, "UTF-8"),
        HostLinkCommentEncoding::Cp932 => (SHIFT_JIS, "CP932/Windows-31J"),
    };
    if matches!(encoding, HostLinkCommentEncoding::Cp932)
        && !cp932_bytes_are_structurally_valid(payload)
    {
        return Err(invalid_comment_encoding(name));
    }
    codec
        .decode_without_bom_handling_and_without_replacement(payload)
        .map(|text| text.into_owned())
        .ok_or_else(|| invalid_comment_encoding(name))
}

fn cp932_bytes_are_structurally_valid(payload: &[u8]) -> bool {
    let mut index = 0;
    while index < payload.len() {
        match payload[index] {
            0x00..=0x7F | 0xA1..=0xDF => index += 1,
            0x81..=0x9F | 0xE0..=0xFC => {
                let Some(&trail) = payload.get(index + 1) else {
                    return false;
                };
                if !matches!(trail, 0x40..=0x7E | 0x80..=0xFC) {
                    return false;
                }
                index += 2;
            }
            _ => return false,
        }
    }
    true
}

fn invalid_comment_encoding(name: &str) -> HostLinkError {
    HostLinkError::protocol(format!(
        "RDC comment payload is not valid {name}; no fallback or replacement was applied"
    ))
}

pub fn ensure_success(response_text: String) -> Result<String, HostLinkError> {
    if response_text.len() == 2
        && response_text.starts_with('E')
        && response_text.as_bytes()[1].is_ascii_digit()
    {
        let code = response_text.clone();
        return Err(HostLinkError::plc(code, response_text));
    }

    Ok(response_text)
}

pub fn split_data_tokens(response_text: &str) -> Vec<String> {
    response_text
        .split([' ', ','])
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{MAX_FRAME_BODY_BYTES, build_frame, decode_comment_payload};
    use crate::model::HostLinkCommentEncoding;

    #[test]
    fn frame_builder_preserves_body_and_appends_one_cr() {
        assert_eq!(build_frame(" RD DM0.U ").unwrap(), b" RD DM0.U \r");
    }

    #[test]
    fn frame_builder_rejects_injected_terminators_and_non_ascii() {
        assert!(build_frame("").is_err());
        assert!(build_frame("RD DM0.U\rWR DM0.U 1").is_err());
        assert!(build_frame("RDC 日本語").is_err());
    }

    #[test]
    fn frame_builder_accepts_exact_body_capacity_and_rejects_one_byte_over() {
        let maximum = "A".repeat(MAX_FRAME_BODY_BYTES);
        let frame = build_frame(&maximum).unwrap();
        assert_eq!(frame.len(), MAX_FRAME_BODY_BYTES + 1);
        assert_eq!(frame.last(), Some(&b'\r'));

        let oversized = "A".repeat(MAX_FRAME_BODY_BYTES + 1);
        assert!(build_frame(&oversized).is_err());
    }

    #[test]
    fn comment_decoder_uses_only_the_selected_codec() {
        let ambiguous = b"\xC2\xA2";
        assert_eq!(
            decode_comment_payload(ambiguous, HostLinkCommentEncoding::Utf8).unwrap(),
            "¢"
        );
        assert_eq!(
            decode_comment_payload(ambiguous, HostLinkCommentEncoding::Cp932).unwrap(),
            "ﾂ｢"
        );

        let cp932_only = b"\x82\xA0";
        assert!(decode_comment_payload(cp932_only, HostLinkCommentEncoding::Utf8).is_err());
        assert_eq!(
            decode_comment_payload(cp932_only, HostLinkCommentEncoding::Cp932).unwrap(),
            "あ"
        );

        let utf8_bom = b"\xEF\xBB\xBFA";
        assert_eq!(
            decode_comment_payload(utf8_bom, HostLinkCommentEncoding::Utf8).unwrap(),
            "\u{FEFF}A"
        );
        assert!(decode_comment_payload(utf8_bom, HostLinkCommentEncoding::Cp932).is_err());
    }

    #[test]
    fn comment_decoder_is_strict_and_trims_only_ascii_space_padding() {
        assert!(decode_comment_payload(b"\x81\x30", HostLinkCommentEncoding::Cp932).is_err());
        assert!(decode_comment_payload(b"\xFF", HostLinkCommentEncoding::Utf8).is_err());
        assert_eq!(
            decode_comment_payload(b"A B  ", HostLinkCommentEncoding::Utf8).unwrap(),
            "A B"
        );
        assert_eq!(
            decode_comment_payload("TEXT \t".as_bytes(), HostLinkCommentEncoding::Utf8).unwrap(),
            "TEXT \t"
        );
    }

    #[test]
    fn cp932_decoder_matches_cross_runtime_boundary_contract() {
        assert_eq!(
            decode_comment_payload(b"\x1A\x1C\x7F", HostLinkCommentEncoding::Cp932).unwrap(),
            "\u{001A}\u{001C}\u{007F}"
        );

        for invalid in [b"\x80".as_slice(), b"\xA0", b"\xFD", b"\xFE", b"\xFF"] {
            assert!(decode_comment_payload(invalid, HostLinkCommentEncoding::Cp932).is_err());
        }
        for invalid in [
            b"\x81".as_slice(),
            b"\x81\x30",
            b"\x81\x7F",
            b"\x81\xAD",
            b"\x87\x9D",
            b"\xEE\xFD",
            b"\xEF\xFC",
            b"\xFC\x4C",
        ] {
            assert!(decode_comment_payload(invalid, HostLinkCommentEncoding::Cp932).is_err());
        }

        assert_eq!(
            decode_comment_payload(b"\x81\x80", HostLinkCommentEncoding::Cp932).unwrap(),
            "\u{00F7}"
        );
        assert_eq!(
            decode_comment_payload(b"\x87\x90", HostLinkCommentEncoding::Cp932).unwrap(),
            "\u{2252}"
        );
        assert_eq!(
            decode_comment_payload(b"\xED\x40", HostLinkCommentEncoding::Cp932).unwrap(),
            "\u{7E8A}"
        );
        assert_eq!(
            decode_comment_payload(b"\xFA\x4A", HostLinkCommentEncoding::Cp932).unwrap(),
            "\u{2160}"
        );
    }
}
