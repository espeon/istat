use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    // Session errors
    SessionNotFound,
    SessionExpired,
    InvalidSessionState,

    // OAuth errors
    InvalidGrant,
    InvalidRequest(String),
    InvalidClient,
    UnauthorizedClient,
    UnsupportedGrantType,
    Unauthorized,
    DpopProofRequired,
    DpopNonceRequired(String), // Contains the nonce to send back

    // DPoP errors
    DpopMethodMismatch,
    DpopUrlMismatch,
    DpopNonceReused,
    DpopExpired,
    DpopInvalid,

    // Key errors
    KeyNotFound,
    KeyGenerationFailed,

    // Storage errors
    StorageError(String),

    // Network errors
    NetworkError(String),

    // Generic errors
    Internal(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::SessionNotFound => write!(f, "session not found"),
            Error::SessionExpired => write!(f, "session expired"),
            Error::InvalidSessionState => write!(f, "invalid session state"),
            Error::InvalidGrant => write!(f, "invalid_grant"),
            Error::InvalidRequest(msg) => write!(f, "invalid_request: {}", msg),
            Error::InvalidClient => write!(f, "invalid_client"),
            Error::UnauthorizedClient => write!(f, "unauthorized_client"),
            Error::UnsupportedGrantType => write!(f, "unsupported_grant_type"),
            Error::Unauthorized => write!(f, "unauthorized"),
            Error::DpopProofRequired => write!(f, "DPoP proof required"),
            Error::DpopNonceRequired(_) => write!(f, "use_dpop_nonce"),
            Error::DpopMethodMismatch => write!(f, "DPoP htm mismatch"),
            Error::DpopUrlMismatch => write!(f, "DPoP htu mismatch"),
            Error::DpopNonceReused => write!(f, "DPoP nonce reused"),
            Error::DpopExpired => write!(f, "DPoP proof expired"),
            Error::DpopInvalid => write!(f, "invalid DPoP proof"),
            Error::KeyNotFound => write!(f, "key not found"),
            Error::KeyGenerationFailed => write!(f, "key generation failed"),
            Error::StorageError(msg) => write!(f, "storage error: {}", msg),
            Error::NetworkError(msg) => write!(f, "network error: {}", msg),
            Error::Internal(msg) => write!(f, "internal error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Error::Internal(e.to_string())
    }
}

// axum IntoResponse implementation
impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        use axum::Json;
        use axum::http::StatusCode;

        let status = match self {
            Error::SessionNotFound | Error::SessionExpired | Error::Unauthorized => {
                StatusCode::UNAUTHORIZED
            }
            Error::InvalidGrant | Error::InvalidClient => StatusCode::BAD_REQUEST,
            Error::DpopProofRequired => StatusCode::UNAUTHORIZED,
            Error::DpopNonceRequired(ref nonce) => {
                // Return OAuth error format with DPoP-Nonce header
                let error_body = serde_json::json!({
                    "error": "use_dpop_nonce",
                    "error_description": "Authorization server requires nonce in DPoP proof"
                });

                return (
                    StatusCode::BAD_REQUEST,
                    [(
                        axum::http::header::HeaderName::from_static("dpop-nonce"),
                        nonce.clone(),
                    )],
                    Json(error_body),
                )
                    .into_response();
            }
            Error::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, self.to_string()).into_response()
    }
}
