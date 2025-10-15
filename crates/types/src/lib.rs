use axum::{
    http::StatusCode, response::{IntoResponse, Response}, Json
};

pub const ERROR_CODE_SERVICE_NOT_FOUND: (i32, &str) = (10001, "service not found");
pub const ERROR_CODE_INTERNAL_ERROR: (i32, &str) = (10002, "internal error");
pub const ERROR_CODE_RPC_TIMEOUT: (i32, &str) = (10003, "rpc timeout");
pub const ERROR_CODE_DESERIALIZE: (i32, &str) = (10004, "internal error");
pub const ERROR_CODE_RPC_NOT_IMPLEMENTED: (i32, &str)= (10005, "rpc not implemented");

type ErrorType = (i32, &'static str);

#[derive(Debug, bitcode::Encode, bitcode::Decode, serde::Serialize, serde::Deserialize)]
pub struct Error {
    pub code: i32,
    pub message: String,
}

pub type Result<T> = std::result::Result<T, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error({}): {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let body = Json(self);
        (StatusCode::OK, body).into_response()
    }
}

impl From<ErrorType> for Error {
    fn from(value: ErrorType) -> Self {
        Error{
            code: value.0,
            message: value.1.to_string(),
        }
    }
}

#[derive(Debug, bitcode::Encode, bitcode::Decode, serde::Serialize, serde::Deserialize)]
pub struct ClusterRequest{
    pub zid: String,
    pub version: String,
    pub query: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, bitcode::Encode, bitcode::Decode, serde::Serialize, serde::Deserialize)]
pub struct ClusterResponse{
    pub zid: String,
    pub status: u16,
    pub payload: Option<Vec<u8>>,
}

impl IntoResponse for ClusterResponse {
    fn into_response(self) -> Response {
        let status_code = StatusCode::from_u16(self.status).unwrap_or_default();
        let json = match self.payload {
            Some(v) => {
                serde_json::from_slice(&v).unwrap_or_default()
            },
            None => {
                serde_json::Value::Null
            }   
        };
        let body = Json(json);
        (status_code, body).into_response()
    }
}