/// Combine json and println macro.
macro_rules! println_json {
  ( $( $field:expr ),+ ) => {
    {
      println!("{}", serde_json::json!({ $(stringify!($field): $field,)* }))
    }
  }
}

pub mod cmd;
pub use {anyhow::Result, fuzzy_filter::Source, structopt::StructOpt};

mod error;
mod light_command;
