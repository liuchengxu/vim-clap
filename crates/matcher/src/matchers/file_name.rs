use super::MatchItem;
use pattern::file_name_only;

#[derive(Clone, Debug)]
pub struct FileNameMatcher<'a>(&'a str);

impl<'a> MatchItem<'a> for FileNameMatcher<'a> {
    fn display_text(&self) -> &'a str {
        self.0
    }

    fn match_text(&self) -> Option<(&'a str, usize)> {
        file_name_only(self.0)
    }
}

impl<'a> From<&'a str> for FileNameMatcher<'a> {
    fn from(inner: &'a str) -> Self {
        Self(inner)
    }
}
