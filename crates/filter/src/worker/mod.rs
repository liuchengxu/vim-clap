use std::sync::Arc;

use matcher::{ClapItem, MatchScope, Matcher};
use types::{FileNameItem, GrepItem, SourceItem};

pub mod iterator;
pub mod par_iterator;

/// Converts the raw line into a clap item.
pub(crate) fn try_into_clap_item(matcher: &Matcher, line: String) -> Option<Arc<dyn ClapItem>> {
    match matcher.match_scope() {
        MatchScope::GrepLine => {
            GrepItem::try_new(line).map(|item| Arc::new(item) as Arc<dyn ClapItem>)
        }
        MatchScope::FileName => {
            FileNameItem::try_new(line).map(|item| Arc::new(item) as Arc<dyn ClapItem>)
        }
        _ => Some(Arc::new(SourceItem::from(line))),
    }
}
