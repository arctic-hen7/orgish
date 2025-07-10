//! The core logic for converting a machine-readable representation of a document
//! into valid textual form.

use super::{
    keyword::Keyword, Document, Node, ParseId, Planning, Priority, Properties, Tags, Timestamp,
};
use crate::{Attributes, Format, ParseString};
use indexmap::IndexMap;
use serde::Serialize;

impl<K: Keyword, I: ParseId, S: ParseString> Document<K, I, S> {
    /// Converts this document into a string.
    pub fn into_string(mut self, format: Format) -> String {
        // Implant the title and tags back into the attributes (we ned to provide the format in
        // case there were no attributes before and we need to create some, in which case we may as
        // well align with the format we're outputting to)
        self.attributes
            .set_title(self.root.title.to_string(format), format);
        self.attributes.set_tags(self.root.tags.to_vec(), format);
        // This won't include the attributes
        let root_str = self.root.into_string(format);
        // Put the attributes in the appropriate place depending on the format. Note that the
        // parser will note down newlines from the content (the only palce formatting may not be
        // preserved is between attributes in Org mode and between frontmatter and properties in
        // Markdown).
        match format {
            Format::Markdown => {
                let attributes_str = self.attributes.into_string(format);
                if !attributes_str.is_empty() {
                    format!("{}\n{root_str}", attributes_str)
                } else {
                    root_str
                }
            }
            Format::Org => {
                let attributes_str = self.attributes.into_string(format);
                if !attributes_str.is_empty() {
                    if root_str.starts_with(":PROPERTIES:") {
                        // We'll put the attributes after the properties (no spacing therebetween)
                        root_str.replacen(":END", &format!(":END:\n{}", attributes_str), 1)
                    } else {
                        // There are no top-level properties, we'll put the attributes at the start
                        format!("{}\n{root_str}", attributes_str)
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
            let title = self.title.to_string(format);
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
            // For the root, we only care about properties (the title and tags will be handled at
            // the document-level, implanting from the attributes)
            push_part(self.properties.into_string(format));
        }

        if let Some(body) = self.body {
            // Even if this is empty, we still want to push it, because `Some("")` is a single
            // empty line
            node_parts.push(body.to_string(format));
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
            properties_str.push_str(&v.to_string(format));
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

impl Attributes {
    /// Sets the tags in these attributes to the given value.
    fn set_tags(&mut self, tags: Vec<String>, format: Format) {
        if tags.is_empty() {
            // We have no tags, remove the property proactively
            match self {
                Self::Org(map) => {
                    map.shift_remove("filetags");
                }
                Self::MarkdownYaml(map) => {
                    map.remove("tags");
                }
                Self::MarkdownToml(map) => {
                    map.remove("tags");
                }
                Self::None => {}
            }
        } else {
            match self {
                Self::Org(map) => {
                    map.insert("filetags".to_string(), format!(":{}:", tags.join(":")));
                }
                Self::MarkdownYaml(map) => {
                    map.insert("tags".into(), tags.into());
                }
                Self::MarkdownToml(map) => {
                    map.insert("tags".to_string(), tags.into());
                }
                // If there are no attributes, use the format to create some appropriately
                Self::None => match format {
                    Format::Org => {
                        let mut map = IndexMap::new();
                        map.insert("filetags".to_string(), format!(":{}:", tags.join(":")));
                        *self = Self::Org(map);
                    }
                    // YAML is the default for Markdown
                    Format::Markdown => {
                        let mut map = serde_yaml::Mapping::new();
                        map.insert("tags".into(), tags.into());
                        *self = Self::MarkdownYaml(map);
                    }
                },
            }
        }
    }
    /// Sets the title in these attributes to the given string (if set).
    fn set_title(&mut self, title: String, format: Format) {
        if title.is_empty() {
            match self {
                Self::Org(map) => {
                    map.shift_remove("title");
                }
                Self::MarkdownYaml(map) => {
                    map.remove("title");
                }
                Self::MarkdownToml(map) => {
                    map.remove("title");
                }
                Self::None => {}
            }
        } else {
            match self {
                Self::Org(map) => {
                    map.insert("title".to_string(), title);
                }
                Self::MarkdownYaml(map) => {
                    map.insert("title".into(), title.into());
                }
                Self::MarkdownToml(map) => {
                    map.insert("title".to_string(), title.into());
                }
                // Create attributes from the format
                Self::None => match format {
                    Format::Org => {
                        let mut map = IndexMap::new();
                        map.insert("title".to_string(), title);
                        *self = Self::Org(map);
                    }
                    Format::Markdown => {
                        let mut map = serde_yaml::Mapping::new();
                        map.insert("title".into(), title.into());
                        *self = Self::MarkdownYaml(map);
                    }
                },
            }
        }
    }
    /// Converts these attributes into a string in the given format. If the format matches what the
    /// attributes were originally parsed as, this will proceed without problems. If converting
    /// from Org to Markdown, YAML frontmatter will be returned. If converting from YAML/TOML
    /// Markdown to Org, any non-string properties will be serialised to strings and inserted as
    /// single-line values.
    fn into_string(self, format: Format) -> String {
        match format {
            Format::Markdown => match self {
                Self::MarkdownYaml(map) => {
                    // This is guaranteed to be a valid YAML value
                    let yaml_str = serde_yaml::to_string(&map).unwrap();
                    // There is a trailing newline
                    format!("---\n{yaml_str}---")
                }
                Self::MarkdownToml(map) => {
                    // This is guaranteed to be a valid TOML value
                    let toml_str = toml::to_string(&map).unwrap();
                    // There is a trailing newline
                    format!("+++\n{toml_str}+++")
                }
                Self::None => String::new(),
                // Org to Markdown means conversion of key-value pairs into YAML, which is
                // infallible, but we must be sure to change `tags` to `filetags`
                Self::Org(map) => {
                    let mut yaml_map = serde_yaml::Mapping::new();
                    for (key, value) in map {
                        // Convert our one "implicit array" to a proper array
                        if key == "filetags" {
                            yaml_map.insert(
                                "tags".into(),
                                serde_yaml::Value::Sequence(
                                    value
                                        .split(':')
                                        .filter(|s| !s.is_empty())
                                        .map(|s| serde_yaml::Value::String(s.to_string()))
                                        .collect::<Vec<_>>(),
                                ),
                            );
                        } else {
                            yaml_map.insert(key.into(), value.into());
                        }
                    }
                    let yaml_str = serde_yaml::to_string(&yaml_map).unwrap();
                    // Implicit newline
                    format!("---\n{yaml_str}---")
                }
            },
            Format::Org => match self {
                Self::Org(map) => map
                    .into_iter()
                    .map(|(key, value)| format!("#+{key}: {value}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                Self::None => String::new(),
                Self::MarkdownToml(_) | Self::MarkdownYaml(_) => {
                    let mut org_map = IndexMap::new();

                    // Stringify every key and value, and add them to the Org map
                    match self {
                        Self::MarkdownYaml(map) => {
                            for (key, value) in map {
                                // Handle `tags` -> `filetags` specially
                                if key.as_str() == Some("tags") {
                                    let tags = value
                                        .as_sequence()
                                        // This can't fail after parsing (which is the only time
                                        // this crate-internal function can be called), because we
                                        // check that `tags` is an array of strings
                                        .unwrap()
                                        .iter()
                                        // See above
                                        .map(|v| v.as_str().unwrap().to_string())
                                        .collect::<Vec<_>>()
                                        .join(":");
                                    org_map.insert("filetags".to_string(), format!(":{tags}:"));
                                } else {
                                    org_map.insert(
                                        // Inherent newlines get put all through this, so make sure we
                                        // don't end up with multiline keys/values under any
                                        // circumstances
                                        serde_yaml::to_string(&key)
                                            .unwrap()
                                            .trim()
                                            .replace("\n", "\\n")
                                            .replace("\r", "\\r"),
                                        serde_yaml::to_string(&value)
                                            .unwrap()
                                            .trim()
                                            .replace("\n", "\\n")
                                            .replace("\r", "\\r"),
                                    );
                                }
                            }
                        }
                        Self::MarkdownToml(map) => {
                            for (key, value) in map {
                                // Need to use the special `ValueSerializer` because we aren't
                                // working with a full document, and we'll make an exception for
                                // strings and serialize them as-is (to avoid quotes). Also
                                // specially handle `tags` and rewrite as `filetags` in Org's
                                // format.
                                let mut value_str = String::new();
                                if key == "tags" {
                                    value_str = format!(
                                        ":{tags}:",
                                        tags = value
                                            .as_array()
                                            // This can't fail after parsing (which is the only
                                            // time this crate-internal function can be called),
                                            // because we check that `tags` is an array of strings
                                            .unwrap()
                                            .iter()
                                            // See above
                                            .map(|v| v.as_str().unwrap().to_string())
                                            .collect::<Vec<_>>()
                                            .join(":")
                                    );
                                } else if value.is_str() {
                                    value_str = value.as_str().unwrap().to_string();
                                } else {
                                    let ser = toml::ser::ValueSerializer::new(&mut value_str);
                                    Serialize::serialize(&value, ser).unwrap();
                                }

                                org_map.insert(
                                    if key == "tags" {
                                        "filetags".to_string()
                                    } else {
                                        key
                                    },
                                    value_str.replace("\n", "\\n").replace("\r", "\\r"),
                                );
                            }
                        }
                        _ => unreachable!(),
                    }

                    org_map
                        .into_iter()
                        .map(|(key, value)| format!("#+{key}: {value}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            },
        }
    }
}
