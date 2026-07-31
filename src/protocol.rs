use crate::error::HostLinkError;
use encoding_rs::SHIFT_JIS;

pub(crate) const MAX_FRAME_BODY_BYTES: usize = 65_536;

pub fn build_frame(body: &str) -> Result<Vec<u8>, HostLinkError> {
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

pub fn decode_comment_response(raw: &[u8]) -> Result<String, HostLinkError> {
    let payload = trim_response(raw)?;
    let mut end = payload.len();
    while end > 0 && payload[end - 1] == b' ' {
        end -= 1;
    }
    let payload = &payload[..end];
    if let Ok(text) = std::str::from_utf8(payload) {
        return Ok(text.to_owned());
    }

    let (text, _, had_errors) = SHIFT_JIS.decode(payload);
    if had_errors {
        return Err(HostLinkError::protocol(
            "Response could not be decoded as UTF-8 or Shift_JIS",
        ));
    }
    Ok(text.into_owned())
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
    use super::{MAX_FRAME_BODY_BYTES, build_frame};

    #[test]
    fn frame_builder_preserves_body_and_appends_one_cr() {
        assert_eq!(build_frame(" RD DM0.U ").unwrap(), b" RD DM0.U \r");
    }

    #[test]
    fn frame_builder_rejects_injected_terminators_and_non_ascii() {
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
}
