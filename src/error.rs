use thiserror::Error;

const ERROR_CODE_MESSAGES: &[(&str, &str)] = &[
    ("E0", "Abnormal device No."),
    ("E1", "Abnormal command"),
    ("E2", "Program not registered"),
    ("E4", "Write disabled"),
    ("E5", "Unit error"),
    ("E6", "No comments"),
];

pub fn decode_error_code(code: &str) -> &'static str {
    ERROR_CODE_MESSAGES
        .iter()
        .find_map(|(key, value)| (*key == code).then_some(*value))
        .unwrap_or("Unknown error")
}

#[derive(Debug, Error)]
pub enum HostLinkError {
    #[error("{0}")]
    Protocol(String),
    #[error("{0}")]
    Connection(String),
    #[error("{code}: {message} (response={response:?})")]
    Plc {
        code: String,
        response: String,
        message: &'static str,
    },
}

impl HostLinkError {
    pub fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol(message.into())
    }

    pub fn connection(message: impl Into<String>) -> Self {
        Self::Connection(message.into())
    }

    pub fn plc(code: impl Into<String>, response: impl Into<String>) -> Self {
        let code = code.into();
        Self::Plc {
            message: decode_error_code(&code),
            response: response.into(),
            code,
        }
    }
}

impl From<std::io::Error> for HostLinkError {
    fn from(value: std::io::Error) -> Self {
        Self::connection(value.to_string())
    }
}
