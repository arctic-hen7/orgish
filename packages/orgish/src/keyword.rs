//! A typed implementation of keywords with support for wildcards.

/// A trait that should be implemented by all user-defined `enum`s used
/// to encode the keywords used in documents.
pub trait Keyword: Sized {
    /// Turns the given string into a keyword. If the given string does not
    /// match any of the known todo keywords, this should return `None`,
    /// *not* constructing a default variant. This function is partly
    /// used as a heuristic internally for when something actually might be
    /// a keyword.
    fn from_str(keyword: &str) -> Option<Self>;
    /// Converts this keyword into a string to put in a document file. This should
    /// match exactly the string that was parsed (although deviating from this
    /// pattern could be used for altering keywords en-masse).
    fn into_string(self) -> String;
    /// Constructs a miscellaneous keyword. This function must be manually
    /// implemented to force implementation to have a wildcard case for
    /// keywords that appear before a defined priority, but which are not
    /// in the given list (as they will still be parsed as keywords
    /// to avoid false-negative identification of new keywords, which would
    /// miss valid priorities).
    fn other(keyword: String) -> Self;
}
