pub mod v0;
pub mod v1;

pub struct AsciiDots;

impl AsciiDots {
    pub const DOTS: &'static str = "..";
    pub const CHAR_LEN: usize = 2;
    pub const BYTE_LEN: usize = 2;
}
