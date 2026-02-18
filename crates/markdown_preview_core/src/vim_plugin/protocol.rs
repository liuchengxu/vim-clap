//! Protocol message types for vim-plugin client-server communication.
//!
//! Defines the messages exchanged between the preview server and clients
//! via WebSocket.

use crate::stats::DocumentStats;
use serde::{Deserialize, Serialize};

/// Messages sent from the server to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Update the displayed content with new HTML.
    UpdateContent {
        /// The rendered HTML content
        data: String,
        /// Number of lines in the source file
        #[serde(skip_serializing_if = "Option::is_none")]
        source_lines: Option<usize>,
        /// Mapping from rendered element index to source line number
        #[serde(skip_serializing_if = "Option::is_none")]
        line_map: Option<Vec<usize>>,
        /// Path to the current file
        #[serde(skip_serializing_if = "Option::is_none")]
        file_path: Option<String>,
        /// Git repository root (if file is in a git repo)
        #[serde(skip_serializing_if = "Option::is_none")]
        git_root: Option<String>,
        /// Whether to focus the browser window
        #[serde(default)]
        should_focus: bool,
        /// Document statistics
        #[serde(skip_serializing_if = "Option::is_none")]
        stats: Option<DocumentStats>,
    },

    /// Scroll to a specific position (percentage).
    Scroll {
        /// Scroll position as percentage (0-100)
        data: usize,
    },

    /// Request the browser window to focus itself.
    FocusWindow,
}

/// Messages sent from clients to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Request to switch to a different file.
    SwitchFile {
        /// Path to the file to switch to
        file_path: String,
    },

    /// Request a refresh of the current content.
    RequestRefresh,
}

/// Internal message type used by the server for processing.
///
/// This is the message type that gets passed through channels
/// between different parts of the server.
#[derive(Debug, Clone)]
pub enum Message {
    /// Markdown file was modified.
    /// The boolean flag indicates whether to focus the browser window.
    FileChanged(String, bool),
    /// Refresh the page with given HTML content.
    UpdateContent(String),
    /// Scroll to the given position specified as a percentage.
    Scroll(usize),
    /// Request the browser window to focus itself.
    FocusWindow,
}

impl Message {
    /// Convert the internal message to a server message for transmission.
    ///
    /// # Arguments
    ///
    /// * `process_file` - A function that processes a file path and returns
    ///   the rendered HTML, line map, stats, and git root.
    pub fn to_server_message<F>(
        self,
        process_file: F,
    ) -> Result<ServerMessage, Box<dyn std::error::Error + Send + Sync>>
    where
        F: FnOnce(
            &str,
        ) -> Result<
            (String, Vec<usize>, DocumentStats, Option<String>),
            Box<dyn std::error::Error + Send + Sync>,
        >,
    {
        match self {
            Message::FileChanged(path, should_focus) => {
                let (html, line_map, stats, git_root) = process_file(&path)?;
                Ok(ServerMessage::UpdateContent {
                    data: html,
                    source_lines: Some(stats.lines),
                    line_map: Some(line_map),
                    file_path: Some(path),
                    git_root,
                    should_focus,
                    stats: Some(stats),
                })
            }
            Message::UpdateContent(content) => Ok(ServerMessage::UpdateContent {
                data: content,
                source_lines: None,
                line_map: None,
                file_path: None,
                git_root: None,
                should_focus: false,
                stats: None,
            }),
            Message::Scroll(position) => Ok(ServerMessage::Scroll { data: position }),
            Message::FocusWindow => Ok(ServerMessage::FocusWindow),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_message_serialization() {
        let msg = ServerMessage::UpdateContent {
            data: "<p>Hello</p>".to_string(),
            source_lines: Some(10),
            line_map: Some(vec![1, 3, 5]),
            file_path: Some("/path/to/file.md".to_string()),
            git_root: None,
            should_focus: false,
            stats: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("update_content"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_client_message_deserialization() {
        let json = r#"{"type": "switch_file", "file_path": "/path/to/file.md"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();

        match msg {
            ClientMessage::SwitchFile { file_path } => {
                assert_eq!(file_path, "/path/to/file.md");
            }
            _ => panic!("Expected SwitchFile message"),
        }
    }

    #[test]
    fn test_scroll_message() {
        let msg = ServerMessage::Scroll { data: 50 };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("scroll"));
        assert!(json.contains("50"));
    }
}
