/// The formats we can parse from and export to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Format {
    /// The Markdown format. This requires the use of `#`-based headings.
    #[cfg_attr(feature = "serde", serde(alias = "md"))]
    #[cfg_attr(feature = "serde", serde(rename = "markdown"))]
    Markdown,
    /// The Org mode format
    #[cfg_attr(feature = "serde", serde(rename = "org"))]
    Org,
}
impl Format {
    /// Gets the character which is repeated to define headings in this format.
    pub(crate) fn heading_char(&self) -> char {
        match &self {
            Self::Markdown => '#',
            Self::Org => '*',
        }
    }
    /// Gets the string used to open property drawers in this format.
    pub(crate) fn get_properties_opener(&self) -> &'static str {
        match &self {
            Self::Markdown => "<!--PROPERTIES",
            Self::Org => ":PROPERTIES:",
        }
    }
    /// Gets the string used to close property drawers in this format.
    pub(crate) fn get_properties_closer(&self) -> &'static str {
        match &self {
            Self::Markdown => "-->",
            Self::Org => ":END:",
        }
    }
}
