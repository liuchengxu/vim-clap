use std::collections::HashMap;
use std::path::Path;

use lazy_static::lazy_static;

pub const DEFAULT_ICON: &'static str = "";
pub const DEFAULT_ICONIZED: &'static str = " ";

#[inline]
fn icon_for(line: &str) -> &str {
    let path = Path::new(line);
    path.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(|ext| {
            let ext: &str = &ext.to_lowercase();
            EXACTMATCH_MAP.get(ext)
        })
        .unwrap_or(
            path.extension()
                .and_then(std::ffi::OsStr::to_str)
                .and_then(|ext| EXTENSION_MAP.get(ext))
                .unwrap_or(&DEFAULT_ICON),
        )
}

pub fn prepend_icon(line: &str) -> String {
    format!("{} {}", icon_for(line), line)
}

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
        m.insert("md", "");
        m.insert("markdown", "");
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
        m.insert("bin", "");
        m.insert("timestamp", "﨟");
        m
    };
}
