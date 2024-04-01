use super::*;
use chrono::{Datelike, Duration, NaiveDate};

macro_rules! test_timestamp {
    ($name:ident, $input:literal $(=> $output:literal)?) => {
        #[test]
        fn $name() {
            let raw = $input;
            let ts = Timestamp::from_str(raw).unwrap();
            #[allow(unused_variables)]
            let expected = raw;
            $( let expected = $output; )?
            assert_eq!(ts.into_string(), expected);
        }
    };
}

test_timestamp!(simple_timestamp_should_work, "<2023-10-19>" => "<2023-10-19 Thu>");
test_timestamp!(simple_timestamp_with_day_should_work, "<2023-10-19 Thu>");
test_timestamp!(timestamp_with_time_no_day_should_work, "<2023-10-19 9:00>" => "<2023-10-19 Thu 09:00>");
test_timestamp!(timestamp_with_time_and_day_should_work, "<2023-10-19 Thu 9:00>" => "<2023-10-19 Thu 09:00>");
test_timestamp!(
    timestamp_with_long_time_and_day_should_work,
    "<2023-10-19 Thu 09:00>"
);
test_timestamp!(timestamp_with_time_range_no_day_should_work, "<2023-10-19 9:00-10:30>" => "<2023-10-19 Thu 09:00-10:30>");
test_timestamp!(
    timestamp_with_time_range_and_day_should_work,
    "<2023-10-19 Thu 09:00-10:30>"
);
test_timestamp!(timestamp_with_repeater_no_day_should_work, "<2023-10-19 +3w>" => "<2023-10-19 Thu +3w>");
test_timestamp!(
    timestamp_with_repeater_and_day_should_work,
    "<2023-10-19 Thu +3w>"
);
test_timestamp!(
    timestamp_with_repeater_day_and_time_should_work,
    "<2023-10-19 Thu 09:00 +3w>"
);
test_timestamp!(
    timestamp_with_all_should_work,
    "<2023-10-19 Thu 09:00-10:30 +6m>"
);
test_timestamp!(redundant_range_timestamp_should_resolve, "<2023-10-19 Thu 09:00>--<2023-10-19 Thu 10:00>" => "<2023-10-19 Thu 09:00-10:00>");
test_timestamp!(
    simple_range_timestamp_should_work,
    "<2023-10-18 Wed>--<2023-10-19 Thu>"
);
test_timestamp!(
    range_timestamp_with_times_should_work,
    "<2023-10-18 Wed 09:00>--<2023-10-19 Thu 10:00>"
);

macro_rules! date {
    ($year:literal, $month:literal, $day:literal) => {
        NaiveDate::from_ymd_opt($year, $month, $day).unwrap()
    };
}

#[test]
fn timestamp_includes_works_for_simple() {
    let simple_date = Timestamp::from_str("<2024-01-01 Mon>").unwrap();
    assert!(simple_date.includes_date(date!(2024, 01, 01)));
    assert!(!simple_date.includes_date(date!(2024, 01, 02)));
    assert!(!simple_date.includes_date(date!(2024, 02, 01)));
    assert!(!simple_date.includes_date(date!(2025, 01, 01)));
}
#[test]
fn timestamp_includes_works_for_simple_range() {
    let simple_date = Timestamp::from_str("<2024-01-01 Mon>--<2024-03-13 Wed>").unwrap();
    assert!(simple_date.includes_date(date!(2024, 01, 01)));
    assert!(simple_date.includes_date(date!(2024, 02, 04)));
    assert!(simple_date.includes_date(date!(2024, 03, 13)));
    assert!(!simple_date.includes_date(date!(2023, 12, 31)));
    assert!(!simple_date.includes_date(date!(2024, 03, 14)));
}

#[test]
fn timestamp_includes_works_for_start_repeat_days() {
    let ts = Timestamp::from_str("<2024-01-01 Mon +4d>").unwrap();
    let day = date!(2024, 01, 01);
    let repeat = Duration::days(4);
    assert!(ts.includes_date(day));
    assert!(ts.includes_date(day + repeat * 600));
    assert!(!ts.includes_date(day - repeat));
    assert!(!ts.includes_date(day + Duration::days(3)));
}
#[test]
fn timestamp_includes_works_for_start_repeat_weeks() {
    let ts = Timestamp::from_str("<2024-01-01 Mon +4w>").unwrap();
    let day = date!(2024, 01, 01);
    let repeat = Duration::weeks(4);
    assert!(ts.includes_date(day));
    assert!(ts.includes_date(day + repeat * 600));
    assert!(!ts.includes_date(day - repeat));
    assert!(!ts.includes_date(day + Duration::days(3)));
}
#[test]
fn timestamp_includes_works_for_start_repeat_months() {
    let ts = Timestamp::from_str("<2024-01-01 Mon +4m>").unwrap();
    assert!(ts.includes_date(date!(2024, 01, 01)));
    assert!(ts.includes_date(date!(2024, 05, 01)));
    assert!(!ts.includes_date(date!(2023, 09, 01)));
    assert!(!ts.includes_date(date!(2024, 02, 01)));
}
#[test]
fn timestamp_includes_works_for_start_repeat_years() {
    let ts = Timestamp::from_str("<2024-01-01 Mon +4y>").unwrap();
    let day = date!(2024, 01, 01);
    assert!(ts.includes_date(day));
    assert!(ts.includes_date(day.with_year(2028).unwrap()));
    assert!(!ts.includes_date(day.with_year(2023).unwrap()));
    assert!(!ts.includes_date(day.with_year(2025).unwrap()));
}

#[test]
fn timestamp_includes_works_for_range_repeat_days() {
    let ts = Timestamp::from_str("<2024-01-01 Mon +4d>--<2024-01-03 Wed>").unwrap();
    let target = date!(2024, 01, 01);
    let day = Duration::days(1);
    assert!(ts.includes_date(target));
    assert!(ts.includes_date(target + day));
    assert!(ts.includes_date(target + day * 2));
    assert!(ts.includes_date(target + day * 5));
    assert!(!ts.includes_date(target + day * 3));
    assert!(!ts.includes_date(target - day * 2));
}
#[test]
fn timestamp_includes_works_for_range_repeat_weeks() {
    let ts = Timestamp::from_str("<2024-01-01 Mon +4w>--<2024-01-03 Wed>").unwrap();
    let target = date!(2024, 01, 01);
    let day = Duration::days(1);
    let week = Duration::weeks(1);
    assert!(ts.includes_date(target));
    assert!(ts.includes_date(target + week * 4));
    assert!(ts.includes_date(target + week * 4 + day));
    assert!(ts.includes_date(target + week * 4 + day * 2));
    assert!(ts.includes_date(target + week * 8));
    assert!(!ts.includes_date(target + week * 3));
    assert!(!ts.includes_date(target + week * 4 + day * 3));
    assert!(!ts.includes_date(target - week * 4));
}
#[test]
fn timestamp_includes_works_for_range_repeat_months_inner() {
    let ts = Timestamp::from_str("<2024-01-01 Mon +4m>--<2024-01-03 Wed>").unwrap();
    assert!(ts.includes_date(date!(2024, 01, 01)));
    assert!(ts.includes_date(date!(2024, 01, 03)));
    assert!(ts.includes_date(date!(2024, 05, 02)));
    assert!(ts.includes_date(date!(2024, 09, 02)));
    assert!(!ts.includes_date(date!(2024, 04, 01)));
    assert!(!ts.includes_date(date!(2024, 05, 04)));
    assert!(!ts.includes_date(date!(2023, 09, 01)));
}
#[test]
fn timestamp_includes_works_for_range_repeat_months_outer() {
    let ts = Timestamp::from_str("<2024-01-04 Mon +4m>--<2024-03-15 Thu>").unwrap();
    assert!(ts.includes_date(date!(2024, 01, 04)));
    assert!(ts.includes_date(date!(2024, 02, 28)));
    assert!(ts.includes_date(date!(2024, 03, 15)));
    assert!(ts.includes_date(date!(2024, 05, 04)));
    assert!(ts.includes_date(date!(2024, 06, 30)));
    assert!(ts.includes_date(date!(2024, 07, 15)));
    assert!(!ts.includes_date(date!(2024, 01, 01)));
    assert!(!ts.includes_date(date!(2024, 05, 03)));
    assert!(!ts.includes_date(date!(2024, 07, 16)));
    assert!(!ts.includes_date(date!(2023, 09, 04)));
}
#[test]
fn timestamp_includes_works_for_range_repeat_years_inner() {
    let ts = Timestamp::from_str("<2024-01-01 Mon +4y>--<2024-08-10 Sat>").unwrap();
    assert!(ts.includes_date(date!(2024, 01, 01)));
    assert!(ts.includes_date(date!(2024, 06, 30)));
    assert!(ts.includes_date(date!(2024, 08, 10)));
    assert!(ts.includes_date(date!(2028, 01, 01)));
    assert!(ts.includes_date(date!(2028, 06, 30)));
    assert!(ts.includes_date(date!(2028, 08, 10)));
    assert!(!ts.includes_date(date!(2024, 08, 11)));
    assert!(!ts.includes_date(date!(2028, 08, 11)));
    assert!(!ts.includes_date(date!(2020, 01, 01)));
}
#[test]
fn timestamp_includes_works_for_range_repeat_years_outer() {
    let ts = Timestamp::from_str("<2024-01-04 Mon +4y>--<2026-07-11 Sat>").unwrap();
    assert!(ts.includes_date(date!(2024, 01, 04)));
    assert!(ts.includes_date(date!(2025, 03, 15)));
    assert!(ts.includes_date(date!(2026, 07, 11)));
    assert!(ts.includes_date(date!(2028, 01, 04)));
    assert!(ts.includes_date(date!(2029, 03, 15)));
    assert!(ts.includes_date(date!(2030, 07, 11)));
    assert!(!ts.includes_date(date!(2024, 01, 01)));
    assert!(!ts.includes_date(date!(2026, 07, 12)));
    assert!(!ts.includes_date(date!(2028, 01, 01)));
    assert!(!ts.includes_date(date!(2030, 07, 12)));
    assert!(!ts.includes_date(date!(2020, 01, 04)));
}

// TODO Add tests for when `after_date < date`
#[test]
fn timestamp_next_date_works_for_days() {
    let ts = Timestamp::from_str("<2024-01-01 Mon +3d>").unwrap();
    assert_eq!(
        ts.get_next_repeat(date!(2024, 01, 04)),
        Some(date!(2024, 01, 04))
    );
    assert_eq!(
        ts.get_next_repeat(date!(2024, 01, 03)),
        Some(date!(2024, 01, 04))
    );
    assert_eq!(
        ts.get_next_repeat(date!(2024, 01, 05)),
        Some(date!(2024, 01, 07))
    );
}
#[test]
fn timestamp_next_date_works_for_weeks() {
    let ts = Timestamp::from_str("<2024-01-01 Mon +3w>").unwrap();
    assert_eq!(
        ts.get_next_repeat(date!(2024, 01, 22)),
        Some(date!(2024, 01, 22))
    );
    assert_eq!(
        ts.get_next_repeat(date!(2024, 01, 03)),
        Some(date!(2024, 01, 22))
    );
    assert_eq!(
        ts.get_next_repeat(date!(2024, 01, 23)),
        Some(date!(2024, 02, 12))
    );
}
#[test]
fn timestamp_next_date_works_for_months() {
    let ts = Timestamp::from_str("<2024-01-02 Tue +3m>").unwrap();
    assert_eq!(
        ts.get_next_repeat(date!(2024, 04, 02)),
        Some(date!(2024, 04, 02))
    );
    assert_eq!(
        ts.get_next_repeat(date!(2024, 01, 03)),
        Some(date!(2024, 04, 02))
    );
    assert_eq!(
        ts.get_next_repeat(date!(2024, 04, 01)),
        Some(date!(2024, 04, 02))
    );
    assert_eq!(
        ts.get_next_repeat(date!(2024, 04, 05)),
        Some(date!(2024, 07, 02))
    );
}
#[test]
fn timestamp_next_date_works_for_years() {
    let ts = Timestamp::from_str("<2024-01-02 Mon +1y>").unwrap();
    assert_eq!(
        ts.get_next_repeat(date!(2025, 01, 02)),
        Some(date!(2025, 01, 02))
    );
    assert_eq!(
        ts.get_next_repeat(date!(2024, 01, 03)),
        Some(date!(2025, 01, 02))
    );
    assert_eq!(
        ts.get_next_repeat(date!(2025, 01, 01)),
        Some(date!(2025, 01, 02))
    );
    assert_eq!(
        ts.get_next_repeat(date!(2025, 01, 05)),
        Some(date!(2026, 01, 02))
    );
}
