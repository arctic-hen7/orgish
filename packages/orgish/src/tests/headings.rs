use super::*;

#[test]
fn heading_parser_should_work() {
    let heading = r#"** TODO [#A] Foo bar <2023-05-09 Tue> :test1:test2:"#;
    let node = Node::<CustomKeyword>::from_heading_str(&heading, Format::Org);
    assert!(node.is_some());
    let node = node.unwrap().unwrap();

    assert_eq!(node.level, 2);
    assert_eq!(node.title, "Foo bar");
    assert_eq!(node.keyword, Some(CustomKeyword::Todo));
    assert_eq!(node.priority, Priority(Some("A".to_string())));
    assert_eq!(node.tags.inner, vec!["test1", "test2"]);
    // TODO Proper timestamp assertion
    assert!(!node.timestamps.is_empty());

    assert_eq!(node.into_string(Format::Org), heading);
}
#[test]
fn heading_parser_should_parse_simple() {
    let heading = "* Foo bar";
    let node = Node::<CustomKeyword>::from_heading_str(&heading, Format::Org);
    assert!(node.is_some());
    let node = node.unwrap().unwrap();

    assert_eq!(node.level, 1);
    assert_eq!(node.priority, Priority(None));
    assert_eq!(node.keyword, None);
    assert_eq!(node.title, "Foo bar");

    assert_eq!(node.into_string(Format::Org), heading);
}
#[test]
fn heading_parser_should_parse_with_keyword() {
    let heading = "* PROJ Test";
    let node = Node::<CustomKeyword>::from_heading_str(&heading, Format::Org);
    assert!(node.is_some());
    let node = node.unwrap().unwrap();

    assert_eq!(node.level, 1);
    assert_eq!(node.priority, Priority(None));
    assert_eq!(node.keyword, Some(CustomKeyword::Proj));
    assert_eq!(node.title, "Test");

    assert_eq!(node.into_string(Format::Org), heading);
}
#[test]
fn heading_parser_should_parse_with_priority() {
    let heading = "* [#A] Test";
    let node = Node::<CustomKeyword>::from_heading_str(&heading, Format::Org);
    assert!(node.is_some());
    let node = node.unwrap().unwrap();

    assert_eq!(node.level, 1);
    assert_eq!(node.priority, Priority(Some("A".to_string())));
    assert_eq!(node.keyword, None);
    assert_eq!(node.title, "Test");

    assert_eq!(node.into_string(Format::Org), heading);
}
#[test]
fn heading_parser_should_parse_with_keyword_and_priority() {
    let heading = "* PROJ [#A] Test";
    let node = Node::<CustomKeyword>::from_heading_str(&heading, Format::Org);
    assert!(node.is_some());
    let node = node.unwrap().unwrap();

    assert_eq!(node.level, 1);
    assert_eq!(node.priority, Priority(Some("A".to_string())));
    assert_eq!(node.keyword, Some(CustomKeyword::Proj));
    assert_eq!(node.title, "Test");

    assert_eq!(node.into_string(Format::Org), heading);
}
#[test]
fn heading_parser_should_parse_with_unknown_keyword_and_priority() {
    let heading = "* BLAH [#A] Test";
    let node = Node::<CustomKeyword>::from_heading_str(&heading, Format::Org);
    assert!(node.is_some());
    let node = node.unwrap().unwrap();

    assert_eq!(node.level, 1);
    assert_eq!(node.priority, Priority(Some("A".to_string())));
    assert_eq!(node.keyword, Some(CustomKeyword::Other("BLAH".to_string())));
    assert_eq!(node.title, "Test");

    assert_eq!(node.into_string(Format::Org), heading);
}
#[test]
fn heading_parser_should_parse_unknown_keyword_without_priority_in_title() {
    let heading = "* BLAH Test"; // Important that this stays as two words (priority parsing)!
    let node = Node::<CustomKeyword>::from_heading_str(&heading, Format::Org);
    assert!(node.is_some());
    let node = node.unwrap().unwrap();

    assert_eq!(node.level, 1);
    assert_eq!(node.priority, Priority(None));
    assert!(node.keyword.is_none());
    assert_eq!(node.title, "BLAH Test");

    assert_eq!(node.into_string(Format::Org), heading);
}
#[test]
fn heading_parser_should_parse_pure_keyword() {
    let heading = "* TODO";
    let node = Node::<CustomKeyword>::from_heading_str(&heading, Format::Org);
    assert!(node.is_some());
    let node = node.unwrap().unwrap();

    assert_eq!(node.level, 1);
    assert_eq!(node.priority, Priority(None));
    assert_eq!(node.keyword, Some(CustomKeyword::Todo));
    assert_eq!(node.title, "");

    assert_eq!(node.into_string(Format::Org), heading);
}
#[test]
fn heading_parser_should_parse_pure_priority() {
    let heading = "* [#A]";
    let node = Node::<CustomKeyword>::from_heading_str(&heading, Format::Org);
    assert!(node.is_some());
    let node = node.unwrap().unwrap();

    assert_eq!(node.level, 1);
    assert_eq!(node.priority, Priority(Some("A".to_string())));
    assert!(node.keyword.is_none());
    assert_eq!(node.title, "");

    assert_eq!(node.into_string(Format::Org), heading);
}
#[test]
fn heading_parser_should_parse_pure_keyword_and_priority() {
    let heading = "* TODO [#A]";
    let node = Node::<CustomKeyword>::from_heading_str(&heading, Format::Org);
    assert!(node.is_some());
    let node = node.unwrap().unwrap();

    assert_eq!(node.level, 1);
    assert_eq!(node.priority, Priority(Some("A".to_string())));
    assert_eq!(node.keyword, Some(CustomKeyword::Todo));
    assert_eq!(node.title, "");

    assert_eq!(node.into_string(Format::Org), heading);
}
#[test]
fn heading_parser_should_parse_pure_unknown_keyword_and_priority() {
    let heading = "* BLAH [#A]";
    let node = Node::<CustomKeyword>::from_heading_str(&heading, Format::Org);
    assert!(node.is_some());
    let node = node.unwrap().unwrap();

    assert_eq!(node.level, 1);
    assert_eq!(node.priority, Priority(Some("A".to_string())));
    assert_eq!(node.keyword, Some(CustomKeyword::Other("BLAH".to_string())));
    assert_eq!(node.title, "");

    assert_eq!(node.into_string(Format::Org), heading);
}
#[test]
fn heading_parser_should_count_early_tags() {
    let heading = "* Test :test1:test2:";
    let node = Node::<CustomKeyword>::from_heading_str(&heading, Format::Org);
    assert!(node.is_some());
    let node = node.unwrap().unwrap();

    assert_eq!(node.level, 1);
    assert_eq!(node.priority, Priority(None));
    assert!(node.keyword.is_none());
    assert_eq!(node.tags.inner, vec!["test1", "test2"]);
    assert_eq!(node.title, "Test");

    assert_eq!(node.into_string(Format::Org), heading);
}
#[test]
fn heading_parser_should_count_early_timestamp() {
    let heading = "* Test <2023-01-01 Sun>";
    let node = Node::<CustomKeyword>::from_heading_str(&heading, Format::Org);
    assert!(node.is_some());
    let node = node.unwrap().unwrap();

    assert_eq!(node.level, 1);
    assert_eq!(node.priority, Priority(None));
    assert!(node.keyword.is_none());
    assert!(!node.timestamps.is_empty()); // TODO Proper valdiation
    assert_eq!(node.title, "Test");

    assert_eq!(node.into_string(Format::Org), heading);
}
#[test]
fn heading_parser_should_fail_on_non_heading() {
    let bad_heading = " ** Test";
    let node = Node::<CustomKeyword>::from_heading_str(&bad_heading, Format::Org);

    assert!(node.is_none());
}
// Ironically, this is the edge case in our parser implementation
#[test]
fn heading_parser_should_parse_single_word_title() {
    let heading = "* Test";
    let node = Node::<CustomKeyword>::from_heading_str(&heading, Format::Org);
    assert!(node.is_some());
    let node = node.unwrap().unwrap();

    assert_eq!(node.level, 1);
    assert_eq!(node.priority, Priority(None));
    assert_eq!(node.keyword, None);
    assert_eq!(node.title, "Test");

    assert_eq!(node.into_string(Format::Org), heading);
}
