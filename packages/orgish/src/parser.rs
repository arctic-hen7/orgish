//! The core parsing logic that converts supported documents into machine-readable representations.

use super::{Document, Keyword, Node, Tags};
use super::{ParseError, ParseId};
use crate::format::Format;
use std::cmp::Ordering;

impl<K: Keyword, I: ParseId> Document<K, I> {
    /// Parses a document from its string representation.
    pub fn from_str(raw_contents: &str, format: Format) -> Result<Self, ParseError> {
        let mut document = Document::<K, I>::default();
        let mut document_attributes = String::new();
        // This will track the active node (*not* used for the root node!)
        let mut curr_node = Node::<K, I>::default();
        let mut curr_parent = &mut document.root;
        // We add lines in the body to a vector to simplify newline management
        let mut curr_body: Vec<&str> = Vec::new();

        // Joins the body and handles special cases
        let finish_body = |curr_body: &mut Vec<&str>, body: &mut Option<String>| {
            // Problem: a single empty newline in the body and a completely empty (i.e.
            // nonexistent) body are difficult to represent. We have a vector of lines
            // in `curr_body`, and an empty vector will produce the same output as a
            // vector with a single newline.
            // Solution: we represent truly empty bodies as `None`.

            *body = if curr_body.is_empty() {
                None
            } else {
                Some(curr_body.join("\n"))
            };
            *curr_body = Vec::new();
        };

        let mut loc = match format {
            Format::Org => ParseLocation::OrgStart(OrgStartLocation::Beginning),
            Format::Markdown => ParseLocation::MarkdownStart(MarkdownStartLocation::Beginning {
                have_seen_frontmatter: false,
            }),
        };

        // NOTE: This will strip a final newline if it appears, which may lead to strange behaviour
        let lines = raw_contents.lines().collect::<Vec<_>>();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];

            // Regardless of where we are, parsing a new node is the same (and should break the current parsing cycle)
            if let Some(new_node) = Node::<K, I>::from_heading_str(line, format) {
                if loc.is_start() {
                    // After we finish with the root node, we should just initialise `curr_node` properly, because we've
                    // been working on `curr_parent`
                    curr_node = new_node;
                    // Handle the body
                    finish_body(&mut curr_body, &mut curr_parent.body);
                } else {
                    // Handle the body
                    finish_body(&mut curr_body, &mut curr_node.body);

                    let new_level = new_node.level;
                    let curr_level = curr_node.level;

                    // Extract the node we've finished with and add it to its parent
                    let last_node = std::mem::replace(&mut curr_node, new_node);
                    // This will now be a mutable reference, and the child will be in the list of children
                    curr_parent.add_child(last_node)?;

                    // Update the parent
                    curr_parent = match new_level.cmp(&curr_level) {
                        // We need to search upward to find another parent (we know one must exist)
                        // NOTE: This wouldn't work for `Ordering::Greater` because we don't have anything at that
                        // level yet
                        Ordering::Less => document.get_last_node_at_level(new_level - 1).unwrap(),
                        // The current node should become the new parent (this will return the current node)
                        // BUG: This panics when we have child headings with no parents?
                        Ordering::Greater => document.get_last_node_at_level(curr_level).unwrap(),
                        // The parent stays the same, we don't have to do anything
                        Ordering::Equal => curr_parent,
                    };
                }

                // Skip the rest of the parsing and move onto the planning lines
                loc = ParseLocation::Planning;
                i += 1;
                continue;
            }

            let trimmed_line = line.trim();
            match loc {
                // Root node (`curr_parent`) from Org
                ParseLocation::OrgStart(ref mut start_loc) => match start_loc {
                    // We look for properties first in Org
                    OrgStartLocation::Beginning => {
                        if trimmed_line == format.get_properties_opener() {
                            *start_loc = OrgStartLocation::Properties;
                        } else if !trimmed_line.is_empty() {
                            *start_loc = OrgStartLocation::Content;
                            // Stay on this line for the next parse (i.e. skip the incrementation)
                            continue;
                        }
                        // NOTE: Empty lines at the start of a document are skipped
                    }
                    OrgStartLocation::Properties => {
                        if trimmed_line == format.get_properties_closer() {
                            // Attributes are line-marked and after the properties
                            *start_loc = OrgStartLocation::Content;
                        } else if !trimmed_line.is_empty() {
                            // Parse this property
                            curr_parent.properties.add_line(trimmed_line)?;
                        }
                    }
                    // Special attribute checking is done on the *untrimmed* lines because
                    // there should be no additional spacing before the attributes
                    OrgStartLocation::Content => {
                        if line.starts_with("#+") {
                            // Only push a newline if there was something beforehand (avoids
                            // spacing issues)
                            if !document_attributes.is_empty() {
                                document_attributes.push('\n');
                            }
                            document_attributes.push_str(line);
                        } else {
                            // Obviously we want to preserve spacing here
                            curr_body.push(line);
                        }
                    }
                },
                // Root node (`curr_parent`) from Markdown
                ParseLocation::MarkdownStart(ref mut start_loc) => match start_loc {
                    // At the beginning of a Markdown document, we might have either properties or
                    // frontmatter (or just content), which we can only parse if we have the
                    // appropriate features enabled (gated because the dependencies are a bit heavy).
                    //
                    // Unlike in the Org parser, we'll come back here after frontmatter parsing. We
                    // check that there are no frontmatter lines before we start to parse a
                    // frontmatter block to prevent the strange case of multiple frontmatter
                    // blocks, which *could* be in different formats, leading to parsing errors.
                    // Better to just treat them as part of the body content.
                    MarkdownStartLocation::Beginning {
                        have_seen_frontmatter,
                    } => {
                        if (trimmed_line == "---" || trimmed_line == "+++")
                            && !*have_seen_frontmatter
                        {
                            // Make sure we get the delimiter in there
                            document_attributes.push_str(line);
                            *start_loc = MarkdownStartLocation::Frontmatter;
                        } else if trimmed_line == format.get_properties_opener() {
                            // Nullify any newlines that might have been recorded between the
                            // frontmatter and the properties, or before the properties (see below)
                            curr_body = Vec::new();
                            *start_loc = MarkdownStartLocation::Properties;
                        } else if !trimmed_line.is_empty() {
                            *start_loc = MarkdownStartLocation::Content;
                            // Stay on this line for the next parse (i.e. skip the incrementation)
                            continue;
                        } else if *have_seen_frontmatter {
                            // We have an empty line, and we've seen the frontmatter already. This
                            // could either be a newline between the frontmatter and properties
                            // (which we'll ignore), or one between the frontmatter and content,
                            // which we do care about. We'll record this as the latter, and if we
                            // reach the properties, we'll just nullify it.
                            curr_body.push("");
                        }

                        // NOTE: Spaces at the start of the document, and between the frontmatter
                        // and properties will not be preserved.
                    }
                    // We support both TOML and YAML syntax (no proper parsing, only superficial)
                    MarkdownStartLocation::Frontmatter => {
                        if trimmed_line == "---" || trimmed_line == "+++" {
                            // Go back to the beginning (we might have content or properties next)
                            //
                            // This won't create multiple frontmatter blocks, there's a check
                            // against that above
                            document_attributes.push('\n');
                            document_attributes.push_str(line);
                            *start_loc = MarkdownStartLocation::Beginning {
                                have_seen_frontmatter: true,
                            };
                        } else {
                            // Fine to always add a newline, there is guaranteed to be the
                            // frontmatter fence at the start
                            document_attributes.push('\n');
                            document_attributes.push_str(line);
                        }
                    }
                    MarkdownStartLocation::Properties => {
                        if trimmed_line == format.get_properties_closer() {
                            // Frontmatter must come before the properties, so we've reached the
                            // end
                            *start_loc = MarkdownStartLocation::Content;
                        } else if !trimmed_line.is_empty() {
                            // Parse this property
                            curr_parent.properties.add_line(trimmed_line)?;
                        }
                    }
                    // There are no attributes to check, or anything else
                    MarkdownStartLocation::Content => curr_body.push(line),
                },
                // Planning items directly underneath a heading
                ParseLocation::Planning => {
                    if trimmed_line == format.get_properties_opener() {
                        // Move on to the properties, planning lines are definitely finished
                        loc = ParseLocation::Properties
                    } else if let Some(res) = curr_node.planning.add_line(line) {
                        let _ = res?;
                        // If we got here, the planning line has been parsed without errors, so we can
                        // happily move on
                    } else {
                        // We got something that wasn't a planning line, and we aren't going into
                        // the properties, so we'll start parsing the body of the node from now
                        loc = ParseLocation::Body;
                        // We want to parse this line again
                        continue;
                    }
                }
                // Properties that come after planning
                ParseLocation::Properties => {
                    if trimmed_line == format.get_properties_closer() {
                        loc = ParseLocation::Body;
                    } else if !trimmed_line.is_empty() {
                        // Parse this property
                        curr_node.properties.add_line(trimmed_line)?;
                    }
                }
                // The body of a non-root node (detection of new nodes happens above, so this
                // is trivial)
                ParseLocation::Body => curr_body.push(line),
            }

            // Manual incrementation so we can jump
            i += 1;
        }

        // Finalise the body
        finish_body(
            &mut curr_body,
            if loc.is_start() {
                &mut curr_parent.body
            } else {
                &mut curr_node.body
            },
        );

        // If we got into any nodes, add the last one to its parent
        if !loc.is_start() {
            curr_parent.add_child(curr_node)?;
        }

        // Segmented to avoid double mutable borrows
        document.attributes = document_attributes;
        Ok(document)
    }
}

/// The type of location we're at in the parsing process.
///
/// This doesn't account for headings because they're only ever one line, and because
/// we track the current node actively and separately.
enum ParseLocation {
    OrgStart(OrgStartLocation),
    MarkdownStart(MarkdownStartLocation),
    Body,
    Planning,
    Properties,
}
impl ParseLocation {
    fn is_start(&self) -> bool {
        match &self {
            Self::OrgStart(_) | Self::MarkdownStart(_) => true,
            _ => false,
        }
    }
}

/// Where in the root node of a document parsed from Org mode we are.
enum OrgStartLocation {
    /// No parsing has occurred yet, we don't know where we are!
    Beginning,
    /// The properties section. This should come first in the document, if at all.
    Properties,
    /// Other content in the root node. Once we're here, we can't go back.
    ///
    /// This will search for the special attributes `#+title` and `#+filetags` and
    /// use them both, on their first occurrences, to form the title and tags of the
    /// document. These will be repositioned in rewriting to the start of the document.
    Content,
}
/// Where in the root node of a document parsed from Markdown we are.
enum MarkdownStartLocation {
    /// No parsing has occurred yet, we don't know where we are!
    Beginning {
        /// Whether or not we've seen the frontmatter yet. We'll return to this section of the
        /// parser afterward so we can find either the properties or the content as appropriate.
        have_seen_frontmatter: bool,
    },
    /// The frontmatter of the document (this will not be parsed, but the title and tags will be
    /// extracted therefrom).
    Frontmatter,
    /// The properties section. This should come after the frontmatter, if at all.
    Properties,
    /// Other content in the root node. Once we're here, we can't go back.
    ///
    /// Unlike in Org mode, there is no data left to be parsed here, because we have
    /// explicitly-gated frontmatter rather than free line-marker attributes.
    Content,
}

impl Tags {
    /// Parses tags from their string representation.
    pub fn from_str(tags_str: &str) -> Result<Self, ParseError> {
        let tags = tags_str
            .strip_prefix(":")
            .map(|s| s.strip_suffix(":"))
            .flatten();
        if let Some(tags) = tags {
            let tags = tags.split(':').map(|s| s.to_string()).collect();
            Ok(Self { inner: tags })
        } else {
            Err(ParseError::InvalidTags)
        }
    }
}
