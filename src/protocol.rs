use crate::error::HostLinkError;
use encoding_rs::SHIFT_JIS;

pub fn build_frame(body: &str, append_lf: bool) -> Vec<u8> {
    let body = body.trim().as_bytes();
    let mut result = Vec::with_capacity(body.len() + 1 + usize::from(append_lf));
    result.extend_from_slice(body);
    result.push(b'\r');
    if append_lf {
        result.push(b'\n');
    }
    result
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
        .split(' ')
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
