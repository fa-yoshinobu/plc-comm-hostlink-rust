use thiserror::Error;

/// Why a completed state-changing operation returned [`HostLinkError::OutcomeUnknown`].
///
/// Dropping a Rust future produces no library result and therefore has no
/// variant here; the caller must classify that cancellation separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostLinkOutcomeUnknownReason {
    Timeout,
    Closed,
    Transport,
    MalformedResponse,
}

#[derive(Debug, Error)]
pub enum HostLinkError {
    #[error("{0}")]
    Protocol(String),
    #[error("client is not connected; call open before sending a command")]
    NotConnected,
    #[error("operation deadline expired: {0}")]
    Timeout(String),
    #[error("client was closed")]
    Closed,
    #[error("{message}")]
    Transport {
        message: String,
        #[source]
        source: Option<std::io::Error>,
    },
    #[error("state-changing operation outcome is unknown ({reason:?}): {source}")]
    OutcomeUnknown {
        reason: HostLinkOutcomeUnknownReason,
        #[source]
        source: Box<HostLinkError>,
    },
    #[error("PLC returned {code} (response={response:?})")]
    Plc { code: String, response: String },
}

impl HostLinkError {
    pub fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol(message.into())
    }

    pub(crate) fn connection(message: impl Into<String>) -> Self {
        Self::Transport {
            message: message.into(),
            source: None,
        }
    }

    pub fn transport(message: impl Into<String>, source: std::io::Error) -> Self {
        Self::Transport {
            message: message.into(),
            source: Some(source),
        }
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::Timeout(message.into())
    }

    pub fn outcome_unknown(reason: HostLinkOutcomeUnknownReason, source: HostLinkError) -> Self {
        Self::OutcomeUnknown {
            reason,
            source: Box::new(source),
        }
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
        let message = value.to_string();
        Self::transport(message, value)
    }
}
