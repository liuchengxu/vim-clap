mod file_name;
mod grep;
mod tag_name;

pub use self::file_name::FileNameMatcher;
pub use self::grep::GrepMatcher;
pub use self::tag_name::TagNameMatcher;

pub trait MatchItem<'a> {
    /// Returns the text for displaying.
    fn display(&self) -> &'a str;

    // Currently we only take care of matching one piece.
    /// Returns the text for matching and the offset (in byte) of it begins.
    fn match_text(&self) -> Option<(&'a str, usize)>;
}
