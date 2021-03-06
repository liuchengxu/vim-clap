use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fs::{read_to_string, File};
use std::io::{LineWriter, Write};
use std::path::Path;

use itertools::Itertools;

fn build_raw_line<S: AsRef<OsStr> + ?Sized>(p: &S, const_name: &str) -> String {
    let json_file_path = Path::new(p);
    let json_file_str = read_to_string(json_file_path).expect("file not found");
    let exactmatch_map: HashMap<String, char> =
        serde_json::from_str(&json_file_str).expect("error while reading json");

    let sorted = exactmatch_map
        .keys()
        .sorted()
        .map(|k| format!("(\"{}\", '{}')", k, exactmatch_map[k]))
        .join(",");

    format!("pub const {}: &[(&str, char)] = &[{}];", const_name, sorted)
}

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("constants.rs");

    let file = File::create(dest_path).expect("can not create file");
    let mut file = LineWriter::new(file);

    let current_dir = std::env::current_dir().unwrap();

    let path_for = |filename: &str| {
        let mut icon_path = current_dir.clone();
        icon_path.push(filename);
        icon_path
    };

    let line = build_raw_line(&path_for("exactmatch_map.json"), "EXACTMATCH_ICON_TABLE");
    file.write_all(format!("{}\n", line).as_bytes()).unwrap();

    let line = build_raw_line(&path_for("extension_map.json"), "EXTENSION_ICON_TABLE");
    file.write_all(format!("\n{}\n", line).as_bytes()).unwrap();

    let line = build_raw_line(&path_for("tagkind_map.json"), "TAGKIND_ICON_TABLE");
    file.write_all(format!("\n{}\n", line).as_bytes()).unwrap();

    file.write_all(
        "
pub fn bsearch_icon_table(c: &str, table: &[(&str, char)]) ->Option<usize> {
    table.binary_search_by(|&(key, _)| key.cmp(&c)).ok()
}
\n"
        .as_bytes(),
    )
    .unwrap();

    println!("cargo:rerun-if-changed=build.rs");
}
