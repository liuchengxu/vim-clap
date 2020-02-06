use std::collections::HashMap;
use std::path::Path;

use lazy_static::lazy_static;
use regex::Regex;

pub const DEFAULT_ICON: &str = "";
#[allow(dead_code)]
pub const DEFAULT_ICONIZED: &str = " ";
pub const FOLDER_ICON: &str = "";
pub const DEFAULT_FILER_ICON: &str = "";

lazy_static! {
    pub static ref EXACTMATCH_MAP: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("gruntfile.js", "");
        m.insert("gruntfile.ls", "");
        m.insert("gruntfile.coffee", "");
        m.insert("gulpfile.js", "");
        m.insert("gulpfile.ls", "");
        m.insert("gulpfile.coffee", "");
        m.insert("mix.lock", "");
        m.insert("dropbox", "");
        m.insert("go.mod", "");
        m.insert("go.sum", "");
        m.insert("readme", "");
        m.insert("gemfile", "");
        m.insert(".ds_store", "");
        m.insert(".gitignore", "");
        m.insert(".gitconfig", "");
        m.insert(".editorconfig", "");
        m.insert(".gitlab-ci.yml", "");
        m.insert(".zshrc", "");
        m.insert(".bashrc", "");
        m.insert("makefile", "");
        m.insert(".vimrc", "");
        m.insert("_vimrc", "");
        m.insert(".gvimrc", "");
        m.insert("_gvimrc", "");
        m.insert("favicon.ico", "");
        m.insert(".bashprofile", "");
        m.insert("license", "");
        m.insert("react.jsx", "");
        m.insert("node_modules", "");
        m.insert("procfile", "");
        m.insert("rust-toolchain", "");
        m.insert("dockerfile", "");
        m.insert("docker-compose.yml", "");
        m
    };
    pub static ref EXTENSION_MAP: HashMap<&'static str, &'static str> = {
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
        m.insert("md", "");
        m.insert("markdown", "");
        m.insert("txt", "");
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
        m.insert("cfg", "");
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
        m.insert("rmeta", "");
        m.insert("d", "");
        m.insert("erl", "");
        m.insert("hrl", "");
        m.insert("ex", "");
        m.insert("exs", "");
        m.insert("eex", "");
        m.insert("vim", "");
        m.insert("vimrc", "");
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
        m.insert("lock", "");
        m.insert("log", "");
        m.insert("plist", "况");
        m.insert("bin", "");
        m.insert("dylib", "");
        m.insert("so", "");
        m.insert("timestamp", "﨟");
        m.insert("gz", "");
        m.insert("zip", "");
        m
    };
}

#[inline]
pub fn icon_for_path(path: &Path) -> &str {
    path.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(|ext| {
            let ext: &str = &ext.to_lowercase();
            EXACTMATCH_MAP.get(ext)
        })
        .unwrap_or_else(|| {
            path.extension()
                .and_then(std::ffi::OsStr::to_str)
                .and_then(|ext| EXTENSION_MAP.get(ext))
                .unwrap_or(&DEFAULT_ICON)
        })
}

fn icon_for(line: &str) -> &str {
    let path = Path::new(line);
    icon_for_path(&path)
}

pub fn prepend_icon(line: &str) -> String {
    format!("{} {}", icon_for(line), line)
}

#[inline]
pub fn icon_for_filer(path: &Path) -> &str {
    if path.is_dir() {
        FOLDER_ICON
    } else {
        path.file_name()
            .and_then(std::ffi::OsStr::to_str)
            .and_then(|ext| {
                let ext: &str = &ext.to_lowercase();
                EXACTMATCH_MAP.get(ext)
            })
            .unwrap_or_else(|| {
                path.extension()
                    .and_then(std::ffi::OsStr::to_str)
                    .and_then(|ext| EXTENSION_MAP.get(ext))
                    .unwrap_or(&DEFAULT_FILER_ICON)
            })
    }
}

pub fn prepend_filer_icon(path: &Path, line: &str) -> String {
    format!("{} {}", icon_for_filer(path), line)
}

#[allow(dead_code)]
pub fn prepend_grep_icon(line: &str) -> String {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^(.*):\d+:\d+:").unwrap();
    }
    let icon = RE
        .captures(line)
        .and_then(|cap| cap.get(1))
        .map(|m| icon_for(m.as_str()))
        .unwrap_or(DEFAULT_ICON);
    format!("{} {}", icon, line)
}
