use serde::{Deserialize, Serialize};
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
    pub fn session_id(&self) -> u64 {
        match self {
            Self::MethodCall(method_call) => method_call.session_id,
            Self::Notification(notification) => notification.session_id,
        }
    }

    pub fn unwrap_method_call(self) -> MethodCall {
        match self {
            Self::MethodCall(method_call) => method_call,
            _ => unreachable!("Unwrapping MethodCall but met Notification"),
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

/// Error object as defined in Spec
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Error {
    /// Code
    pub code: jsonrpc_core::ErrorCode,
    /// Message
    pub message: String,
    /// Optional data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

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
