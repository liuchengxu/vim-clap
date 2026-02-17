//! Custom StarDict dictionary parser.
//!
//! Parses `.ifo`, `.idx`, and `.dict` (or `.dict.dz`) files from the StarDict format.
//! MIT-licensed implementation — no dependency on the GPL `stardict` crate.

use byteorder::{BigEndian, ReadBytesExt};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Metadata from a `.ifo` file.
pub struct DictInfo {
    pub bookname: String,
    pub wordcount: usize,
    pub sametypesequence: String,
    #[allow(dead_code)]
    pub author: String,
    #[allow(dead_code)]
    pub description: String,
}

/// A single index entry mapping a word to its data location.
struct IndexEntry {
    word: String,
    offset: u64,
    size: u32,
}

/// A parsed definition segment from a dictionary entry.
pub struct DefinitionSegment {
    /// Segment type: 'm'=plain text, 'h'=HTML, 'x'=XDXF, 'g'=Pango, etc.
    pub segment_type: char,
    pub content: String,
}

/// A fully loaded StarDict dictionary (`.ifo` + `.idx` + `.dict`).
pub struct StarDictionary {
    info: DictInfo,
    /// Sorted by word for binary search.
    index: Vec<IndexEntry>,
    /// Entire `.dict` content (decompressed if `.dict.dz`).
    dict_data: Vec<u8>,
}

impl StarDictionary {
    /// Load a StarDict dictionary from a `.ifo` file path.
    pub fn load(ifo_path: &Path) -> Result<Self, String> {
        let info = parse_ifo(ifo_path)?;
        let index = parse_idx(ifo_path, info.wordcount)?;
        let dict_data = load_dict_data(ifo_path)?;

        Ok(Self {
            info,
            index,
            dict_data,
        })
    }

    /// Look up a word, returning definition segments if found.
    ///
    /// Tries case-sensitive binary search first, then case-insensitive fallback.
    pub fn lookup(&self, word: &str) -> Option<Vec<DefinitionSegment>> {
        // Case-sensitive binary search
        if let Ok(idx) = self.index.binary_search_by(|e| e.word.as_str().cmp(word)) {
            return self.read_entry(&self.index[idx]);
        }

        // Case-insensitive fallback (linear scan)
        let word_lower = word.to_lowercase();
        for entry in &self.index {
            if entry.word.to_lowercase() == word_lower {
                return self.read_entry(entry);
            }
        }

        None
    }

    /// Dictionary name from the `.ifo` bookname field.
    pub fn name(&self) -> &str {
        &self.info.bookname
    }

    /// Number of words in the dictionary.
    pub fn word_count(&self) -> usize {
        self.info.wordcount
    }

    /// Read and parse an entry's raw data into definition segments.
    fn read_entry(&self, entry: &IndexEntry) -> Option<Vec<DefinitionSegment>> {
        let offset = entry.offset as usize;
        let size = entry.size as usize;
        if offset.checked_add(size)? > self.dict_data.len() {
            return None;
        }
        let data = &self.dict_data[offset..offset + size];
        Some(parse_segments(data, &self.info.sametypesequence))
    }
}

/// Find all `.ifo` files in a directory (non-recursive).
pub fn scan_dir(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "ifo"))
        .collect()
}

/// Parse a `.ifo` file into `DictInfo`.
fn parse_ifo(ifo_path: &Path) -> Result<DictInfo, String> {
    let content = std::fs::read_to_string(ifo_path)
        .map_err(|error| format!("Failed to read .ifo file: {error}"))?;

    let mut bookname = String::new();
    let mut wordcount: usize = 0;
    let mut sametypesequence = String::new();
    let mut author = String::new();
    let mut description = String::new();

    for line in content.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("bookname=") {
            bookname = value.to_string();
        } else if let Some(value) = line.strip_prefix("wordcount=") {
            wordcount = value
                .parse()
                .map_err(|error| format!("Invalid wordcount: {error}"))?;
        } else if let Some(value) = line.strip_prefix("sametypesequence=") {
            sametypesequence = value.to_string();
        } else if let Some(value) = line.strip_prefix("author=") {
            author = value.to_string();
        } else if let Some(value) = line.strip_prefix("description=") {
            description = value.to_string();
        }
    }

    if bookname.is_empty() {
        return Err("Missing bookname in .ifo file".to_string());
    }

    Ok(DictInfo {
        bookname,
        wordcount,
        sametypesequence,
        author,
        description,
    })
}

/// Parse a `.idx` file into a sorted vector of `IndexEntry`.
fn parse_idx(ifo_path: &Path, expected_count: usize) -> Result<Vec<IndexEntry>, String> {
    let idx_path = ifo_path.with_extension("idx");
    let data =
        std::fs::read(&idx_path).map_err(|error| format!("Failed to read .idx file: {error}"))?;

    let mut entries = Vec::with_capacity(expected_count);
    let mut cursor = 0;

    while cursor < data.len() {
        // Read null-terminated word
        let word_start = cursor;
        while cursor < data.len() && data[cursor] != 0 {
            cursor += 1;
        }
        if cursor >= data.len() {
            break;
        }
        let word = String::from_utf8_lossy(&data[word_start..cursor]).to_string();
        cursor += 1; // skip null terminator

        // Read u32 offset and u32 size (big-endian)
        if cursor + 8 > data.len() {
            break;
        }
        let mut reader = &data[cursor..cursor + 8];
        let offset = reader
            .read_u32::<BigEndian>()
            .map_err(|error| format!("Failed to read offset: {error}"))?
            as u64;
        let size = reader
            .read_u32::<BigEndian>()
            .map_err(|error| format!("Failed to read size: {error}"))?;
        cursor += 8;

        entries.push(IndexEntry { word, offset, size });
    }

    // Index should already be sorted per StarDict spec, but verify
    entries.sort_by(|a, b| a.word.cmp(&b.word));

    Ok(entries)
}

/// Load `.dict` or `.dict.dz` data (decompressing if needed).
fn load_dict_data(ifo_path: &Path) -> Result<Vec<u8>, String> {
    let dict_path = ifo_path.with_extension("dict");
    let dict_dz_path = {
        let mut p = ifo_path.with_extension("dict");
        let mut name = p
            .file_name()
            .expect("ifo_path has a filename")
            .to_os_string();
        name.push(".dz");
        p.set_file_name(name);
        p
    };

    if dict_path.exists() {
        std::fs::read(&dict_path).map_err(|error| format!("Failed to read .dict file: {error}"))
    } else if dict_dz_path.exists() {
        let compressed = std::fs::read(&dict_dz_path)
            .map_err(|error| format!("Failed to read .dict.dz file: {error}"))?;
        let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|error| format!("Failed to decompress .dict.dz: {error}"))?;
        Ok(decompressed)
    } else {
        Err(format!(
            "Neither .dict nor .dict.dz found for {}",
            ifo_path.display()
        ))
    }
}

/// Returns true if `ch` is a lowercase text-type character per the StarDict spec.
fn is_text_type(ch: char) -> bool {
    matches!(ch, 'm' | 'g' | 't' | 'x' | 'y' | 'k' | 'w' | 'h')
}

/// Parse raw entry data into definition segments.
///
/// Two modes:
/// - Mode A: `sametypesequence` is set in `.ifo` — type bytes not in data.
/// - Mode B: no `sametypesequence` — each segment starts with a type byte.
fn parse_segments(data: &[u8], sametypesequence: &str) -> Vec<DefinitionSegment> {
    if sametypesequence.is_empty() {
        parse_segments_mode_b(data)
    } else {
        parse_segments_mode_a(data, sametypesequence)
    }
}

/// Mode A: types declared in `.ifo`, not present in data.
fn parse_segments_mode_a(data: &[u8], sequence: &str) -> Vec<DefinitionSegment> {
    let mut segments = Vec::new();
    let mut cursor = 0;
    let type_chars: Vec<char> = sequence.chars().collect();

    for (i, &type_char) in type_chars.iter().enumerate() {
        if cursor >= data.len() {
            break;
        }
        let is_last = i == type_chars.len() - 1;

        if is_text_type(type_char) {
            // Text type: null-terminated, but last segment may omit trailing null
            let content = if is_last {
                // Last text segment: read to end
                let content = String::from_utf8_lossy(&data[cursor..]).to_string();
                cursor = data.len();
                content
            } else {
                // Find null terminator
                let end = data[cursor..]
                    .iter()
                    .position(|&b| b == 0)
                    .map(|p| cursor + p)
                    .unwrap_or(data.len());
                let content = String::from_utf8_lossy(&data[cursor..end]).to_string();
                cursor = if end < data.len() { end + 1 } else { end };
                content
            };
            segments.push(DefinitionSegment {
                segment_type: type_char,
                content,
            });
        } else {
            // Binary type (uppercase): u32 size prefix
            if cursor + 4 > data.len() {
                break;
            }
            let payload_size = u32::from_be_bytes([
                data[cursor],
                data[cursor + 1],
                data[cursor + 2],
                data[cursor + 3],
            ]) as usize;
            cursor += 4;
            // Skip binary content (WAV, PNG, etc.)
            cursor += payload_size.min(data.len().saturating_sub(cursor));
        }
    }

    segments
}

/// Mode B: each segment starts with a type byte.
fn parse_segments_mode_b(data: &[u8]) -> Vec<DefinitionSegment> {
    let mut segments = Vec::new();
    let mut cursor = 0;

    while cursor < data.len() {
        let type_byte = data[cursor] as char;
        cursor += 1;

        if is_text_type(type_byte) {
            // Text type: find null terminator or read to end
            let end = data[cursor..]
                .iter()
                .position(|&b| b == 0)
                .map(|p| cursor + p);
            let (content, next_cursor) = match end {
                Some(end_pos) => {
                    let content = String::from_utf8_lossy(&data[cursor..end_pos]).to_string();
                    (content, end_pos + 1)
                }
                None => {
                    // Last segment, no null — read to end
                    let content = String::from_utf8_lossy(&data[cursor..]).to_string();
                    (content, data.len())
                }
            };
            segments.push(DefinitionSegment {
                segment_type: type_byte,
                content,
            });
            cursor = next_cursor;
        } else {
            // Binary type (uppercase): u32 size prefix
            if cursor + 4 > data.len() {
                break;
            }
            let payload_size = u32::from_be_bytes([
                data[cursor],
                data[cursor + 1],
                data[cursor + 2],
                data[cursor + 3],
            ]) as usize;
            cursor += 4;
            // Skip binary content
            cursor += payload_size.min(data.len().saturating_sub(cursor));
        }
    }

    segments
}

/// Convert definition segments to HTML for display.
pub fn segments_to_html(segments: &[DefinitionSegment]) -> String {
    let mut html = String::new();
    for segment in segments {
        match segment.segment_type {
            'm' | 't' => {
                // Plain text: HTML-escape, convert newlines to <br>
                let escaped = html_escape(&segment.content);
                html.push_str("<p>");
                html.push_str(&escaped.replace('\n', "<br>"));
                html.push_str("</p>");
            }
            'h' => {
                // HTML: pass through as-is (frontend sanitizes with DOMPurify)
                html.push_str(&segment.content);
            }
            'x' => {
                // XDXF: basic XML tag conversion to HTML
                html.push_str(&xdxf_to_html(&segment.content));
            }
            'g' => {
                // Pango markup: strip tags, HTML-escape
                let stripped = strip_pango_tags(&segment.content);
                let escaped = html_escape(&stripped);
                html.push_str("<p>");
                html.push_str(&escaped);
                html.push_str("</p>");
            }
            _ => {
                // Other text types: HTML-escape and wrap in <pre>
                let escaped = html_escape(&segment.content);
                html.push_str("<pre>");
                html.push_str(&escaped);
                html.push_str("</pre>");
            }
        }
    }
    html
}

/// HTML-escape a string.
fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Convert XDXF markup to basic HTML.
fn xdxf_to_html(xdxf: &str) -> String {
    xdxf.replace("<kref>", "<a>")
        .replace("</kref>", "</a>")
        .replace("<k>", "<b>")
        .replace("</k>", "</b>")
        .replace("<ex>", "<i>")
        .replace("</ex>", "</i>")
        .replace("<dtrn>", "<span>")
        .replace("</dtrn>", "</span>")
        .replace("<abr>", "<em>")
        .replace("</abr>", "</em>")
        .replace("<gr>", "<span class=\"grammar\">")
        .replace("</gr>", "</span>")
        .replace("<tr>", "<span class=\"transcription\">[")
        .replace("</tr>", "]</span>")
}

/// Strip Pango markup tags, keeping only text content.
fn strip_pango_tags(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_segments_mode_b_single_text() {
        // Type 'm', then "hello\0"
        let data = b"mhello\0";
        let segments = parse_segments_mode_b(data);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].segment_type, 'm');
        assert_eq!(segments[0].content, "hello");
    }

    #[test]
    fn test_parse_segments_mode_b_last_no_null() {
        // Type 'm', then "hello" (no trailing null — last segment)
        let data = b"mhello";
        let segments = parse_segments_mode_b(data);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].segment_type, 'm');
        assert_eq!(segments[0].content, "hello");
    }

    #[test]
    fn test_parse_segments_mode_a_single() {
        let data = b"hello world";
        let segments = parse_segments_mode_a(data, "m");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].segment_type, 'm');
        assert_eq!(segments[0].content, "hello world");
    }

    #[test]
    fn test_parse_segments_mode_a_multi() {
        // "m" segment null-terminated, then "h" segment to end
        let data = b"plain text\0<b>html</b>";
        let segments = parse_segments_mode_a(data, "mh");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].segment_type, 'm');
        assert_eq!(segments[0].content, "plain text");
        assert_eq!(segments[1].segment_type, 'h');
        assert_eq!(segments[1].content, "<b>html</b>");
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<b>test</b>"), "&lt;b&gt;test&lt;/b&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
    }

    #[test]
    fn test_segments_to_html_plain() {
        let segments = vec![DefinitionSegment {
            segment_type: 'm',
            content: "Hello\nWorld".to_string(),
        }];
        let html = segments_to_html(&segments);
        assert_eq!(html, "<p>Hello<br>World</p>");
    }

    #[test]
    fn test_segments_to_html_html_passthrough() {
        let segments = vec![DefinitionSegment {
            segment_type: 'h',
            content: "<b>bold</b>".to_string(),
        }];
        let html = segments_to_html(&segments);
        assert_eq!(html, "<b>bold</b>");
    }

    #[test]
    fn test_strip_pango_tags() {
        assert_eq!(
            strip_pango_tags("<b>bold</b> and <i>italic</i>"),
            "bold and italic"
        );
    }

    #[test]
    fn test_is_text_type() {
        assert!(is_text_type('m'));
        assert!(is_text_type('h'));
        assert!(is_text_type('x'));
        assert!(!is_text_type('W'));
        assert!(!is_text_type('P'));
    }
}
