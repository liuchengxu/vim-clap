use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::{MethodCall, Notification};

/// Request message actively sent from the Vim side.
///
/// Message sent via `clap#client#notify` or `clap#client#call`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Call {
    MethodCall(MethodCall),
    Notification(Notification),
}

impl Call {
    pub fn session_id(&self) -> Option<u64> {
        match self {
            Self::MethodCall(method_call) => Some(method_call.session_id),
            Self::Notification(notification) => notification.session_id,
        }
    }
}

/// Message pass through the stdio channel.
///
/// RawMessage are composed of [`Call`] and the response message
/// to a call initiated on Rust side.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RawMessage {
    MethodCall(MethodCall),
    Notification(Notification),
    /// Response of a message requested from Rust.
    Output(Output),
}

type Id = u64;

/// Successful response
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Success {
    /// Result
    pub result: Value,
    /// Correlation id
    pub id: Id,
}

/// Unsuccessful response
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    /// Error
    pub error: Error,
    /// Correlation id
    pub id: Id,
}

/// JSONRPC error code
#[derive(Debug, PartialEq, Clone)]
pub enum ErrorCode {
    /// Invalid JSON was received by the server.
    /// An error occurred on the server while parsing the JSON text.
    ParseError,
    /// The JSON sent is not a valid Request object.
    InvalidRequest,
    /// The method does not exist / is not available.
    MethodNotFound,
    /// Invalid method parameter(s).
    InvalidParams,
    /// Internal JSON-RPC error.
    InternalError,
    /// Reserved for implementation-defined server-errors.
    ServerError(i64),
}

impl ErrorCode {
    /// Returns integer code value
    pub fn code(&self) -> i64 {
        match *self {
            ErrorCode::ParseError => -32700,
            ErrorCode::InvalidRequest => -32600,
            ErrorCode::MethodNotFound => -32601,
            ErrorCode::InvalidParams => -32602,
            ErrorCode::InternalError => -32603,
            ErrorCode::ServerError(code) => code,
        }
    }

    /// Returns human-readable description
    pub fn description(&self) -> String {
        let desc = match *self {
            ErrorCode::ParseError => "Parse error",
            ErrorCode::InvalidRequest => "Invalid request",
            ErrorCode::MethodNotFound => "Method not found",
            ErrorCode::InvalidParams => "Invalid params",
            ErrorCode::InternalError => "Internal error",
            ErrorCode::ServerError(_) => "Server error",
        };
        desc.to_string()
    }
}

impl From<i64> for ErrorCode {
    fn from(code: i64) -> Self {
        match code {
            -32700 => ErrorCode::ParseError,
            -32600 => ErrorCode::InvalidRequest,
            -32601 => ErrorCode::MethodNotFound,
            -32602 => ErrorCode::InvalidParams,
            -32603 => ErrorCode::InternalError,
            code => ErrorCode::ServerError(code),
        }
    }
}

impl<'a> Deserialize<'a> for ErrorCode {
    fn deserialize<D>(deserializer: D) -> Result<ErrorCode, D::Error>
    where
        D: Deserializer<'a>,
    {
        let code: i64 = Deserialize::deserialize(deserializer)?;
        Ok(ErrorCode::from(code))
    }
}

impl Serialize for ErrorCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(self.code())
    }
}

/// Error object as defined in Spec
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Error {
    /// Code
    pub code: ErrorCode,
    /// Message
    pub message: String,
    /// Optional data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Error {
    /// Wraps given `ErrorCode`
    pub fn new(code: ErrorCode) -> Self {
        Error {
            message: code.description(),
            code,
            data: None,
        }
    }

    /// Creates new `ParseError`
    pub fn parse_error() -> Self {
        Self::new(ErrorCode::ParseError)
    }

    /// Creates new `InvalidRequest`
    pub fn invalid_request() -> Self {
        Self::new(ErrorCode::InvalidRequest)
    }

    /// Creates new `MethodNotFound`
    pub fn method_not_found() -> Self {
        Self::new(ErrorCode::MethodNotFound)
    }

    /// Creates new `InvalidParams`
    pub fn invalid_params<M>(message: M) -> Self
    where
        M: Into<String>,
    {
        Error {
            code: ErrorCode::InvalidParams,
            message: message.into(),
            data: None,
        }
    }

    /// Creates `InvalidParams` for given parameter, with details.
    pub fn invalid_params_with_details<M, T>(message: M, details: T) -> Error
    where
        M: Into<String>,
        T: std::fmt::Debug,
    {
        Error {
            code: ErrorCode::InvalidParams,
            message: format!("Invalid parameters: {}", message.into()),
            data: Some(Value::String(format!("{:?}", details))),
        }
    }

    /// Creates new `InternalError`
    pub fn internal_error() -> Self {
        Self::new(ErrorCode::InternalError)
    }

    /// Creates new `InvalidRequest` with invalid version description
    pub fn invalid_version() -> Self {
        Error {
            code: ErrorCode::InvalidRequest,
            message: "Unsupported JSON-RPC protocol version".to_owned(),
            data: None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.description(), self.message)
    }
}

impl std::error::Error for Error {}
/// Represents output - failure or success
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum Output {
    /// Success
    Success(Success),
    /// Failure
    Failure(Failure),
}

impl Output {
    /// Get the correlation id.
    pub fn id(&self) -> &Id {
        match self {
            Self::Success(ref s) => &s.id,
            Self::Failure(ref f) => &f.id,
        }
    }
}

/// Request parameters
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum Params {
    /// No parameters
    None,
    /// Any single value.
    Value(Value),
    /// Array of values
    Array(Vec<Value>),
    /// Map of values
    Map(serde_json::Map<String, Value>),
}

impl Params {
    /// Parse incoming `Params` into expected types.
    pub fn parse<D>(self) -> Result<D, Error>
    where
        D: DeserializeOwned,
    {
        let value: Value = self.into();
        serde_json::value::from_value(value)
            .map_err(|e| Error::invalid_params(format!("Invalid params: {}.", e)))
    }

    /// Check for no params, returns Err if any params
    pub fn expect_no_params(self) -> Result<(), Error> {
        match self {
            Params::None => Ok(()),
            Params::Array(ref v) if v.is_empty() => Ok(()),
            p => Err(Error::invalid_params_with_details(
                "No parameters were expected",
                p,
            )),
        }
    }
}

impl From<Params> for Value {
    fn from(params: Params) -> Value {
        match params {
            Params::Array(vec) => Value::Array(vec),
            Params::Map(map) => Value::Object(map),
            Params::Value(value) => value,
            Params::None => Value::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, ErrorCode, Params, Value};

    #[test]
    fn params_deserialization() {
        let s = r#"[null, true, -1, 4, 2.3, "hello", [0], {"key": "value"}, []]"#;
        let deserialized: Params = serde_json::from_str(s).unwrap();

        let mut map = serde_json::Map::new();
        map.insert("key".to_string(), Value::String("value".to_string()));

        assert_eq!(
            Params::Array(vec![
                Value::Null,
                Value::Bool(true),
                Value::from(-1),
                Value::from(4),
                Value::from(2.3),
                Value::String("hello".to_string()),
                Value::Array(vec![Value::from(0)]),
                Value::Object(map),
                Value::Array(vec![]),
            ]),
            deserialized
        );
    }

    #[test]
    fn should_return_meaningful_error_when_deserialization_fails() {
        // given
        let s = r#"[1, true]"#;
        let params = || serde_json::from_str::<Params>(s).unwrap();

        // when
        let v1: Result<(Option<u8>, String), Error> = params().parse();
        let v2: Result<(u8, bool, String), Error> = params().parse();
        let err1 = v1.unwrap_err();
        let err2 = v2.unwrap_err();

        // then
        assert_eq!(err1.code, ErrorCode::InvalidParams);
        assert_eq!(
            err1.message,
            "Invalid params: invalid type: boolean `true`, expected a string."
        );
        assert_eq!(err1.data, None);
        assert_eq!(err2.code, ErrorCode::InvalidParams);
        assert_eq!(
            err2.message,
            "Invalid params: invalid length 2, expected a tuple of size 3."
        );
        assert_eq!(err2.data, None);
    }

    #[test]
    fn single_param_parsed_as_tuple() {
        let params: (u64,) = Params::Array(vec![Value::from(1)]).parse().unwrap();
        assert_eq!(params, (1,));
    }
}
