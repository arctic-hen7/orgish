//! A semantic document parser built on the basis of textual analysis of document files in defined
//! formats, such as Org mode and Markdown. For Org mode, this was made without reference to the
//! original ELisp implementation, and it implements almost identical parsing for Markdown, as it
//! aims to parse only the semantic details of the outline of a document for working with it as a
//! data file above all else. This will *not* parse markup like italics or boldface, but will parse
//! the outline of a file and properties on its nodes. With support for Org-style TODO states,
//! property drawers, tags, timestamps, and so forth, it aims to bring the extensible data-focused
//! parts of the Org mode specification to other formats, especially Markdown.
//!
//! NOTE: At the moment, this has been made with almost no care to support formats beyond Markdown
//! or Org mode, though it should work with any format that defines headlines in a similar way
//! (i.e. through the repetition of some heading character like `#` or `*`), and where properties
//! can be delimited within a heading through some special character sequence (e.g. `PROPERTIES:`
//! or `+++`).

/// Errors to do with parsing and handling the representations of documents.
pub mod error;
mod format;
mod heading_parser;
mod into_format;
pub mod keyword;
mod parse_id;
mod parse_string;
mod parser;
pub mod timestamp;

// Using this structure for ease of storing utility functions
#[cfg(test)]
pub mod tests;

pub use self::keyword::Keyword;
pub use format::*;
pub use parse_id::*;
pub use parse_string::ParseString;
pub use timestamp::Timestamp;

use error::ParseError;
use indexmap::IndexMap;
use std::{collections::HashMap, convert::identity};

/// A document in some format. The document's properties and root body will be captured in the root node.
/// This does *not* save the document's format details, and conversion into another format is
/// trivial.
#[derive(Debug, Clone)]
pub struct Document<K: Keyword, I: ParseId = StringId, S: ParseString = String> {
    /// The root node.
    pub root: Node<K, I, S>,
    /// Top-level attributes for the whole document.
    ///
    /// In Org mode, these will be things like `#+title` or `#+description`, while in Markdown
    /// they'll be the parsed contents of a YAML or TOML frontmatter block.
    ///
    /// The title and tags will be extracted from here and implanted into the root node, where they
    /// may be changed during operation, before being updated in the attributes again when written
    /// back to a string. As such, the title and tags in here should *not* be depended on!
    pub attributes: Attributes,
}
impl<K: Keyword, I: ParseId, S: ParseString> Default for Document<K, I, S> {
    fn default() -> Self {
        Self {
            root: Node::default(),
            attributes: Attributes::None,
        }
    }
}
impl<K: Keyword, I: ParseId, S: ParseString> Document<K, I, S> {
    // /// Creates a new document with the given attributes and document-level tags.
    // // TODO: We don't take tags at the root anymore...
    // pub fn new(attributes: String, tags: Vec<String>) -> Self {
    //     let mut root = Node::new(0, S::default(), None);
    //     *root.tags = tags;
    //
    //     Self { root, attributes }
    // }
    /// Transforms all nodes in this document to have a different type of unique identifier. This is extremely
    /// useful for mass migrations, as well as for removing identifiers in testing.
    pub fn map_ids<J: ParseId>(self, f: impl Fn(I) -> J) -> Document<K, J, S> {
        fn map<K: Keyword, I: ParseId, J: ParseId, S: ParseString>(
            mut node: Node<K, I, S>,
            f: &impl Fn(I) -> J,
        ) -> Node<K, J, S> {
            let props = std::mem::take(&mut node.properties);
            let new_id = f(props.id);
            Node {
                level: node.level,
                title: node.title,
                priority: node.priority,
                tags: node.tags,
                planning: node.planning,
                properties: Properties {
                    id: new_id,
                    inner: props.inner,
                },
                keyword: node.keyword,
                body: node.body,
                timestamps: node.timestamps,
                children: node
                    .children
                    .into_iter()
                    .map(|child| map(child, f))
                    .collect(),
            }
        }

        Document {
            root: map(self.root, &f),
            attributes: self.attributes,
        }
    }
    /// Strips identifiers from the document and all nodes therein. This is almost exclusively useful in
    /// testing, where testing random UUIDs is generally very inconvenient, and simple string comparisons
    /// are far more efficient.
    pub fn strip_ids(self) -> Document<K, NoId, S> {
        self.map_ids(|_| NoId)
    }
    /// Transforms all nodes in this document to have a different keyword type. This is extremely useful for
    /// mass migrations.
    pub fn map_keywords<L: Keyword>(
        self,
        f: &impl Fn(Option<K>) -> Option<L>,
    ) -> Document<L, I, S> {
        fn map<K: Keyword, I: ParseId, L: Keyword, S: ParseString>(
            mut node: Node<K, I, S>,
            f: &impl Fn(Option<K>) -> Option<L>,
        ) -> Node<L, I, S> {
            let new_keyword = f(std::mem::take(&mut node.keyword));
            Node {
                level: node.level,
                title: node.title,
                priority: node.priority,
                tags: node.tags,
                planning: node.planning,
                properties: node.properties,
                keyword: new_keyword,
                body: node.body,
                timestamps: node.timestamps,
                children: node
                    .children
                    .into_iter()
                    .map(|child| map(child, f))
                    .collect(),
            }
        }

        Document {
            root: map(self.root, &f),
            attributes: self.attributes,
        }
    }
    /// Gets the last node in the tree at a certain level. This is used in the parser to get the correct
    /// parent for the next node at `level + 1`. This will return `None` if there are no nodes of the given
    /// level at the latest point in the tree.
    fn get_last_node_at_level(&mut self, level: u8) -> Option<&mut Node<K, I, S>> {
        let mut curr_level = 0;
        let mut curr_parent = &mut self.root;
        while curr_level < level {
            curr_parent = curr_parent.children.last_mut()?;
            curr_level += 1;
        }

        Some(curr_parent)
    }
}

/// The attributes a document can contain at its start. These are stored in generally
/// format-specific ways, and are parsed only for a title and tags. They will be left in the order
/// they were originall parsed.
#[derive(Clone, Debug)]
pub enum Attributes {
    /// An ordered map of Org properties, which must be key-value string pairs (any additional
    /// parsing should be done by the user). For an Org-mode document, this is *guaranteed* to be
    /// the attribute format.
    ///
    /// Note that all attribute keys will be converted to lowercase when written back to a string.
    Org(IndexMap<String, String>),
    /// YAML properties from Markdown frontmatter. This is guaranteed to be the attribute format
    /// used for Markdown documents with `---` frontmatter.
    ///
    /// If a conversion from Org to Markdown is performed, this format will be used.
    MarkdownYaml(serde_yaml::Mapping),
    /// TOML properties from Markdown frontmatter. This is guaranteed to be the attribute format
    /// used for Markdown documents with `+++` frontmatter.
    MarkdownToml(toml::Table),
    None,
}
impl Attributes {
    /// Gets the title from these attributes, if there is one.
    pub(crate) fn title(&self) -> Result<String, ParseError> {
        match &self {
            Self::Org(map) => Ok(map.get("title").cloned().unwrap_or_else(String::new)),
            // A non-string title is an error because otherwise we'd overwrite it with nothing wehn
            // writing back to a string
            Self::MarkdownYaml(map) => map
                .get("title")
                .map(|val| {
                    val.as_str()
                        .map(|s| s.to_string())
                        .ok_or(ParseError::RootTitleNotString)
                })
                .unwrap_or_else(|| Ok(String::new())),
            Self::MarkdownToml(map) => map
                .get("title")
                .map(|val| {
                    val.as_str()
                        .map(|s| s.to_string())
                        .ok_or(ParseError::RootTitleNotString)
                })
                .unwrap_or_else(|| Ok(String::new())),
            Self::None => Ok(String::new()),
        }
    }
    /// Gets the tags from these attributes. If none are defined, this will be an empty vector,
    /// which should be written back as an absence of the `tags` property.
    pub(crate) fn tags(&self) -> Result<Vec<String>, ParseError> {
        match &self {
            Self::Org(map) => Ok(map
                // Yes, this is officially how Org handles top-level tags
                .get("filetags")
                // Split tags of the form `:hello:world:`
                .map(|tags_str| {
                    tags_str
                        .split(':')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_else(|| Vec::new())),
            // A non-string title is an error because otherwise we'd overwrite it with nothing wehn
            // writing back to a string
            Self::MarkdownYaml(map) => map
                .get("tags")
                .map(|val| {
                    // Try to convert to a vector...
                    val.as_sequence()
                        .ok_or(ParseError::RootTagsNotStringVec)
                        // ...if that worked, try to convert every element to a string
                        .map(|seq| {
                            seq.into_iter()
                                .map(|val| {
                                    val.as_str()
                                        .map(|s| s.to_string())
                                        .ok_or(ParseError::RootTagsNotStringVec)
                                })
                                .collect::<Result<Vec<String>, ParseError>>()
                        })
                        // Now we have an outer `Result` for a non-vector, and an inner result for any
                        // non-strings, flatten the two
                        .map_or(Err(ParseError::RootTagsNotStringVec), identity)
                })
                // No defined tags are the same as defined no tags
                .unwrap_or_else(|| Ok(Vec::new())),
            Self::MarkdownToml(map) => map
                .get("tags")
                .map(|val| {
                    val.as_array()
                        .ok_or(ParseError::RootTagsNotStringVec)
                        .map(|arr| {
                            arr.into_iter()
                                .map(|val| {
                                    val.as_str()
                                        .map(|s| s.to_string())
                                        .ok_or(ParseError::RootTagsNotStringVec)
                                })
                                .collect::<Result<Vec<String>, ParseError>>()
                        })
                        .map_or(Err(ParseError::RootTagsNotStringVec), identity)
                })
                // No defined tags are the same as defined no tags
                .unwrap_or_else(|| Ok(Vec::new())),
            Self::None => Ok(Vec::new()),
        }
    }
}

/// A single *node*, the term used by this parser for a heading-like element, which may also
/// be the root of a document.
///
/// Note that, due to the breadth of where they can be placed, attributes (e.g. `#+caption`) are not
/// parsed here (and they notably cannot validly appear for headings), and will instead appear in the
/// untyped `body` property of the relevant node.
#[derive(Debug, Clone)]
pub struct Node<K: Keyword, I: ParseId = StringId, S: ParseString = String> {
    /// The indent level of the heading. The root of the document will have level `0`. Once instantiated,
    /// the level of a node cannot be changed in order to preserve the property that levels increase in
    /// nesting.
    ///
    /// Users wishing to promote the level of a node should restructure the document tree rather than
    /// introducing an invalid representation.
    level: u8,
    /// The textual title of the node. This will *not* contain any todo keywords.
    ///
    /// In the root node of a document, this will be empty (the `attributes` map will contain the
    /// title instead).
    pub title: S,
    /// The priority of the heading.
    pub priority: Priority,
    /// Any tags the node has. Tag inheritance is *not* automatically implemented by this parser, and, as such,
    /// this contains only the tags defined directly on this node, not any that might exist in parent headings
    /// or the root node.
    ///
    /// Tags on documents will not appear in the root node, but in top-level attributes.
    pub tags: Tags,
    /// Planning items, such as deadlines or "scheduled" markers. These are represented similarly to properties,
    /// but occur textually before them.
    pub planning: Planning,
    /// The properties of this node. Textually, these come directly after the planning information.
    pub properties: Properties<I, S>,
    /// The keyword for the node. This will be identified if it comes before a priority, or if it is the starting
    /// word of a title and matches one of the list of todo keywords given during parsing.
    pub keyword: Option<K>,
    /// The untyped body string of a node. This may contain all manner of markup mode elements, from source blocks
    /// to lists to links, etc., but it will not contain any subheadings, those will be parsed separately as
    /// children.
    ///
    /// This is represented as an `Option<String>` to prevent the issue that a nonexistent body and a body consisting
    /// of a single empty line are represented in the same way. Representing this way instead allows separating
    /// these cases and preventing issues with incorrectly rewriting newlines.
    ///
    /// **Warning:** it is possible to modify the body of a node to introduce new nodes, however these will
    /// not be parsed, and it is strongly recommended to not do this, as there are far better programmatic
    /// ways to do this! It is, however, not technically an invalidity.
    pub body: Option<S>,
    /// Timestamps on this node, if any are present. We allow multiple (*not* a feature of most Org
    /// parsers, but does work in Emacs, hence implemented here).
    ///
    /// Note that, when written back to text, timestamps in a heading will *always* be written at the end of the
    /// title, before any tags, regardless of where they were originally placed.
    pub timestamps: Vec<Timestamp>,
    /// The *top-level* children of this node. Ideally, the levels of all these children would be one greater
    /// than the level of this node, but *this is not guaranteed*. It is only guaranteed that, under normal
    /// operation, they will never be less than this node's level. As such, this property is private and
    /// manipulated through a series of methods.
    children: Vec<Node<K, I, S>>,
}
// Manual `Default` impl to avoid requiring a default keyword
impl<K: Keyword, I: ParseId, S: ParseString> Default for Node<K, I, S> {
    fn default() -> Self {
        Self {
            level: 0,
            title: S::default(),
            priority: Priority::default(),
            tags: Tags::default(),
            planning: Planning::default(),
            properties: Properties::default(),
            keyword: None,
            body: None,
            timestamps: Vec::new(),
            children: Vec::new(),
        }
    }
}
// TODO: Create a `Children` guard for modifying the children, which could allow adding them
// safely, but disallow raw changes to the underlying vector.
impl<K: Keyword, I: ParseId, S: ParseString> Node<K, I, S> {
    /// Creates a new node at the given level with the given title and body.
    pub fn new(level: u8, title: S, body: Option<S>) -> Self {
        Self {
            level,
            title,
            body,
            priority: Priority::default(),
            tags: Tags::default(),
            planning: Planning::default(),
            properties: Properties::default(),
            children: Vec::new(),
            keyword: None,
            timestamps: Vec::new(),
        }
    }
    /// Gets an immutable reference to the children of this node.
    pub fn children(&self) -> &Vec<Self> {
        &self.children
    }
    /// Gets a mutable reference to the children of this node.
    ///
    /// This should be used with extreme care, as it may lead to invalid tree structures if the
    /// requirement that children do not have levels lower than their parent is not upheld!
    pub fn unchecked_mut_children(&mut self) -> &mut Vec<Self> {
        &mut self.children
    }
    /// Gets an owned reference to the children of this node, consuming `self`.
    ///
    /// See [`Self::children`] for why mutable references are not available for children.
    pub fn into_children(self) -> Vec<Self> {
        self.children
    }
    /// Removes the children from this node and returns them, leaving the rest of the node intact.
    /// This is used in some secondary parsing behaviours related to higher-order functions, but
    /// generally `.children()` or `.into_children()` should be preferred.
    pub fn take_children(&mut self) -> Vec<Self> {
        let children = std::mem::take(&mut self.children);
        children
    }
    /// Sets the children of this node to the given vector, without checking them. Undefined behaviour
    /// from the parser's perspective (not actual UB, just very bad things that are undefined because we
    /// can't legally read Org mode's code, so if you chuck results into it they may re-parse as something
    /// completely different) may result if this is used without proper validation!
    pub fn unchecked_set_children(&mut self, children: Vec<Self>) {
        self.children = children;
    }
    /// Adds the given node as a child of this node. This will fail if the child would be
    /// equal to or higher than this node in the outline hierarchy (i.e. if its level is numerically
    /// lower than this node's).
    ///
    /// Note that this does **not** enforce the logical requirement that the headings proceed in
    /// *continuous* order, so there may be a level 2 heading as a child of the root without any
    /// errors occurring. This is a common pattern for exports to PDFs and other formats when one
    /// wishes to superficially control the size of headings without special configuration (by *common*,
    /// I mean I do it, so it's supported!).
    pub fn add_child(&mut self, child: Self) -> Result<(), ParseError> {
        if child.level > self.level {
            self.children.push(child);
            Ok(())
        } else {
            Err(ParseError::InvalidChildLevel {
                parent_level: self.level,
                bad_child_level: child.level,
            })
        }
    }
    /// Gets the level of this node (immutably).
    pub fn level(&self) -> u8 {
        self.level
    }
    /// Sets the level of this node. This will compute the difference between this and the current
    /// level and apply that as a patch to the level of all hcild nodes recursively, effectively
    /// in/decrementing the level of this entire tree as appropriate.
    ///
    /// This cannot fail, because it is always valid to change the level of a node in itself,
    /// however this operation may not be valid in the tree this node is in! As such, it is marked
    /// as unchecked, and should be used with caution.
    pub fn unchecked_set_level(&mut self, level: u8) {
        // Recursively applies the level diff to the given node and all children
        fn set_level<K: Keyword, I: ParseId, S: ParseString>(node: &mut Node<K, I, S>, diff: i8) {
            let new_level = node.level as i8 - diff;
            // This is completely valid because `diff` was generated from the highest level in this
            // tree minus the new level, so it can't cause this to become negative or anything else
            // crazy
            node.level = new_level as u8;

            for child in &mut node.children {
                set_level(child, diff);
            }
        }

        // This is the amount by which all levels will change. Note that, if this would lead
        let diff = self.level as i8 - level as i8;
        set_level(self, diff);
    }
}

/// Planning items of some heading. This is *very* closely derived from Org mode.
#[derive(Debug, Default, Clone)]
pub struct Planning {
    pub deadline: Option<Timestamp>,
    pub scheduled: Option<Timestamp>,
    pub closed: Option<Timestamp>,
}
impl Planning {
    /// Adds the given line of planning to this set of planning items. This will return `None`
    /// if the given line is not a planning line, and `Some(Err(_))` if an error occurred while
    /// parsing (especially the timestamp).
    pub fn add_line(&mut self, line: &str) -> Option<Result<(), ParseError>> {
        // Only split into two parts (timestamp may contain colons)
        let parts = line.splitn(2, ':').collect::<Vec<_>>();
        // Format: `TITLE: <timestamp>`
        if parts.len() != 2 {
            return None;
        };

        let key = parts[0].trim();
        let timestamp = parts[1].trim();

        // This abstracts over which property of `self` we're setting
        let update_self = |prop: &mut Option<Timestamp>| -> Option<Result<(), ParseError>> {
            Some(match Timestamp::from_str(timestamp) {
                Ok(timestamp) => {
                    *prop = Some(timestamp);
                    Ok(())
                }
                Err(err) => Err(err.into()),
            })
        };

        match key {
            "DEADLINE" => update_self(&mut self.deadline),
            "SCHEDULED" => update_self(&mut self.scheduled),
            "CLOSED" => update_self(&mut self.closed),
            _ => None,
        }
    }
}

/// Properties of some entry in a document. This will typically apply to a heading, but it can equally
/// apply to an entire document.
///
/// Properties are generic over the type of ID required.
#[derive(Debug, Clone)]
pub struct Properties<I: ParseId, S: ParseString> {
    /// The unique identifier of this entry.
    pub id: I,
    /// Freeform properties other than the ID.
    inner: HashMap<String, S>,
}
impl<I: ParseId, S: ParseString> Properties<I, S> {
    /// Adds a property pair from the given line to this set of properties. This is the general
    /// property parsing logic.
    pub(crate) fn add_line(&mut self, line: &str) -> Result<(), ParseError> {
        // Form: `:KEY: value` (first colon won't appear in Markdown, so we treat it as optional)
        let line = line.strip_prefix(':').unwrap_or(line);
        // Get the key and value
        let parts = line.splitn(2, ':').collect::<Vec<_>>();
        if parts.len() != 2 {
            return Err(ParseError::InvalidProperty {
                line: line.to_string(),
            });
        };

        let key = parts[0].trim();
        let value = parts[1].trim();

        // If this is an ID, parse it according to the given logic
        if key == "ID" {
            if let Some(id) = I::parse(&value) {
                self.id = id;
            } else {
                return Err(ParseError::IdParseFailed {
                    value: value.to_string(),
                });
            }
        } else {
            self.inner.insert(
                key.to_string(),
                S::from_str(value.to_string()).map_err(|source| ParseError::ParseStringFailed {
                    source: Box::new(source),
                })?,
            );
        }

        Ok(())
    }
}
impl<I: ParseId, S: ParseString> Default for Properties<I, S> {
    fn default() -> Self {
        Self {
            // This might create something like `None`, or it might create a new random UUID (which
            // would force all nodes to have IDs, but then override the pre-created ones if they already
            // have them)
            id: I::initial(),
            inner: HashMap::default(),
        }
    }
}
// Even though we have the ID, properties are overwhelmingly manipulated like this
impl<I: ParseId, S: ParseString> std::ops::Deref for Properties<I, S> {
    type Target = HashMap<String, S>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<I: ParseId, S: ParseString> std::ops::DerefMut for Properties<I, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// A priority note on a heading. As these notes can contain any kind of string, they should be
/// manually parsed from here, and they are represented using a newtype wrapper to allow implementing
/// custom traits for convenient parsing logic.
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Priority(pub Option<String>);

/// The tags on a node.
#[derive(Debug, Default, Clone)]
pub struct Tags {
    inner: Vec<String>,
}
impl std::ops::Deref for Tags {
    type Target = Vec<String>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl std::ops::DerefMut for Tags {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
