//! Parsing logic for converting headings into machine-readable representations.

use super::{keyword::Keyword, Node, ParseId, Priority, Tags, Timestamp};
use crate::format::Format;

impl<K: Keyword, I: ParseId> Node<K, I> {
    /// Checks if the given line represents a new heading. If so, this will parse the heading
    /// and return its details as a new [`Node`].
    ///
    /// This takes a format to use for the parsing process.
    pub fn from_heading_str(heading: &str, format: Format) -> Option<Self> {
        if heading.starts_with(format.heading_char()) {
            let mut node = Node::<K, I>::default();
            let mut loc = NodeParseLocation::Stars;

            // The current thing we're parsing (whatever that might be)
            let mut curr = String::new();
            // This is used exclusively (right not) to track the edge case of one-word headings,
            // which aren't accounted for naturally by the pre-title metadata parser
            let mut is_end;

            let mut chars = heading.chars().collect::<Vec<_>>();
            // Add a final space so everything gets reliably parsed
            chars.push(' ');
            let mut i = 0;
            while i < chars.len() {
                is_end = i == chars.len() - 1;

                let c = chars[i];
                let next_c = chars.get(i + 1);

                match loc {
                    NodeParseLocation::Stars => {
                        if c == format.heading_char() {
                            node.level += 1;
                        } else if c == ' ' {
                            // We've finished the heading characters
                            loc = NodeParseLocation::PreTitle(
                                0,
                                KeywordStatus::None,
                                PriorityStatus::None,
                            );
                            curr = String::new();
                        } else {
                            // This is bold text, probably in the root
                            return None;
                        }
                    }
                    NodeParseLocation::PreTitle(
                        ref mut tokens_parsed,
                        ref mut keyword_status,
                        ref mut priority_status,
                    ) => {
                        if c == ' ' {
                            *tokens_parsed += 1;

                            // Parse this token first and interpret it
                            if let Some(keyword) = K::from_str(&curr) {
                                node.keyword = Some(keyword);
                                *keyword_status = KeywordStatus::Definite;
                            } else if let Some(priority) = parse_priority(&curr) {
                                node.priority = Priority(Some(priority));
                                *priority_status = PriorityStatus::Found;
                            } else if !keyword_status.is_ambiguous() {
                                // **For the first run, this is an `else`.**
                                //
                                // Anything that's not a priority and not clearly a keyword in the
                                // pre-title section should be parsed as an ambiguous keyword.
                                // We should only do this if there isn't already an ambiguous keyword
                                // (otherwise we'd override the real keyword with the second word).
                                *keyword_status =
                                    KeywordStatus::Ambiguous(std::mem::take(&mut curr));
                            }

                            // Exhaustive matching gets the compiler to enforce case handling
                            match (keyword_status, priority_status) {
                                // The above conditional prevents this case (because the keyword status
                                // can't start as ambiguous)
                                (KeywordStatus::None, PriorityStatus::None) => unreachable!(),
                                // The first item was a priority, so we can't have a keyword
                                (KeywordStatus::None, PriorityStatus::Found) |
                                // We have everything we need
                                (KeywordStatus::Definite, PriorityStatus::Found) => {
                                    loc = NodeParseLocation::Title;
                                },
                                // We have an explicit priority, so parse the ambiguous keyword as a keyword
                                (KeywordStatus::Ambiguous(potential_kw), PriorityStatus::Found) => {
                                    node.keyword = Some(K::other(potential_kw.to_string()));
                                    loc = NodeParseLocation::Title;
                                },
                                // We have a clear keyword and no priority yet, keep going if we have space
                                //
                                // NOTE: `is_end` isn't necessary here, because the keyword will already have been parsed
                                // for one-word headings
                                (KeywordStatus::Definite, PriorityStatus::None) => if *tokens_parsed >= 2 {
                                    // The first word was a keyword, but the second one *wasn't* a priority,
                                    // so we should backtrack and parse it as part of the title instead.
                                    //
                                    // One extra because we're on the space right now.
                                    i = i - curr.chars().count() - 1;
                                    loc = NodeParseLocation::Title;
                                },
                                // We have something that might be a keyword, keep going if we have spare space
                                //
                                // NOTE: We need to check if we're at the first and last word, because that would require parsing
                                // the ambiguous keyword with the title
                                (KeywordStatus::Ambiguous(potential_kw), PriorityStatus::None) => if *tokens_parsed >= 2 || is_end {
                                    // We have something that might be a keyword, but no priority after it, so we'll
                                    // parse it as part of the title; this means we have to backtrack by *two*
                                    // words (one extra for the space)
                                    i = i - (curr.chars().count() + potential_kw.chars().count() + 2);
                                    loc = NodeParseLocation::Title;
                                },
                            }

                            curr = String::new();
                        } else {
                            curr.push(c);
                        }
                    }
                    NodeParseLocation::Title => {
                        // We block out the same starting character to avoid malicious input from massively
                        // increasing the time the parser runs for (we would be starting internal loops over
                        // and over again if there are continual starting characters, and they're invalid
                        // anyway, so we can use them as sentinels)
                        let is_tag_starter = c == ':'
                            && !matches!(next_c, Some(' '))
                            && !matches!(next_c, Some(':'));
                        let is_timestamp_starter = c == '<'
                            && !matches!(next_c, Some(' '))
                            && !matches!(next_c, Some('<'));

                        // If we find the starting character of either a tag or timestamp, start an inner
                        // loop through the remaining characters until we find a break
                        if is_tag_starter {
                            // We don't add directly to the node because we require tags to come at the end
                            // of the heading
                            let mut tags = Vec::new();
                            let mut tag = String::new();
                            let mut was_valid = true;
                            // We start this from just after the first `:` to avoid accumulating an empty tag.
                            // We critically avoid the final space!
                            for j in (i + 1)..(chars.len() - 1) {
                                let c = chars[j];

                                // Make sure this character is valid in a tag (see https://orgmode.org/manual/Tags.html)
                                if c == ':' {
                                    // We know this is non-empty because of the `is_tag_starter` check
                                    let last_tag = std::mem::take(&mut tag);
                                    tags.push(last_tag);
                                } else if c.is_alphanumeric() || c == '_' || c == '@' {
                                    tag.push(c);
                                } else {
                                    // The validity of this tag is broken, and that implicitly means there are more
                                    // characters after this lot of tags, so abort completely
                                    was_valid = false;
                                    break;
                                }
                            }

                            // Make sure the tags came at the end of the heading before progressing. This is
                            // a valid way to do that because, if `was_valid` is true, then the above loop completed
                            // without breaking, meaning it accumulated tags from the entire rest of the heading.
                            if was_valid {
                                node.tags = Tags { inner: tags };
                                break; // We've parsed the whole heading
                            } else {
                                // Otherwise keep on parsing the title
                                curr.push(c);
                            }
                        } else if is_timestamp_starter {
                            let mut timestamp = String::new();
                            for j in i..chars.len() {
                                let c = chars[j];

                                // Stop when the timestamp ends, but otherwise leave parsing up to the
                                // dedicated timestamp parser
                                if c == '>' {
                                    // We still want that final closing tag
                                    timestamp.push(c);
                                    // Jump forward after the timestamp
                                    i = j + 1;
                                    break;
                                } else {
                                    timestamp.push(c);
                                }
                            }
                            if let Ok(timestamp) = Timestamp::from_str(&timestamp) {
                                node.timestamps.push(timestamp);
                            } else {
                                // It wasn't a valid timestamp, abort all this and keep parsing (the
                                // innter loop will be discarded)
                                curr.push(c); // This pushes `<`
                            }
                        } else {
                            curr.push(c);
                        }
                    }
                }

                // Manual incrementation so we can jump
                i += 1;
            }

            if let NodeParseLocation::Title = loc {
                // Trim the title (spaces before tags and timestamps get accumulated)
                node.title = curr.trim().to_string();
            }

            Some(node)
        } else {
            None
        }
    }
}

/// Checks if the given text is a priority, parsing it if so.
fn parse_priority(text: &str) -> Option<String> {
    let chars = text.chars().collect::<Vec<_>>();
    if !chars.is_empty()
        && chars[0] == '['
        && chars.get(1).is_some_and(|c| *c == '#')
        && chars.last().unwrap() == &']'
    {
        Some(text[2..text.len() - 1].to_string())
    } else {
        None
    }
}

enum NodeParseLocation {
    /// The `*` characters indicating the level of the heading.
    Stars,
    /// The keyword and priority before the actual title starts. This includes
    /// a counter for how many "words" (space-delimited tokens) have been parsed.
    PreTitle(u8, KeywordStatus, PriorityStatus),
    /// The title of the node, including any tags and timestamps.
    Title,
}

/// States the parser can be in while trying to find a keyword in a heading.
enum KeywordStatus {
    /// No keyword has been found yet.
    None,
    /// A keyword has definitely been found, and it parsed as one.
    Definite,
    /// Something was found where a keyword could be expected, but it doesn't
    /// parse as a keyword. It will only be interpreted as one if an explicit
    /// priority is found after it.
    Ambiguous(String),
}
impl KeywordStatus {
    /// Returns whether or not an ambiguous keyword has been found.
    fn is_ambiguous(&self) -> bool {
        match &self {
            Self::Ambiguous(_) => true,
            _ => false,
        }
    }
}

/// States the parser can be in while trying to find a priority.
enum PriorityStatus {
    /// No priority has been found yet.
    None,
    /// An explicit priority has been found.
    Found,
}
