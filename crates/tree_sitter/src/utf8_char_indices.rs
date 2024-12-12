/// Efficient version of `String::from_utf8_lossy(input).char_indices()` without allocation.
pub struct UncheckedUtf8CharIndices<'a> {
    input: &'a [u8],
    byte_index: usize,
}

impl<'a> UncheckedUtf8CharIndices<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            byte_index: 0,
        }
    }
}

impl Iterator for UncheckedUtf8CharIndices<'_> {
    type Item = (usize, char);

    fn next(&mut self) -> Option<Self::Item> {
        if self.byte_index >= self.input.len() {
            return None;
        }

        let start_index = self.byte_index;
        let s = unsafe { std::str::from_utf8_unchecked(&self.input[start_index..]) };
        if let Some(ch) = s.chars().next() {
            let char_width = ch.len_utf8();
            self.byte_index += char_width;
            Some((start_index, ch))
        } else {
            // Empty string case, handle as needed
            None
        }
    }
}

/// Equivalent of `String::from_utf8_lossy(input).char_indices()`.
pub struct Utf8CharIndices<'a> {
    input: &'a [u8],
    byte_index: usize,
}

impl<'a> Utf8CharIndices<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            byte_index: 0,
        }
    }
}

impl Iterator for Utf8CharIndices<'_> {
    type Item = (usize, char);

    fn next(&mut self) -> Option<Self::Item> {
        if self.byte_index >= self.input.len() {
            return None;
        }

        let start_index = self.byte_index;
        match std::str::from_utf8(&self.input[start_index..]) {
            Ok(s) => {
                if let Some(ch) = s.chars().next() {
                    let char_width = ch.len_utf8();
                    self.byte_index += char_width;
                    Some((start_index, ch))
                } else {
                    // Empty string case, handle as needed
                    None
                }
            }
            Err(_) => {
                // Handle the error case (invalid UTF-8) as needed
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf8_char_indices() {
        let input = "你好，世界！\nHello, World!".as_bytes();

        assert_eq!(
            String::from_utf8_lossy(input)
                .char_indices()
                .collect::<Vec<_>>(),
            Utf8CharIndices::new(input).collect::<Vec<_>>(),
        );

        assert_eq!(
            String::from_utf8_lossy(input)
                .char_indices()
                .collect::<Vec<_>>(),
            UncheckedUtf8CharIndices::new(input).collect::<Vec<_>>(),
        );
    }
}
