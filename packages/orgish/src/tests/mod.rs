mod documents;
mod headings;
mod markdown_documents;
mod timestamps;

pub use super::*;

#[derive(Debug, PartialEq)]
pub enum CustomKeyword {
    Todo,
    Proj,
    Other(String),
}
impl Keyword for CustomKeyword {
    fn from_str(keyword: &str) -> Option<Self> {
        match keyword {
            "TODO" => Some(Self::Todo),
            "PROJ" => Some(Self::Proj),
            _ => None,
        }
    }
    fn into_string(self) -> String {
        match self {
            Self::Todo => "TODO".to_string(),
            Self::Proj => "PROJ".to_string(),
            Self::Other(s) => s,
        }
    }
    fn other(keyword: String) -> Self {
        Self::Other(keyword)
    }
}
