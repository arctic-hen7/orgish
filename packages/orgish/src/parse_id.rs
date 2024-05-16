/// A trait that any unique identifier implementations must implement. This will allow the identifiers
/// to be sensibly parsed and even automatically created for all nodes in a document.
///
/// This is used to parse the value of the `ID` property, wherever it exists.
pub trait ParseId: Sized + std::fmt::Debug {
    /// Creates some initial value of the identifier. This will be used for *all* nodes that don't have
    /// an explicitly set identifier. Typically, this would return something akin to `None`, but it can
    /// also create a new identifier, which will mean that rewriting a parsed document will add identifiers
    /// to any nodes that didn't previously have them (which can be very useful in some cases).
    fn initial() -> Self;
    /// Parses the given string as an identifier if possible. If the string is clearly not an identifier,
    /// `None` should be returned, and an error will be returned through the parser.
    fn parse(value: &str) -> Option<Self>;
    /// Determines whether or not the current value is none-like. This is used to determine whether or not
    /// an ID needs to be written at all. Depending on how this is implemented, the document parser may rewrite
    /// certain parts of the document (e.g. if a blank ID was previously specified and this registers as
    /// none-like, that ID would be stripped).
    fn is_none(&self) -> bool;
    /// The opposite of [`Self::is_none`].
    fn is_some(&self) -> bool {
        !self.is_none()
    }
    /// Turns this identifier into a string to be written to a file. This will only ever be called
    /// if `self.is_some()` is true, so you can safely mark cases where there is no identifier as `unreachable!()`,
    /// or, if you'd prefer, produce an empty string instead.
    ///
    /// Notably, this can be used with a custom parser to convert all the identifiers in a file from
    /// one format into another (by combining, say, a timestamp-based parser with a UUID creation function
    /// here).
    fn into_string(self) -> String;
}

/// A string representation of an identifier. This will parse any identifier as valid, and is the default
/// identifier if none other is specified.
#[derive(Debug, Clone)]
pub struct StringId(Option<String>);
impl std::ops::Deref for StringId {
    type Target = Option<String>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for StringId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl ParseId for StringId {
    fn initial() -> Self {
        Self(None)
    }
    fn parse(value: &str) -> Option<Self> {
        Some(Self(Some(value.to_string())))
    }
    fn is_none(&self) -> bool {
        self.0.is_none()
    }
    // NOTE: We use panics here to ensure in testing that this function is only called if we have an identifier
    // at all.
    fn into_string(self) -> String {
        if let Some(inner) = self.0 {
            inner
        } else {
            unreachable!()
        }
    }
}

/// A nonexistent identifier. This can be used to strip the IDs from a document/node, or in testing,
/// where it can be very useful to test parsing with IDs, and to then strip them to avoid having to
/// handle them in string equivalence checks. It is rare to use this in production applications.
#[derive(Debug, Clone)]
pub struct NoId;
impl ParseId for NoId {
    fn initial() -> Self {
        Self
    }
    fn parse(_: &str) -> Option<Self> {
        Some(Self)
    }
    fn is_none(&self) -> bool {
        true
    }
    // This will never be called
    fn into_string(self) -> String {
        unreachable!()
    }
}

#[cfg(feature = "uuid-id-parser")]
mod uuid_parser {
    use super::ParseId;
    use uuid::Uuid;

    /// An identifier parser built on v4 (random) UUIDs. This will assume any node with an ID is using
    /// the v4 UUID generation scheme, but will not force UUID creation for nodes without identifier.
    #[derive(Debug, Clone)]
    pub struct UuidId(Option<Uuid>);
    impl std::ops::Deref for UuidId {
        type Target = Option<Uuid>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl std::ops::DerefMut for UuidId {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
    impl ParseId for UuidId {
        fn initial() -> Self {
            Self(None)
        }
        fn parse(value: &str) -> Option<Self> {
            let uuid = Uuid::parse_str(value).ok()?;
            Some(Self(Some(uuid)))
        }
        fn is_none(&self) -> bool {
            self.0.is_none()
        }
        fn into_string(self) -> String {
            if let Some(uuid) = self.0 {
                format!("{}", uuid.hyphenated())
            } else {
                unreachable!()
            }
        }
    }
}
#[cfg(feature = "uuid-id-parser")]
mod force_uuid_parser {
    use super::ParseId;
    use uuid::Uuid;

    /// An identifier parser built on v4 (random) UUIDs. This will assume any node with an ID is using
    /// the v4 UUId generation scheme, and will **forcibly create** new UUIDs for all nodes that don't
    /// have identifiers already defined.
    ///
    /// Unless you want to aggressively force all nodes in a document to have identifiers, you should use
    /// [`UuidId`] instead.
    #[derive(Debug, Clone)]
    pub struct ForceUuidId(Uuid);
    impl std::ops::Deref for ForceUuidId {
        type Target = Uuid;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl std::ops::DerefMut for ForceUuidId {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
    impl ParseId for ForceUuidId {
        fn initial() -> Self {
            Self(Uuid::new_v4())
        }
        fn parse(value: &str) -> Option<Self> {
            let uuid = Uuid::parse_str(value).ok()?;
            Some(Self(uuid))
        }
        fn is_none(&self) -> bool {
            // We will always have a value with this parser
            false
        }
        fn into_string(self) -> String {
            format!("{}", self.0.hyphenated())
        }
    }
}

#[cfg(feature = "uuid-id-parser")]
pub use force_uuid_parser::ForceUuidId;
#[cfg(feature = "uuid-id-parser")]
pub use uuid_parser::UuidId;
