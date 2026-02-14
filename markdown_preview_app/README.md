# Markdown Preview App

Standalone Tauri application for markdown preview.

## Project Structure

```
markdown_preview_app/
├── src/              # Rust backend code
├── frontend/         # GENERATED - do not edit directly!
└── build.rs          # Generates frontend/ from maple_markdown/js/

../maple_markdown/js/
├── index.html        # HTML template (source)
├── styles.css        # CSS styles (source)
├── themes.css        # Theme styles (source)
├── core.js           # Shared UI functionality (source)
└── tauri-app.js      # Tauri-specific code (source)
```

## Important: Frontend Source Files

The `frontend/` directory is **generated** during build from `maple_markdown/js/` sources.

**Do NOT edit files in `frontend/`** - changes will be lost on rebuild.

To modify the UI:
- CSS styles: Edit `../maple_markdown/js/styles.css`
- Theme styles: Edit `../maple_markdown/js/themes.css`
- HTML structure: Edit `../maple_markdown/js/index.html`
- JavaScript: Edit `../maple_markdown/js/core.js` or `tauri-app.js`

## Building

```bash
cargo build --release
```

The build process (in `build.rs`) will:
1. Read source files from `maple_markdown/js/`
2. Inline CSS and JS into the HTML template
3. Write the generated `frontend/index.html`
