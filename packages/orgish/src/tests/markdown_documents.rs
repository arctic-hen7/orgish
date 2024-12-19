use super::*;

#[test]
fn parser_should_work_for_md() {
    let text = r#"---
title: Test Document
author: Test
---

Root

# Heading 1
## Heading 1.1
### TODO [#B] Task 1 <2023-01-01 Sun>
- Some contents
### PROJ Project 1 :tag1:
#### TODO Task 1.1 :tag1:tag2:
DEADLINE: <2023-01-01 Sun>
# [#A] Heading 2
<!--PROPERTIES
FOO: bar
-->"#;
    let document = Document::<CustomKeyword>::from_str(text, Format::Markdown).unwrap();

    // The easiest way of testing this is to ensure that everything gets rewritten correctly
    assert_eq!(document.into_string(Format::Markdown), text);
}
#[test]
fn parser_should_work_for_md_with_props() {
    // Note the lack of spacing after the properties (testing this works)
    let text = r#"---
title: Test Document
author: Test
---
<!--PROPERTIES
FOO: bar
-->
Root

# Heading 1
## Heading 1.1
### TODO [#B] Task 1 <2023-01-01 Sun>
- Some contents
### PROJ Project 1 :tag1:
#### TODO Task 1.1 :tag1:tag2:
DEADLINE: <2023-01-01 Sun>
# [#A] Heading 2
<!--PROPERTIES
FOO: bar
-->

Test"#;
    let document = Document::<CustomKeyword>::from_str(text, Format::Markdown).unwrap();

    // The easiest way of testing this is to ensure that everything gets rewritten correctly
    assert_eq!(document.into_string(Format::Markdown), text);
}
