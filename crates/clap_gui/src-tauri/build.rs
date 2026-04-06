fn main() {
    // Rerun build script when icons change so they get re-embedded
    println!("cargo:rerun-if-changed=icons/");
    tauri_build::build()
}
