//! Logic for handling Org mode-style timestamps and their parsing. This is designed to
//! be as comprehensive as possible, and includes logic for handling user inputs for
//! creating new timestamps relative to a given date.

use super::error::TimestampParseError;
use chrono::{Datelike, Duration, NaiveDate, NaiveTime};

/// An abstraction over dates and times where the times are optional.
#[derive(Debug, Clone)]
pub struct DateTime {
    pub date: NaiveDate,
    pub time: Option<NaiveTime>,
}
/// The repeater in a timestamp (e.g. `+1w`).
#[derive(Debug, Clone)]
pub struct Repeater {
    pub count: usize,
    pub unit: RepeaterUnit,
}
impl Repeater {
    /// Converts this repeater into its mode representation (e.g. `+10d`).
    fn into_string(self) -> String {
        format!("+{}{}", self.count, self.unit.into_char())
    }
}
/// The different units for repeaters.
// TODO Org's documentation doesn't list all possible repeaters, so I am literally
// guessing here given I can't look at the source!
#[derive(Debug, Clone, Copy)]
pub enum RepeaterUnit {
    Day,
    Week,
    Month,
    Year,
}
impl RepeaterUnit {
    /// Converts the given character into a repeater unit if possible.
    fn from_char(c: char) -> Option<Self> {
        match c {
            'd' => Some(Self::Day),
            'w' => Some(Self::Week),
            'm' => Some(Self::Month),
            'y' => Some(Self::Year),
            _ => None,
        }
    }
    /// Converts this repeater unit into the corresponding mode character used to
    /// represent it.
    fn into_char(self) -> char {
        match self {
            Self::Day => 'd',
            Self::Week => 'w',
            Self::Month => 'm',
            Self::Year => 'y',
        }
    }
}

/// An Org mode-style timestamp.
///
/// # Information loss
///
/// Parsing then writing a timestamp will never lead to information loss, however it
/// may lead to reformatting. For example, the range timestamp
/// `<2023-01-01 Sun 9:00>--<2023-01-01 Sun 10:00>` would be, because the start and end
/// are on the same day, be simplified when the timestamp is written back to a string as
/// `<2023-01-01 Sun 9:00-10:00>`.
#[derive(Debug, Clone)]
pub struct Timestamp {
    /// The date (and optional time) that the timestamp begins at. If it has only one
    /// datetime entry, that will be considered as the start.
    pub start: DateTime,
    /// An optional ending date, with a further optional time.
    pub end: Option<DateTime>,
    /// An expression indicating how, if at all, the timestamp should repeat over time.
    pub repeater: Option<Repeater>,
    /// Whether or not the timestamp is active.
    pub active: bool,
}
impl Timestamp {
    /// Returns whether or not this timestamp, or any of its subsequent repeats, falls on the given date.
    pub fn includes_date(&self, date: NaiveDate) -> bool {
        if let Some(repeater) = &self.repeater {
            match repeater.unit {
                RepeaterUnit::Day => {
                    // For checking if the day is a repeat, just turn the end and target dates into
                    // a number of days since the start date, and then check if the target is in
                    // the range (taking the modulus of the frequency)
                    let end = self
                        .end
                        .as_ref()
                        .map(|dt| (dt.date - self.start.date).num_days());
                    let target = (date - self.start.date).num_days();
                    // This makes sure everything is positibe (i.e. the first repeat has happened
                    // relative to the target date)
                    in_range_mod(target, (0, end), repeater.count).0
                }
                RepeaterUnit::Week => {
                    // Same as for the days, but multiply the frequency in weeks by 7 to get days
                    // (avoid floats)
                    let end = self
                        .end
                        .as_ref()
                        .map(|dt| (dt.date - self.start.date).num_days());
                    let target = (date - self.start.date).num_days();
                    in_range_mod(target, (0, end), repeater.count * 7).0
                }
                RepeaterUnit::Month => {
                    // To check months, we'll convert the target and end dates to a number of
                    // months since the start date (by converting years to months and then normalising
                    // with the start as 0). We use signed integers to prevent overflows if the target
                    // is before the first repeat.
                    let start_months =
                        (self.start.date.year_ce().1 * 12 + self.start.date.month0()) as i64;
                    let end_months = self
                        .end
                        .as_ref()
                        .map(|end| (end.date.year_ce().1 * 12 + end.date.month0()) as i64);
                    let target_months = (date.year_ce().1 * 12 + date.month0()) as i64;

                    let (in_range, normalised_month) = in_range_mod(
                        target_months - start_months,
                        (0, end_months.map(|n| n - start_months)),
                        repeater.count,
                    );
                    if in_range {
                        if normalised_month == 0 {
                            // Starting month, check if the date is after the given start date
                            let start_day = self.start.date.day0();
                            let target_day = date.day0();
                            if end_months.is_some_and(|end| (end - start_months) as u64 == 0) {
                                // The start and end months are the same, make sure we check the end date as well
                                let end_day = self.end.as_ref().unwrap().date.day0();
                                start_day <= target_day && target_day <= end_day
                            } else {
                                start_day <= target_day
                            }
                        } else if end_months
                            .is_some_and(|end| (end - start_months) as u64 == normalised_month)
                        {
                            // NOTE: The above conversion to `u64` can't panic because `in_range` can only be `true`
                            // if all arguments to `in_range_mod()` were positive.
                            // Ending month, check if the date is before the given end date (note that the start and
                            // end months can't be the same here, otherwise `normalised` would be 0).
                            let end_day = self.end.as_ref().unwrap().date.day0();
                            let target_day = date.day0();
                            target_day <= end_day
                        } else {
                            // In between month, the date can be anything
                            true
                        }
                    } else {
                        // The month isn't right, no point in checking the date
                        false
                    }
                }
                RepeaterUnit::Year => {
                    // Very similar approach to the months, except we check month and day at the
                    // same time using `month0 * 100 + day0`, which is like an ordinal except it
                    // works with leap years
                    let start_years = self.start.date.year_ce().1 as i64;
                    let end_years = self.end.as_ref().map(|end| end.date.year_ce().1 as i64);
                    let target_years = date.year_ce().1 as i64;

                    let (in_range, normalised_year) = in_range_mod(
                        target_years - start_years,
                        (0, end_years.map(|n| n - start_years)),
                        repeater.count,
                    );
                    if in_range {
                        if normalised_year == 0 {
                            // Starting month, check if the date is after the given start date
                            let start_ordinal =
                                self.start.date.month0() * 100 + self.start.date.day0();
                            let target_ordinal = date.month0() * 100 + date.day0();
                            if end_years.is_some_and(|end| (end - start_years) as u64 == 0) {
                                // The start and end years are the same, make sure we check the end date as well
                                let end_date = self.end.as_ref().unwrap().date;
                                let end_ordinal = end_date.month0() * 100 + end_date.day0();
                                start_ordinal <= target_ordinal && target_ordinal <= end_ordinal
                            } else {
                                start_ordinal <= target_ordinal
                            }
                        } else if end_years
                            .is_some_and(|end| (end - start_years) as u64 == normalised_year)
                        {
                            // NOTE: The above conversion to `u64` can't panic because `in_range` can only be `true`
                            // if all arguments to `in_range_mod()` were positive
                            // Ending month, check if the date is before the given end date
                            let end_date = self.end.as_ref().unwrap().date;
                            let end_ordinal = end_date.month0() * 100 + end_date.day0();
                            let target_ordinal = date.month0() * 100 + date.day0();
                            target_ordinal <= end_ordinal
                        } else {
                            // In between month, the date can be anything
                            true
                        }
                    } else {
                        // The month isn't right, no point in checking the date
                        false
                    }
                }
            }
        } else {
            // Without a repeater, we just have this range
            if let Some(end) = &self.end {
                self.start.date <= date && end.date >= date
            } else {
                date == self.start.date
            }
        }
    }
    /// Returns when this timestamp occurs relative to the given date, not regarding repeaters. See
    /// [`TimestampWhen`] for details.
    pub fn when(&self, date: NaiveDate) -> TimestampWhen {
        if let Some(end) = &self.end {
            if date < self.start.date {
                TimestampWhen::Future
            } else if end.date < date {
                TimestampWhen::Past
            } else {
                TimestampWhen::Present
            }
        } else {
            // We have a single date
            if date < self.start.date {
                TimestampWhen::Future
            } else if self.start.date < date {
                TimestampWhen::Past
            } else {
                TimestampWhen::Present
            }
        }
    }
    /// Returns when this timestamp applies to the given date. This is the main method third-party callers
    /// should use when querying about timestamps, as it contains a large deal of information. However, computing
    /// this for range timestamps is slightly more expensive, so [`Self::includes_date`] should be preferred
    /// where the additional information is not required immediately.
    pub fn applies(&self, date: NaiveDate) -> TimestampApplies {
        if !self.includes_date(date) {
            return TimestampApplies::None;
        }

        if let Some(end) = &self.end {
            // We have an end date, if there are no times on either this is simple. If otherwise,
            // we'll construct single-day timestamps (with the same repeaters!) on the start and
            // end dates and see what they give us (this allows returning `true` for days in the
            // middle of timed ranges, and `false` for the ends of such ranges)
            if self.start.time.is_none() && end.time.is_none() {
                TimestampApplies::AllDay
            } else if self.start.date == end.date {
                // If we have a timestamp with start and end dates, we can figure the application
                // out easily
                if self.start.time.is_some() && end.time.is_some() {
                    TimestampApplies::Block(self.start.time.unwrap(), end.time.unwrap())
                } else if self.start.time.is_some() {
                    TimestampApplies::Start(self.start.time.unwrap())
                } else if end.time.is_some() {
                    // This would be expressed by a range timestamp
                    TimestampApplies::End(end.time.unwrap())
                } else {
                    // Redundant, handled by the previous branch, but here to appease the compiler
                    TimestampApplies::AllDay
                }
            } else {
                // We have a range timestamp that goes over multiple days
                let start_only_ts = Timestamp {
                    start: self.start.clone(),
                    end: None,
                    repeater: self.repeater.clone(),
                    active: self.active,
                };
                let end_only_ts = Timestamp {
                    start: end.clone(),
                    end: None,
                    repeater: self.repeater.clone(),
                    active: self.active,
                };

                if start_only_ts.includes_date(date) {
                    // We're on the start date
                    if let Some(time) = self.start.time {
                        TimestampApplies::Start(time)
                    } else {
                        TimestampApplies::AllDay
                    }
                } else if end_only_ts.includes_date(date) {
                    // We're on the end date
                    if let Some(time) = end.time {
                        TimestampApplies::End(time)
                    } else {
                        TimestampApplies::AllDay
                    }
                } else {
                    // We're somewhere in between, where there are only full days
                    TimestampApplies::AllDay
                }
            }
        } else {
            // We just have a start date, this is simple
            if let Some(time) = self.start.time {
                TimestampApplies::Start(time)
            } else {
                TimestampApplies::AllDay
            }
        }
    }
    /// Gets the next date after the given date on which this timestamp will repeat. This is
    /// calculated by advancing the original date of the timestamp by its repeater until a date
    /// after `after_date` is reached.
    ///
    /// This will produce a date for the **starting date only**, the end date should be calculated
    /// manually if necessary by calculating the day difference between the two.
    ///
    /// If this timestamp has no repeat, this will return `None`.
    ///
    /// Importantly, if the repeat is by month, and the day index would fall outside the bounds of
    /// the month (e.g. monthly on the 30th, but we're in February), the next repeat will be used.
    pub fn get_next_repeat(&self, after_date: NaiveDate) -> Option<NaiveDate> {
        let date = self.start.date;
        // If this date is before the first date, then our first repeat is that date
        if after_date < date {
            return Some(date);
        }

        let repeater = self.repeater.as_ref()?;
        match repeater.unit {
            RepeaterUnit::Day => {
                // Guaranteed to be positive because of the earlier sanity check
                let days_diff = (after_date - date).num_days() as usize;
                // If we're on a repeat, today is the day
                if days_diff % repeater.count == 0 {
                    Some(after_date)
                } else {
                    // We'll increment by one more than the number of completed repeats
                    let num_completed_repeats = days_diff / repeater.count;
                    let next_date = date
                        + Duration::try_days((repeater.count * (num_completed_repeats + 1)) as i64)
                            .unwrap();
                    Some(next_date)
                }
            }
            RepeaterUnit::Week => {
                let repeater_days_count = repeater.count * 7;
                // Guaranteed to be positive because of the earlier sanity check
                let days_diff = (after_date - date).num_days() as usize;
                // If we're on a repeat, today is the day
                if days_diff % repeater_days_count == 0 {
                    Some(after_date)
                } else {
                    // We'll increment by one more than the number of completed repeats
                    let num_completed_repeats = days_diff / repeater_days_count;
                    let next_date = date
                        + Duration::try_days(
                            (repeater_days_count * (num_completed_repeats + 1)) as i64,
                        )
                        .unwrap();
                    Some(next_date)
                }
            }
            RepeaterUnit::Month => {
                // Resolve both dates to a number of months since year 0, ignoring the day for now
                let date_months = date.year_ce().1 * 12 + date.month0();
                let after_date_months = after_date.year_ce().1 * 12 + after_date.month0();
                // Again, guaranteed to be positive
                let months_diff = after_date_months - date_months;

                // Calculate the next repeat assuming we're between repeats, this will be used in
                // several cases
                let next_months =
                    date_months + (months_diff / repeater.count as u32 + 1) * repeater.count as u32;
                let next_date = NaiveDate::from_ymd_opt(
                    next_months as i32 / 12,
                    next_months % 12 + 1,
                    date.day(),
                )
                .unwrap();
                if months_diff % repeater.count as u32 == 0 {
                    // We're in the right month, but now the day index matters
                    if after_date.day0() > date.day0() {
                        // We're after the repeat in this month, go to the next repeat
                        Some(next_date)
                    } else if after_date.day0() < date.day0() {
                        // We're before the repeat in this month, set the next repeat to `after_date`
                        // with `date`'s day index
                        // This could fail if the day index is outside this month's bounds, in which
                        // case we'll move on to the next repeat.
                        Some(after_date.with_day0(date.day0()).unwrap_or(next_date))
                    } else {
                        // We're on precisely the right day
                        Some(after_date)
                    }
                } else {
                    Some(next_date)
                }
            }
            RepeaterUnit::Year => {
                // Resolve both dates to a number of year since year 0, ignoring the ordinal
                let date_years = date.year_ce().1;
                let after_date_years = after_date.year_ce().1;
                // Again, guaranteed to be positive
                let years_diff = after_date_years - date_years;

                // Calculate the next repeat assuming we're between repeats, this will be used in
                // several cases
                let next_years =
                    date_years + (years_diff / repeater.count as u32 + 1) * repeater.count as u32;
                let next_date =
                    NaiveDate::from_ymd_opt(next_years as i32, date.month(), date.day()).unwrap();
                if years_diff % repeater.count as u32 == 0 {
                    // We're in the right year, but now the month and day matter (we can't use ordinals
                    // because of leap years); this is a simple way of doing the comparisons all in one
                    if after_date.month0() * 100 + after_date.day0()
                        > date.month0() * 100 + date.day0()
                    {
                        // We're after the repeat in this year, go to the next repeat
                        Some(next_date)
                    } else if after_date.month0() * 100 + after_date.day0()
                        < date.month0() * 100 + date.day0()
                    {
                        // We're before the repeat in this year, set the next repeat to `after_date`
                        // with `date`'s month and day. This can't fail because the last representable date is
                        // the last day of a year.
                        //
                        // We have to first assign the date to 0 so we don't try to go into a month
                        // with a date that doesn't exist (e.g. on the 31st, try to go to
                        // November).
                        Some(
                            after_date
                                .with_day0(0)
                                .unwrap()
                                .with_month0(date.month0())
                                .unwrap()
                                .with_day0(date.day0())
                                .unwrap(),
                        )
                    } else {
                        // We're on precisely the right day
                        Some(after_date)
                    }
                } else {
                    Some(next_date)
                }
            }
        }
    }
    /// Converts this timestamp into the next repeat of itself, or its original self if there is no
    /// repeat.
    ///
    /// This will preserve times and handle repeating timestamps that go across multiple dates by
    /// computing the distance between the end date and the start date, and adding this on to the
    /// new start date from [`Self::get_next_repeat`].
    pub fn into_next_repeat_after(self, after_date: NaiveDate) -> Result<Self, Self> {
        // Verbose to avoid later moved value errors
        let next_repeat = match self.get_next_repeat(after_date) {
            Some(r) => r,
            None => return Err(self),
        };

        let next_end = if let Some(end) = self.end {
            Some(DateTime {
                date: next_repeat
                    + Duration::try_days((end.date - self.start.date).num_days()).unwrap(),
                time: end.time,
            })
        } else {
            None
        };

        Ok(Timestamp {
            start: DateTime {
                date: next_repeat,
                time: self.start.time,
            },
            end: next_end,
            repeater: self.repeater,
            active: self.active,
        })
    }
    /// Converts this timestamp into the next repeat of itself. This is the same as calling
    /// [`Self::into_next_repeat_after`] with the date one day after the current date on this
    /// timestamp. In other words, this will always get the immediately next repeat, regardless of
    /// the current date.
    ///
    /// This is useful for mimicking the behaviour of Org mode when an entry is marked as `DONE`
    /// and timestamps need to be progressed to their next repeats (if a deadline has not yet been
    /// reached, it will still need to be progressed).
    pub fn into_next_repeat(self) -> Result<Self, Self> {
        let date_one_after = self.start.date + Duration::try_days(1).unwrap();
        self.into_next_repeat_after(date_one_after)
    }
}
impl Timestamp {
    /// Parses a timestamp from the given string.
    pub fn from_str(raw: &str) -> Result<Self, TimestampParseError> {
        let raw = raw.trim();

        // Handle range timestamps recursively
        if raw.contains("--") {
            // We know there must be exactly two components because we know the range delimiter
            // exists
            let range_parts = raw.splitn(2, "--").collect::<Vec<_>>();
            let start_ts = Self::from_str(range_parts[0])?;
            let end_ts = Self::from_str(range_parts[1])?;

            // Make sure neither timestamp has an end to it (otherwise we would have recursive
            // ranges)
            return if start_ts.end.is_some() || end_ts.end.is_some() {
                Err(TimestampParseError::RangeInRange {
                    timestamp: raw.to_string(),
                })
            } else {
                Ok(Self {
                    start: start_ts.start,
                    end: Some(end_ts.start),
                    repeater: start_ts.repeater,
                    // It will be active if either component is
                    active: start_ts.active || end_ts.active,
                })
            };
        }

        // Usefully, timestamps are pure ASCII, so we can split characters off confidently
        // without worrying about codepoint boundaries
        if !raw.is_ascii() {
            return Err(TimestampParseError::NotAscii);
        }
        // <YYYY-mm-dd>
        if raw.len() < 12 {
            return Err(TimestampParseError::TooShort { len: raw.len() });
        }

        // Determine if the timestamp is active or not, and, while we're at it, check if it's
        // even of a valid form
        let active = if raw.starts_with('<') && raw.ends_with('>') {
            true
        } else if raw.starts_with('[') && raw.ends_with(']') {
            false
        } else {
            return Err(TimestampParseError::InvalidStartEnd {
                // We know these are fine because we did a length check above
                start: raw.chars().next().unwrap(),
                end: raw.chars().last().unwrap(),
            });
        };
        // We can safely strip the boundary characters (`<>` or `[]`)
        // NOTE: This is all valid ASCII, and has at least 10 elements
        let mut raw = (&raw[1..raw.len() - 1]).to_string();

        // Get out the date component first (we've guaranteed this won't panic in the earlier length check)
        let remaining = raw.split_off(10);
        let date_parts = raw.split('-').collect::<Vec<_>>();
        if date_parts.len() != 3 {
            return Err(TimestampParseError::InvalidDate { date: raw });
        }
        let year = date_parts[0]
            .parse::<i32>()
            .map_err(|_| TimestampParseError::InvalidYear {
                year: date_parts[0].to_string(),
            })?;
        let month =
            date_parts[1]
                .parse::<u32>()
                .map_err(|_| TimestampParseError::InvalidMonth {
                    month: date_parts[1].to_string(),
                })?;
        let day = date_parts[2]
            .parse::<u32>()
            .map_err(|_| TimestampParseError::InvalidDay {
                day: date_parts[2].to_string(),
            })?;
        // Construct an actual date from this
        let date = NaiveDate::from_ymd_opt(year, month, day)
            .ok_or(TimestampParseError::InvalidDateComponents { year, month, day })?;

        // We'll update this as we get more data
        let mut timestamp = Self {
            start: DateTime { date, time: None },
            end: None,
            repeater: None,
            active,
        };

        let chars = remaining.chars().collect::<Vec<_>>();
        // Used to keep track of the length of the day name
        let mut day_name = String::new();
        // This will consist solely of numeric characters
        let mut repeater_count = String::new();
        let mut has_end_time = false;
        let mut start_time = String::new();
        let mut end_time = String::new();

        let mut loc = TimestampLocation::Start;
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let next_c = chars.get(i + 1);

            match loc {
                TimestampLocation::Start => {
                    if c == ' ' {
                        // Continue past any whitespace at the start
                        i += 1;
                        continue;
                    } else if c.is_alphabetic() {
                        // We have a day name, parse this first character again
                        loc = TimestampLocation::DayName;
                        continue;
                    } else if c.is_numeric() {
                        // We have a time, parse this first character again
                        loc = TimestampLocation::Time;
                        continue;
                    } else if c == '+' {
                        // We have a repeater (but we don't need to parse the `+` again)
                        loc = TimestampLocation::Repeater;
                    } else {
                        return Err(TimestampParseError::BadCharacter { c });
                    }
                }
                TimestampLocation::DayName => {
                    if c == ' ' {
                        // End of the day name, we either have a time or repeater next, if anything
                        if let Some(next_c) = next_c {
                            if next_c.is_numeric() {
                                loc = TimestampLocation::Time;
                            } else if *next_c == '+' {
                                // As above, we don't need to parse the `+` (but we're looking at `next_c`, so
                                // increment here and then again later on)
                                loc = TimestampLocation::Repeater;
                                i += 1;
                            } else {
                                return Err(TimestampParseError::BadCharacter { c });
                            }
                        }
                    } else if c.is_alphabetic() && day_name.len() < 3 {
                        day_name.push(c);
                    } else if c.is_alphabetic() {
                        // Day names should be shorter than three characters
                        return Err(TimestampParseError::DayNameTooLong {
                            current: day_name,
                            next_c: c,
                        });
                    } else {
                        return Err(TimestampParseError::BadCharacter { c });
                    }
                }
                TimestampLocation::Time => {
                    if c == ' ' {
                        // End of the time, if we have anything, it has to be a repeater
                        if let Some(next_c) = next_c {
                            if *next_c == '+' {
                                // As above, we don't need to parse the `+`
                                loc = TimestampLocation::Repeater;
                            } else {
                                return Err(TimestampParseError::BadCharacter { c });
                            }
                        }
                    } else if c.is_numeric() || c == ':' {
                        // Push this character (which should be part of the time) to the appropriate string for now,
                        // we'll handle them properly when we're done
                        if has_end_time {
                            end_time.push(c);
                        } else {
                            start_time.push(c);
                        }
                    } else if c == '-' {
                        // We've got an end time
                        has_end_time = true;
                    } else {
                        return Err(TimestampParseError::BadCharacter { c });
                    }
                }
                TimestampLocation::Repeater => {
                    if c.is_numeric() {
                        // We have a number
                        repeater_count.push(c);
                    } else if c.is_alphabetic() {
                        // We've reached the unit, parse the count first (this will only consist of numeric characters)
                        let repeater_count_num = repeater_count.parse::<usize>().unwrap();
                        if let Some(unit) = RepeaterUnit::from_char(c) {
                            let repeater = Repeater {
                                count: repeater_count_num,
                                unit,
                            };
                            timestamp.repeater = Some(repeater);
                        } else {
                            return Err(TimestampParseError::BadRepeaterUnit { c });
                        }
                    }
                }
            }

            i += 1;
        }

        // We will have parsed everything valid in the timestamp by this point, but we still
        // need to parse the actual start and end timestamps!
        if !start_time.is_empty() {
            timestamp.start.time = Some(NaiveTime::parse_from_str(&start_time, "%H:%M").map_err(
                |err| TimestampParseError::InvalidTime {
                    time_str: start_time,
                    source: err,
                },
            )?);
        }
        if !end_time.is_empty() {
            let parsed_end_time = NaiveTime::parse_from_str(&end_time, "%H:%M").map_err(|err| {
                TimestampParseError::InvalidTime {
                    time_str: end_time,
                    source: err,
                }
            })?;
            timestamp.end = Some(DateTime {
                date: timestamp.start.date,
                time: Some(parsed_end_time),
            })
        }

        Ok(timestamp)
    }
    /// Converts this timestamp into a string. See [`Timestamp`] for how the written
    /// representation may be different to the string that was parsed in (textually, not
    /// logically).
    pub fn into_string(self) -> String {
        let mut ts_str = if self.active { "<" } else { "[" }.to_string();
        // Add the initial start date information (including a day name)
        ts_str.push_str(&self.start.date.format("%Y-%m-%d %a").to_string());
        // Start time if there is one
        if let Some(start_time) = self.start.time {
            ts_str.push(' ');
            ts_str.push_str(&start_time.format("%H:%M").to_string());
        }

        if let Some(end) = self.end {
            if end.date == self.start.date {
                // We have an end date, but it's the same as the start, so we can put
                // everything in one timestamp (all we need to do is push the end timestamp then)
                if let Some(end_time) = end.time {
                    ts_str.push('-');
                    ts_str.push_str(&end_time.format("%H:%M").to_string());
                }
            } else {
                // Range timestamp (we already have the start)
                ts_str.push(if self.active { '>' } else { ']' });
                let start_ts = ts_str;
                let end_ts = Self {
                    start: end,
                    end: None,
                    repeater: None,
                    active: self.active,
                }
                .into_string();

                // We don't have to worry about repeaters
                return format!("{}--{}", start_ts, end_ts);
            }
        }

        if let Some(repeater) = self.repeater {
            ts_str.push(' ');
            ts_str.push_str(&repeater.into_string());
        }
        ts_str.push(if self.active { '>' } else { ']' });

        ts_str
    }
}

/// The location we're in while parsing a timestamp. This covers everything *after* the mandatory
/// date.
enum TimestampLocation {
    /// We've parsed the mandatory date, and we're now up to parsing whatever comes after that.
    Start,
    /// The name of a day, which should be three letters long.
    DayName,
    /// A time, which may be a range.
    Time,
    /// A repeater.
    Repeater,
}

/// Checks if the given value is within the given range, modulus the given value `c`. This assumes
/// all values are given to a `start` value of 0 (asserted on in debug mode).
///
/// This returns a tuple of whether or not the value was in the range, together with the modulus
/// place it appeared. For example, if `x` is 8 and `(start, end)` is `(0, 2)`, with `c` 6, 8 mod 6
/// is 2, so the result would be `(true, 2)`. The second value in the tuple will be 0 if any of the
/// arguments are negative. In general, only rely on the second value if the first is `true`.
fn in_range_mod(x: i64, (start, end): (i64, Option<i64>), c: usize) -> (bool, u64) {
    debug_assert_eq!(start, 0, "start in modulus range check was non-zero");
    // If either the target date or the end date are behind the start, the range makes no sense.
    // This could easily happen if the target date were in the range of the repeater, but if the
    // first repeat were in the future.
    if x.is_negative() || end.is_some_and(|end| end.is_negative()) {
        return (false, 0);
    }

    let normalised = x % (c as i64);
    if let Some(end) = end {
        (normalised >= start && normalised <= end, normalised as u64)
    } else {
        // There is no end date, we just need to check if the target date is a `c` repeat of
        // the start
        (normalised == 0, normalised as u64)
    }
}

/// When a timestamp occurs relative to a given date. This does *not* regard repeaters, as it is
/// used for determining if something like a deadline has lapsed. Org mode by default progresses
/// a timestamp to its next repeat when a node is marked as `DONE`.
pub enum TimestampWhen {
    /// The timestamp occurs today.
    Present,
    /// The timestamp occurred in the past. This does *not* account for repeaters.
    Past,
    /// The timestamp will occur in the future.
    Future,
}

/// The period at which a timestamp applies on a certain date.
pub enum TimestampApplies {
    /// The timestamp applies for the whole day. This timestamp may have started, or may end, on other
    /// days.
    AllDay,
    /// The timestamp applies in a block from a specific start time to a specific end time.
    Block(NaiveTime, NaiveTime),
    /// The timestamp starts on this day at a certain time, but ends on another day.
    Start(NaiveTime),
    /// The timestamp ends on this day at a certain time, but starts on another day.
    End(NaiveTime),
    /// The timestamp does not apply to this date.
    None,
}
