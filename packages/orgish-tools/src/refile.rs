use crate::DocumentFragment;
use anyhow::{anyhow, Context};
use orgish::{Document, Format, Keyword, Node, ParseId};

/// Refiles the given document fragment to the specified location. The location should be given as
/// a path, followed by a double colon and the heading path. E.g. `foo/bar/myfile.md::Test
/// Heading::Subheading 1.3`.
///
/// If no heading is provided in the refile target, the nodes will be apppended verbatim to the end
/// of the file.
pub fn refile_to_file<K: Keyword, I: ParseId>(
    nodes: DocumentFragment<K, I>,
    target: String,
    format: Format,
) -> Result<(), anyhow::Error> {
    let mut parts = target.splitn(2, "::");
    let target_path = parts.next().unwrap();
    // It's fine if we don't have this, we'll just append to the end of the file
    let target_heading = parts.next();

    // Parse the target as a document
    let target_contents = std::fs::read_to_string(target_path)
        .with_context(|| "failed to read from refile target")?;
    let mut target_doc = Document::<K, I>::from_str(&target_contents, format)
        .with_context(|| "failed to parse refile target into document")?;

    refile(nodes, target_heading, &mut target_doc)
        .ok_or(anyhow!("refile target not found in document"))?;
    let updated_doc = target_doc.into_string(format);

    std::fs::write(target_path, updated_doc)
        .with_context(|| "failed to write target document updated from refile")?;
    Ok(())
}

/// Refiles the given nodes into the given parsed document. This is a lower-level utility function
/// for library use, whereas [`refile_to_file`] is often more useful for higher-level
/// application-style behaviour when the target document is an arbitrary file not yet in memory.
/// This function performs its underlying behaviour, and takes a target heading as a `::`-delimited
/// list of heading names. If no such path is provided, the given nodes will be added to the end of
/// the document.
///
/// Note that refiling is a level-aware operation, and the levels of the given nodes will be
/// changed to line up with being direct children of the refile target.
pub fn refile<K: Keyword, I: ParseId>(
    nodes: DocumentFragment<K, I>,
    target_heading: Option<&str>,
    target_doc: &mut Document<K, I>,
) -> Option<()> {
    if let Some(target_heading) = target_heading {
        // Loop recursively through the nodes from the root and follow the path from `target_heading`
        let heading_path = target_heading.split("::").collect::<Vec<_>>();
        // This function takes a node known to be in the path, and loops through its children to
        // find the next one (hence it can be painlessly given the root node)
        fn find_heading_path<'n, K: Keyword, I: ParseId>(
            node: &'n mut Node<K, I>,
            mut heading_path: Vec<&str>,
        ) -> Option<&'n mut Node<K, I>> {
            // If we've run out of path, we've got the node!
            let needle = if heading_path.is_empty() {
                return Some(node);
            } else {
                heading_path.remove(0)
            };

            // Perfectly fine to get a mutable reference here, we'll be adding children in a
            // checked manner anyway
            for child in node.unchecked_mut_children() {
                if child.title == *needle {
                    return find_heading_path(child, heading_path);
                }
            }

            // We have more to look for, but we didn't find it
            None
        }

        if let Some(target_node) = find_heading_path(&mut target_doc.root, heading_path) {
            // Refile the nodes underneath this one, setting their levels appropriately
            let refile_level = target_node.level() + 1;
            for mut node in nodes {
                node.unchecked_set_level(refile_level);
                target_node.add_child(node).unwrap();
            }

            Some(())
        } else {
            None
        }
    } else {
        // We don't have a target *within* the document, just append
        for mut node in nodes {
            // We're refiling to level 1, so give each node the correct level
            node.unchecked_set_level(1);
            // This can't fail, we're refiling a tree at level 1 into the root node, which is not
            // only valid, but continuous
            target_doc.root.add_child(node).unwrap();
        }

        Some(())
    }
}
