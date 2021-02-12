fn main() {
    built::write_built_file()
        .unwrap_or_else(|e| panic!("Failed to acquire build-time information: {:?}", e));
}
