use std::env;
use std::io::{self, BufRead};
use std::process::exit;

use fuzzy_matcher::skim::fuzzy_indices;
use indexmap::IndexMap;
use serde_json::json;

pub fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        println!("Usage: echo <piped_input> | maple <pattern>");
        exit(1);
    }

    let query = &args[1];

    // count the frequency of each letter in a sentence.
    let mut ranked = IndexMap::new();

    for line in io::stdin().lock().lines() {
        if let Ok(line) = line {
            if let Some((score, indices)) = fuzzy_indices(&line, query) {
                ranked.insert(line, (score, indices));
            }
        }
    }

    ranked.sort_by(|_, v1, _, v2| v2.0.cmp(&v1.0));

    for (k, v) in ranked.iter() {
        let matched = json!({
            "text": k,
            "indices":v.1,
        });
        println!("{}", matched.to_string());
    }
}
