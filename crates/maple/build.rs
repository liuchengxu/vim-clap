use std::io::Write;
use std::{env, fs};

fn main() {
    built::write_built_file()
        .unwrap_or_else(|e| panic!("Failed to acquire build-time information: {e:?}"));

    let outdir = env::var("OUT_DIR").unwrap();
    let outfile = format!("{outdir}/compiled_at.txt");

    let mut fh = fs::File::create(outfile).expect("Failed to create compiled_at.txt");
    write!(fh, r#""{}""#, chrono::Local::now()).ok();
}
