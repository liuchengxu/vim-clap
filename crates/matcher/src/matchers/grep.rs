use super::MatchItem;
use pattern::strip_grep_filepath;

#[derive(Clone, Debug)]
pub struct GrepMatcher<'a>(&'a str);

impl<'a> MatchItem<'a> for GrepMatcher<'a> {
    fn display_text(&self) -> &'a str {
        self.0
    }

    fn match_text(&self) -> Option<(&'a str, usize)> {
        strip_grep_filepath(self.0)
    }
}

impl<'a> From<&'a str> for GrepMatcher<'a> {
    fn from(inner: &'a str) -> Self {
        Self(inner)
    }
}
