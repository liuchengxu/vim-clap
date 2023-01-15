/// Combine json and println macro.
#[macro_export]
macro_rules! println_json {
  ( $( $field:expr ),+ ) => {
    {
      println!("{}", serde_json::json!({ $(stringify!($field): $field,)* }))
    }
  }
}

/// Combine json and println macro.
///
/// Neovim needs Content-length info when using stdio-based communication.
#[macro_export]
macro_rules! println_json_with_length {
  ( $( $field:expr ),+ ) => {
    {
      let msg = serde_json::json!({ $(stringify!($field): $field,)* });
      if let Ok(s) = serde_json::to_string(&msg) {
          println!("Content-length: {}\n\n{}", s.len(), s);
      }
    }
  }
}
