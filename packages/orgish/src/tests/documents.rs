use super::*;

#[test]
fn parser_should_work() {
    let text = r#"#+title: Test Document
#+author: Test

Root

* Heading 1
** Heading 1.1
*** TODO [#B] Task 1 <2023-01-01 Sun>
- Some contents
*** PROJ Project 1 :tag1:
**** TODO Task 1.1 :tag1:tag2:
DEADLINE: <2023-01-01 Sun>
* [#A] Heading 2
:PROPERTIES:
:FOO: bar
:END:"#;
    let document = Document::<CustomKeyword>::from_str(text, Format::Org).unwrap();

    // The easiest way of testing this is to ensure that everything gets rewritten correctly
    assert_eq!(document.into_string(Format::Org), text);
}
#[test]
fn parser_should_skip_empty_lines_at_start() {
    let text = r#"
Hello, world!"#;
    let min_text = "Hello, world!";
    let document = Document::<CustomKeyword>::from_str(text, Format::Org).unwrap();
    assert_eq!(document.into_string(Format::Org), min_text);
}
#[test]
fn parser_should_handle_spacing() {
    // Partially empty node bodies can seriously throw the parser off, so we test a large
    // number of cases automatically here.
    // We have to be mindful that final newlines will be stripped by Rust's `.lines()`
    // method, hence the final ending text
    let format_str = r#"Initial contents.
[BODY]* Pure spacing
[BODY]* Spacing after
Starting text.
[BODY]* Spacing before
[BODY]Ending text.
Final text"#;

    /// Generates combinations of spacing recursively. This is generic over the maximum
    /// number of newlines to test up until.
    ///
    /// There are $(M + 1)^4$ combinations that this function will produce.
    fn generate_combinations<const M: usize>(format_str: &str) -> Vec<String> {
        // Count how many placeholders there are so we can size the vector appropriately
        // for speed (it will get very big!)
        let placeholders_split = format_str.split("[BODY]").collect::<Vec<_>>();
        let num_placeholders = placeholders_split.len() - 1;

        // We'll produce (M+1)^num_placeholders combinations
        let mut combinations = Vec::with_capacity((M + 1).pow(num_placeholders as u32));

        for i in 0..=M {
            // Replace the first occurrence of `[BODY]` with the given number of newlines
            let intermediary = format_str.replacen("[BODY]", &"\n".repeat(i), 1);
            // If we've exhausted all the placeholders, actually start adding to the buffer
            if !intermediary.contains("[BODY]") {
                combinations.push(intermediary);
            } else {
                // Otherwise, keep generating
                combinations.extend(generate_combinations::<M>(&intermediary));
            }
        }

        combinations
    }

    let combinations = generate_combinations::<5>(format_str);
    assert_eq!(combinations.len(), 1296); // Sanity check

    // Now test with every single combination
    for text in combinations {
        let document = Document::<CustomKeyword>::from_str(&text, Format::Org).unwrap();
        let rewritten = document.into_string(Format::Org);

        // Manual display
        if rewritten != text {
            eprintln!("==== Expected ====\n\n{text}\n");
            eprintln!("==== Received ====\n\n{rewritten}\n");
            eprintln!("====    END   ====");
            panic!("spacing test case failed");
        }
    }
}

struct CustomString {
    inner: String,
}
impl Default for CustomString {
    fn default() -> Self {
        Self {
            inner: "PRE!".to_string(),
        }
    }
}
impl ParseString for CustomString {
    type Error = std::convert::Infallible;

    fn from_str(s: String) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        Ok(Self {
            inner: "PRE!".to_string() + &s,
        })
    }
    fn to_string(&self) -> String {
        self.inner.clone()
    }
}

#[test]
fn parser_should_work_with_custom_strings() {
    let text = r#"#+title: Test Document
#+author: Test

Root

* Heading 1

** Heading 1.1
*** TODO [#B] Task 1 <2023-01-01 Sun>
- Some contents
*** PROJ Project 1 :tag1:
**** TODO Task 1.1 :tag1:tag2:
DEADLINE: <2023-01-01 Sun>
* [#A] Heading 2
:PROPERTIES:
:FOO: bar
:END:"#;
    let modified_text = r#"#+title: Test Document
#+author: Test
PRE!
Root

* PRE!Heading 1
PRE!
** PRE!Heading 1.1
*** TODO [#B] PRE!Task 1 <2023-01-01 Sun>
PRE!- Some contents
*** PROJ PRE!Project 1 :tag1:
**** TODO PRE!Task 1.1 :tag1:tag2:
DEADLINE: <2023-01-01 Sun>
* [#A] PRE!Heading 2
:PROPERTIES:
:FOO: PRE!bar
:END:"#;
    let document =
        Document::<CustomKeyword, StringId, CustomString>::from_str(text, Format::Org).unwrap();

    // The easiest way of testing this is to ensure that everything gets rewritten correctly
    assert_eq!(document.into_string(Format::Org), modified_text);
}

#[test]
fn parser_should_handle_org_attrs() {
    let text = r#"#+title: Test Document
#+author: Test
#+filetags: :foo:

Root"#;
    let mut document = Document::<CustomKeyword>::from_str(text, Format::Org).unwrap();
    document.root.title = "Test Documents".to_string();
    document.root.tags.push("bar".to_string());

    assert_eq!(
        text.replace("Test Document", "Test Documents")
            .replace(":foo:", ":foo:bar:"),
        document.into_string(Format::Org)
    );
}

#[test]
fn parser_should_handle_yaml_attrs() {
    let text = r#"---
title: Test Document
author: Test
tags:
- foo
---

Root"#;
    let mut document = Document::<CustomKeyword>::from_str(text, Format::Markdown).unwrap();
    document.root.title = "Test Documents".to_string();
    document.root.tags.push("bar".to_string());

    assert_eq!(
        text.replace("Test Document", "Test Documents")
            .replace("- foo", "- foo\n- bar"),
        document.into_string(Format::Markdown)
    );
}

#[test]
fn parser_should_handle_toml_attrs() {
    let text = r#"+++
title = "Test Document"
author = "Test"
tags = ["foo"]
+++

Root"#;
    let mut document = Document::<CustomKeyword>::from_str(text, Format::Markdown).unwrap();
    document.root.title = "Test Documents".to_string();
    document.root.tags.push("bar".to_string());

    assert_eq!(
        text.replace("Test Document", "Test Documents")
            .replace("[\"foo\"]", "[\"foo\", \"bar\"]"),
        document.into_string(Format::Markdown)
    );
}

#[test]
fn conversion_org_to_md_should_work() {
    let text_md = r#"---
title: Test Document
author: Test
tags:
- foo
custom_prop: hello,world
---

Root"#;
    let text_org = r#"#+title: Test Document
#+author: Test
#+filetags: :foo:
#+custom_prop: hello,world

Root"#;
    let document = Document::<CustomKeyword>::from_str(text_org, Format::Org).unwrap();

    assert_eq!(text_md, document.into_string(Format::Markdown));
}

#[test]
fn conversion_toml_to_org_should_work() {
    let text_md = r#"+++
title = "Test Document"
author = "Test"
tags = ["foo", "bar"]

[some_table]
prop_1 = 5
prop_2 = 2024-05-07
+++

Root"#;
    let text_org = r#"#+title: Test Document
#+author: Test
#+filetags: :foo:bar:
#+some_table: { prop_1 = 5, prop_2 = 2024-05-07 }

Root"#;
    let document = Document::<CustomKeyword>::from_str(text_md, Format::Markdown).unwrap();

    assert_eq!(text_org, document.into_string(Format::Org));
}

#[test]
fn conversion_yaml_to_org_should_work() {
    let text_md = r#"---
title: Test Document
author: Test
tags:
- foo
- bar

some_table:
  prop_1: 5
  prop_2: 2024-05-07
---

Root"#;
    let text_org = r#"#+title: Test Document
#+author: Test
#+filetags: :foo:bar:
#+some_table: prop_1: 5\nprop_2: 2024-05-07

Root"#;
    let document = Document::<CustomKeyword>::from_str(text_md, Format::Markdown).unwrap();

    assert_eq!(text_org, document.into_string(Format::Org));
}
