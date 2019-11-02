use std::env;
use std::io::{self, BufRead};
use std::process::exit;

use fuzzy_matcher::skim::fuzzy_indices;
use serde_json::json;

// TODO rank?
fn skim(query: &str, line: &str) {
    if let Some((_score, indices)) = fuzzy_indices(line, query) {
        let matched = json!({
            "text": line,
            "indices": indices
        });
        println!("{}", matched.to_string());
    }
}

pub fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        println!("Usage: echo <piped_input> | maple <pattern>");
        exit(1);
    }

    let query = &args[1];

    for line in io::stdin().lock().lines() {
        if let Ok(line) = line {
            skim(query, &line);
        }
    }
}
