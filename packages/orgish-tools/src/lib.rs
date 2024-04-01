mod mark_done;
mod refile;

pub use mark_done::*;
pub use refile::*;

use orgish::{Node, StringId};

/// A type alias for a series of nodes, considered a document *fragment*.
pub type DocumentFragment<K, I = StringId> = Vec<Node<K, I>>;
