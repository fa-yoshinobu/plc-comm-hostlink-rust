use thiserror::Error;

#[derive(Debug, Error)]
pub enum HostLinkError {
    #[error("{0}")]
    Protocol(String),
    #[error("client is not connected; call open before sending a command")]
    NotConnected,
    #[error("{0}")]
    Connection(String),
    #[error("PLC returned {code} (response={response:?})")]
    Plc { code: String, response: String },
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
