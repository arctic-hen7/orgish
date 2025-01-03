use thiserror::Error;

/// Errors that can occur while parsing a document.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error(transparent)]
    TimestampParseError(#[from] TimestampParseError),

    #[error("attempted to add a child with at a level higher than or equal to its parent ({bad_child_level} <= {parent_level})")]
    InvalidChildLevel {
        parent_level: u8,
        bad_child_level: u8,
    },
    #[error("failed to parse the following line as a property key/value pair: {line}")]
    InvalidProperty { line: String },
    #[error("invalid tags string received (expected it to begin and end with ':')")]
    InvalidTags,
    // This is an error because we can't reproduce both
    #[error("two planning lines with the same keyword were found: {line}")]
    PlanningRepeat { line: String },
    #[error("failed to parse node identifier: {value}")]
    IdParseFailed { value: String },
    #[error("failed to parse string")]
    ParseStringFailed {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("failed to parse yaml frontmatter in markdown document")]
    YamlFrontmatterParseFailed {
        #[source]
        source: serde_yaml::Error,
    },
    #[error("failed to parse toml frontmatter in markdown document")]
    TomlFrontmatterParseFailed {
        #[source]
        source: toml::de::Error,
    },
    #[error("found incomplete attributes (e.g. unclosed frontmatter block)")]
    IncompleteAttributes,
    #[error("found non-string `title` attribute on the document root")]
    RootTitleNotString,
    #[error("found `tags` attribute on the document root that wasn't an array of strings")]
    RootTagsNotStringVec,
}

/// Errors that can occur specifically while parsing timestamps.
#[derive(Debug, Error)]
pub enum TimestampParseError {
    #[error(
        "timestamps must be 12 or more characters long, found alleged timestamp of length {len}"
    )]
    TooShort { len: usize },
    #[error("found alleged timestamp of form '{start}..{end}' (expected '<..>' or '[..]')")]
    InvalidStartEnd { start: char, end: char },
    #[error("alleged timestamp contained non-ascii characters")]
    NotAscii,
    #[error("invalid date component in timestamp, expected `YYYY-MM-dd`, found '{date}'")]
    InvalidDate { date: String },
    #[error("invalid year found in timestamp: '{year}'")]
    InvalidYear { year: String },
    #[error("invalid month found in timestamp: '{month}'")]
    InvalidMonth { month: String },
    #[error("invalid day found in timestamp: '{day}")]
    InvalidDay { day: String },
    #[error("could not construct valid date from date components '{year}-{month}-{day}'")]
    InvalidDateComponents { year: i32, month: u32, day: u32 },
    #[error("found range timestamp `<..>--<..>` with ranges inside")]
    RangeInRange { timestamp: String },
    #[error("found unexpected character '{c}' in timestamp")]
    BadCharacter { c: char },
    #[error("found a day name in a timestamp that was more than three characters (had '{current}', but then found '{next_c}')")]
    DayNameTooLong { current: String, next_c: char },
    #[error("foud invalid repeater unit '{c}' in timestamp (expected d/w/m/y)")]
    BadRepeaterUnit { c: char },
    #[error("found invalid time in timestamp: '{time_str}'")]
    InvalidTime {
        time_str: String,
        #[source]
        source: chrono::ParseError,
    },
}
