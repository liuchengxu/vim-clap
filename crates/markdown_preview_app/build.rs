use std::fs;
use std::path::Path;

fn main() {
    // Generate the HTML file with inlined CSS and JS for Tauri
    generate_frontend_html();

    tauri_build::build()
}

/// Generate index.html with inlined CSS and JS from maple_markdown sources.
///
/// The JS is split into:
/// - core.js: Shared UI functionality (TOC, themes, fuzzy finder, etc.)
/// - tauri-app.js: Tauri-specific IPC and initialization
fn generate_frontend_html() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let base_path = Path::new(&manifest_dir).parent().unwrap();

    // Source files from maple_markdown/js
    let js_dir = base_path.join("maple_markdown/js");
    let html_template =
        fs::read_to_string(js_dir.join("index.html")).expect("Failed to read index.html template");
    let styles_css =
        fs::read_to_string(js_dir.join("styles.css")).expect("Failed to read styles.css");
    let themes_css =
        fs::read_to_string(js_dir.join("themes.css")).expect("Failed to read themes.css");

    // Load modular JS files
    let core_js = fs::read_to_string(js_dir.join("core.js")).expect("Failed to read core.js");
    let tauri_app_js =
        fs::read_to_string(js_dir.join("tauri-app.js")).expect("Failed to read tauri-app.js");

    // Combine core + tauri-app for standalone app
    let combined_js = format!("{core_js}\n\n{tauri_app_js}");

    // Replace placeholders
    let mut html = html_template;
    html = html.replace("/*__STYLES_CSS__*/", &styles_css);
    html = html.replace("/*__THEMES_CSS__*/", &themes_css);
    html = html.replace("/*__APP_JS__*/", &combined_js);

    // Create frontend directory for Tauri
    let frontend_dir = Path::new(&manifest_dir).join("frontend");
    fs::create_dir_all(&frontend_dir).expect("Failed to create frontend directory");

    // Write the generated HTML
    let output_path = frontend_dir.join("index.html");
    fs::write(&output_path, html).expect("Failed to write generated index.html");

    // Tell Cargo to rerun if source files change
    println!("cargo:rerun-if-changed=../maple_markdown/js/index.html");
    println!("cargo:rerun-if-changed=../maple_markdown/js/styles.css");
    println!("cargo:rerun-if-changed=../maple_markdown/js/themes.css");
    println!("cargo:rerun-if-changed=../maple_markdown/js/core.js");
    println!("cargo:rerun-if-changed=../maple_markdown/js/tauri-app.js");
}
