use super::MatchItem;
use pattern::tag_name_only;

#[derive(Clone, Debug)]
pub struct TagNameMatcher<'a>(&'a str);

impl<'a> MatchItem<'a> for TagNameMatcher<'a> {
    fn display_text(&self) -> &'a str {
        self.0
    }

    fn match_text(&self) -> Option<(&'a str, usize)> {
        tag_name_only(self.0).map(|s| (s, 0))
    }
}

impl<'a> From<&'a str> for TagNameMatcher<'a> {
    fn from(inner: &'a str) -> Self {
        Self(inner)
    }
}
