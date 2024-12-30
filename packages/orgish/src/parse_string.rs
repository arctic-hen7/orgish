/// A trait for systems that can parse strings into representations containing additional data.
/// This can be used to, for example, parse low-level syntax like that for bold and italics, or for
/// parsing connections, etc. This is applied to the strings from node titles, bodies, and property
/// values.
///
/// Note that such systems must have a `Default` implementation which produces an empty string when
/// `.to_string()` is called.
pub trait ParseString: Default {
    /// Errors that can occur when parsing from a string.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Parses the given string.
    fn from_str(s: String) -> Result<Self, Self::Error>
    where
        Self: Sized;
    /// Produces a string representation of self.
    fn to_string(&self) -> String;
}

impl ParseString for String {
    type Error = std::convert::Infallible;

    fn from_str(s: String) -> Result<Self, Self::Error> {
        Ok(s)
    }
    fn to_string(&self) -> String {
        self.clone()
    }
}
