use crate::DocumentFragment;
use chrono::NaiveDateTime;
use orgish::{timestamp::DateTime, Keyword, Node, ParseId, Timestamp};

/// Marks all top-level nodes as done in the given document. This takes a keyword to be interpreted
/// as `DONE`, meaning it can be used to convert nodes into other states like Org's traditional
/// `KILL`. The main thing this function does is handles timestamps with repeaters by moving them
/// to their next iteration.
///
/// The keywords repesenting the `DONE` state may be different for nodes that repeat and those that
/// don't. (E.g. repeating nodes may return to `TODO`.)
///
/// This also takes a date relative to which the timestamps will be updated (i.e. their next repeat
/// after this date will be inserted, for those which repeat). Non-repeating timestamps in nodes
/// that have one or more repeating timestamps will be removed outright.
///
/// For repeating nodes, the `LAST_REPEAT` node will automatically be set if `completion_time` is
/// provided.
pub fn mark_nodes_done<K: Keyword + Clone, I: ParseId + Clone>(
    nodes: DocumentFragment<K, I>,
    new_keyword_repeating: K,
    new_keyword_not_repeating: K,
    completion_time: Option<NaiveDateTime>,
) -> Vec<CompletedNode<K, I>> {
    // Go through all the top-level nodes (any underneath won't be changed, they'll be
    // left entirely alone)
    let mut annotated_nodes = Vec::new();
    for mut node in nodes {
        // If the node repeats, we might need to put it in two places
        let mut repeating_node = node.clone();
        // This is all we need to do if it doesn't repeat
        node.keyword = Some(new_keyword_not_repeating.clone());

        // If any of the timestamps in the node repeats (deadline, scheduled time, anything), we
        // should keep this node around. All non-repeating timestamps will be axed.
        let mut has_repeating_ts = false;
        repeating_node.timestamps = repeating_node
            .timestamps
            .into_iter()
            .map(|ts| ts.into_next_repeat())
            .filter_map(|ts_opt| ts_opt.ok())
            .collect();
        if !repeating_node.timestamps.is_empty() {
            has_repeating_ts = true;
        }
        repeating_node.planning.deadline = repeating_node
            .planning
            .deadline
            .map(|ts| ts.into_next_repeat().ok())
            .flatten();
        if repeating_node.planning.deadline.is_some() {
            has_repeating_ts = true;
        }
        repeating_node.planning.scheduled = repeating_node
            .planning
            .scheduled
            .map(|ts| ts.into_next_repeat().ok())
            .flatten();
        if repeating_node.planning.scheduled.is_some() {
            has_repeating_ts = true;
        }
        repeating_node.planning.closed = repeating_node
            .planning
            .closed
            .map(|ts| ts.into_next_repeat().ok())
            .flatten();
        if repeating_node.planning.closed.is_some() {
            has_repeating_ts = true;
        }

        // If this node has at least one repeating timestamp, push a version of it with that
        // timestamp progressed to its next repeat and all other, non-repeating timestamps removed.
        //
        // Otherwise, add the node with everything intact, but its keyword changed to `DONE`,
        if has_repeating_ts {
            repeating_node.keyword = Some(new_keyword_repeating.clone());
            // If we have a completion time, set `LAST_REPEAT`
            if let Some(completion_time) = completion_time {
                repeating_node.properties.insert(
                    "LAST_REPEAT".to_string(),
                    // This is a string timestamp, and won't be parsed by Orgish automatically, so
                    // it won't interfere with anything
                    Timestamp {
                        start: DateTime {
                            date: completion_time.date(),
                            time: Some(completion_time.time()),
                        },
                        end: None,
                        repeater: None,
                        active: false,
                    }
                    .into_string(),
                );
            }
            annotated_nodes.push(CompletedNode::Repeating {
                completed: node,
                repeating: repeating_node,
            });
        } else {
            annotated_nodes.push(CompletedNode::Done(node));
        }
    }

    annotated_nodes
}

/// A representation of a completed node.
pub enum CompletedNode<K: Keyword, I: ParseId> {
    /// The node is completed and does not repeat.
    Done(Node<K, I>),
    /// The node repeats.
    Repeating {
        /// The completed version of the node.
        completed: Node<K, I>,
        /// The repeating version of the node.
        repeating: Node<K, I>,
    },
}
