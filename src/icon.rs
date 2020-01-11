use std::collections::HashMap;
use std::path::Path;

use lazy_static::lazy_static;
use regex::Regex;

pub const DEFAULT_ICON: &str = "";

lazy_static! {
    pub static ref ICONMAP: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("styl", "");
        m.insert("sass", "");
        m.insert("scss", "");
        m.insert("htm", "");
        m.insert("html", "");
        m.insert("slim", "");
        m.insert("ejs", "");
        m.insert("css", "");
        m.insert("less", "");
        m.insert("md", "");
        m.insert("markdown", "");
        m.insert("rmd", "");
        m.insert("json", "");
        m.insert("js", "");
        m.insert("jsx", "");
        m.insert("rb", "");
        m.insert("php", "");
        m.insert("py", "");
        m.insert("pyc", "");
        m.insert("pyo", "");
        m.insert("pyd", "");
        m.insert("coffee", "");
        m.insert("mustache", "");
        m.insert("hbs", "");
        m.insert("conf", "");
        m.insert("ini", "");
        m.insert("yml", "");
        m.insert("yaml", "");
        m.insert("toml", "");
        m.insert("bat", "");
        m.insert("jpg", "");
        m.insert("jpeg", "");
        m.insert("bmp", "");
        m.insert("png", "");
        m.insert("gif", "");
        m.insert("ico", "");
        m.insert("twig", "");
        m.insert("cpp", "");
        m.insert("cc", "");
        m.insert("cp", "");
        m.insert("c", "");
        m.insert("h", "");
        m.insert("hpp", "");
        m.insert("hs", "");
        m.insert("lhs", "");
        m.insert("lua", "");
        m.insert("java", "");
        m.insert("sh", "");
        m.insert("fish", "");
        m.insert("bash", "");
        m.insert("zsh", "");
        m.insert("ksh", "");
        m.insert("csh", "");
        m.insert("awk", "");
        m.insert("ps1", "");
        m.insert("ml", "λ");
        m.insert("mli", "λ");
        m.insert("diff", "");
        m.insert("db", "");
        m.insert("sql", "");
        m.insert("dump", "");
        m.insert("clj", "");
        m.insert("cljc", "");
        m.insert("cljs", "");
        m.insert("edn", "");
        m.insert("scala", "");
        m.insert("go", "");
        m.insert("dart", "");
        m.insert("xul", "");
        m.insert("sln", "");
        m.insert("suo", "");
        m.insert("pl", "");
        m.insert("pm", "");
        m.insert("t", "");
        m.insert("rss", "");
        m.insert("fsx", "");
        m.insert("fs", "");
        m.insert("fsi", "");
        m.insert("rs", "");
        m.insert("rlib", "");
        m.insert("d", "");
        m.insert("erl", "");
        m.insert("hrl", "");
        m.insert("ex", "");
        m.insert("exs", "");
        m.insert("eex", "");
        m.insert("vim", "");
        m.insert("ai", "");
        m.insert("psd", "");
        m.insert("psb", "");
        m.insert("ts", "");
        m.insert("tsx", "");
        m.insert("jl", "");
        m.insert("pp", "");
        m.insert("vue", "﵂");
        m.insert("swift", "");
        m.insert("xcplayground", "");
        m
    };
}

#[inline]
fn icon_for(path: &str) -> &str {
    Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(|ext| ICONMAP.get(ext))
        .unwrap_or(&DEFAULT_ICON)
}

pub fn prepend_icon(path: &str) -> String {
    format!("{} {}", icon_for(path), path)
}

pub fn prepend_grep_icon(line: &str) -> String {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^(.*):\d+:\d+:").unwrap();
    }
    let icon = RE
        .captures(line)
        .and_then(|cap| cap.get(1).map(|m| icon_for(m.as_str())))
        .unwrap_or(DEFAULT_ICON);
    format!("{} {}", icon, line)
}
