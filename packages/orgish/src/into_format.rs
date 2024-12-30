//! The core logic for converting a machine-readable representation of a document
//! into valid textual form.

use super::{
    keyword::Keyword, Document, Node, ParseId, Planning, Priority, Properties, Tags, Timestamp,
};
use crate::{Format, ParseString};

impl<K: Keyword, I: ParseId, S: ParseString> Document<K, I, S> {
    /// Converts this document into a string.
    pub fn into_string(self, format: Format) -> String {
        let root_str = self.root.into_string(format);
        // Put the attributes in the appropriate place depending on the format. Note that the
        // parser will note down newlines from the content (the only palce formatting may not be
        // preserved is between attributes in Org mode and between frontmatter and properties in
        // Org mode).
        match format {
            Format::Markdown => {
                if !self.attributes.is_empty() {
                    format!("{}\n{root_str}", self.attributes)
                } else {
                    root_str
                }
            }
            Format::Org => {
                if !self.attributes.is_empty() {
                    if root_str.starts_with(":PROPERTIES:") {
                        // We'll put the attributes after the properties (no spacing therebetween)
                        root_str.replacen(":END", &format!(":END:\n{}", self.attributes), 1)
                    } else {
                        // There are no top-level properties, we'll put the attributes at the start
                        format!("{}\n{root_str}", self.attributes)
                    }
                } else {
                    root_str
                }
            }
        }
    }
}

impl<K: Keyword, I: ParseId, S: ParseString> Node<K, I, S> {
    /// Converts this node into a string in the given format. Note that, while this function guarantees
    /// no information will be lost in the process of parse-then-write, information may be reshuffled,
    /// notably top-level attributes such as `#+title` and `#+filetags`, which will always come first,
    /// in that order, when a parsed document is written to a string.
    ///
    /// If called for the root node (i.e. a node with level `0`), this function will
    /// not produce a heading, only the body contents (by recursively calling this
    /// method on the rest of the node tree).
    // Implementation: this is only possible if the representation of each node is *totally*
    // self-contained, a property that must be preserved by the parser.
    pub fn into_string(self, format: Format) -> String {
        let mut node_parts = Vec::new();
        // Alias closure for pushing things that aren't empty (otherwise we get too many newlines)
        let mut push_part = |part: String| {
            if !part.is_empty() {
                node_parts.push(part);
            }
        };
        let with_space_after = |thing: &str| {
            if thing.is_empty() {
                String::new()
            } else {
                format!("{thing} ")
            }
        };
        let with_space_before = |thing: &str| {
            if thing.is_empty() {
                String::new()
            } else {
                format!(" {thing}")
            }
        };
        // Handling the root node is quite special (keep in mind this will occur in the context of
        // the document parsing itself!)
        if self.level > 0 {
            let stars = format
                .heading_char()
                .to_string()
                .repeat(self.level as usize);
            let tags_str = with_space_before(&self.tags.into_string());
            let title = self.title.to_string();
            let keyword =
                with_space_after(&self.keyword.map(|k| k.into_string()).unwrap_or_default());
            let priority = with_space_after(&self.priority.into_string());
            let timestamps = with_space_before(
                &self
                    .timestamps
                    .into_iter()
                    .map(|t| t.into_string())
                    .collect::<Vec<_>>()
                    .join(" "),
            );

            let heading = format!("{stars} {keyword}{priority}{title}{timestamps}{tags_str}")
                .trim()
                .to_string();
            push_part(heading);
            // Add the planning info (https://orgmode.org/worg/org-syntax.html#Property_Drawers
            // makes clear that nothing else comes before properties)
            push_part(self.planning.into_string());
            push_part(self.properties.into_string(format));
        } else {
            // For the root, we only care about properties (the title and tags are left empty in
            // the tree, and defined opaquely in the format-specific attributes)
            push_part(self.properties.into_string(format));
        }

        if let Some(body) = self.body {
            // Even if this is empty, we still want to push it, because `Some("")` is a single
            // empty line
            node_parts.push(body.to_string());
        }

        // Convert all the top-level children
        for node in self.children {
            // These will definitely be non-empty because they contain headings
            node_parts.push(node.into_string(format));
        }

        node_parts.join("\n")
    }
}

impl Priority {
    /// Converts this priority into its string representation, if there is indeed a priority
    /// specified.
    pub fn into_string(self) -> String {
        match self.0 {
            Some(note) => format!("[#{note}]"),
            None => String::new(),
        }
    }
}

impl Planning {
    /// Converts these planning items into their string representation.
    pub fn into_string(self) -> String {
        let mut planning_items = Vec::new();

        let mut add_item = |prop: Option<Timestamp>, name: &'static str| {
            if let Some(timestamp) = prop {
                let mut item = name.to_string();
                item.push_str(": ");
                item.push_str(&timestamp.into_string());

                planning_items.push(item);
            }
        };

        add_item(self.deadline, "DEADLINE");
        add_item(self.scheduled, "SCHEDULED");
        add_item(self.closed, "CLOSED");

        planning_items.join("\n")
    }
}

impl<I: ParseId, S: ParseString> Properties<I, S> {
    /// Converts these properties into a textual property drawer. With the exception of the `ID`
    /// property, which, if present, will always be placed first, the properties will always be written
    /// in alphabetical order.
    ///
    /// This is format-specific, as properties drawers are opened/closed differently in different formats.
    pub fn into_string(self, format: Format) -> String {
        // Short-circuit if there's nothing to write
        if self.id.is_none() && self.inner.is_empty() {
            return String::new();
        }

        let mut properties_str = format.get_properties_opener().to_string();
        // Check if we have an ID to write (this checker is based on the custom
        // implementation of the ID parser)
        if self.id.is_some() {
            properties_str.push('\n');
            properties_str.push_str(match format {
                Format::Markdown => "ID: ",
                Format::Org => ":ID: ",
            });
            properties_str.push_str(&self.id.into_string());
        }
        // Now do the regular properties (in alphabetical order, for testing consistency)
        let mut keys = self.inner.keys().collect::<Vec<_>>();
        keys.sort();
        for k in keys {
            let v = self.inner.get(k).unwrap();

            properties_str.push('\n');
            // The key-leading colon is Org-only
            if format == Format::Org {
                properties_str.push(':');
            }
            properties_str.push_str(&k);
            properties_str.push_str(": ");
            properties_str.push_str(&v.to_string());
        }
        properties_str.push('\n');
        properties_str.push_str(format.get_properties_closer());

        properties_str
    }
}

impl Tags {
    /// Converts these tags into their string representation (i.e. `:tag1:tag2:tag3:`).
    pub fn into_string(self) -> String {
        if self.inner.is_empty() {
            String::new()
        } else {
            let mut tags_str = ":".to_string();
            for tag in self.inner {
                tags_str.push_str(&tag);
                tags_str.push_str(":");
            }

            tags_str
        }
    }
}
