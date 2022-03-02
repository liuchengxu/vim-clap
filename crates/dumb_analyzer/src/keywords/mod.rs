mod erlang;
mod golang;
mod rust;
mod viml;

pub use erlang::Erlang;
pub use golang::Golang;
pub use rust::Rust;
pub use viml::Viml;

pub trait KeywordWeight {
    /// Definition/Decleration keywords.
    const DEFINITION: &'static [&'static str];

    /// Dummy reference keywords.
    const REFERENCE: &'static [&'static str];

    /// Keywords for simple & compund statement.
    const STATEMENT: &'static [&'static str];

    fn keyword_weight(token: &str) -> Option<usize> {
        if Self::DEFINITION.contains(&token) {
            Some(4)
        } else if Self::REFERENCE.contains(&token) {
            Some(6)
        } else if Self::STATEMENT.contains(&token) {
            Some(8)
        } else {
            None
        }
    }
}
