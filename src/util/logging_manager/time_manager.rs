use std::num::ParseIntError;
use std::str::FromStr;
use std::time::SystemTime;

// returns a String representing the current date and time in UTC
// string is returned in the format "[YYYY-MM-DD HH:MM:SS]"
pub fn curr_timestamp() -> String {
    DateTime::curr_datetime().format()
}

// returns a String representing the current date in UTC
// string is returned in the format "YYYY_MM_DD"
pub fn curr_datestamp() -> String {
    DateTime::curr_datetime().format_date()
}

// check if string is a valid date in the format "YYYY*MM*DD" where "*" can be replaced by any character
// will return false if both "*" delimiters are not the same character or if the date is not valid
pub fn check_date(date: &str) -> bool {
    DateTime::from_str(date).is_ok()
}

enum DateTimeError {
    // date parsed is greater than expected i.e. month = 13
    OutOfBounds,
    // error parsing date into unsigned int
    Parse(ParseIntError),
    // string is incorrect length (must be 10 to fit format "YYYY_MM_DD")
    StringLength,
    // delimiter inconsistent i.e. 2020*02&29
    InconsistentDelimiter,
}

impl From<ParseIntError> for DateTimeError {
    fn from(err: ParseIntError) -> DateTimeError {
        DateTimeError::Parse(err)
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct DateTime {
    year: u16,
    month: u8,
    day: u8,
    hours: u8,
    minutes: u8,
    seconds: u8,
}

impl DateTime {
    fn format(&self) -> String {
        // format as [YYYY-MM-DD HH:MM:SS]
        format!(
            "[{:04}-{:02}-{:02} {:02}:{:02}:{:02}]",
            self.year, self.month, self.day, self.hours, self.minutes, self.seconds
        )
    }

    fn format_date(&self) -> String {
        // format as YYYY_MM_DD
        format!("{:04}_{:02}_{:02}", self.year, self.month, self.day)
    }

    // create a datetime struct from a unix time
    fn from_unix(epoch_diff: u64) -> DateTime {
        // calculate difference from Jan 1, 2020 00:00
        let epoch_difference = match epoch_diff >= 1577836800 {
            true => epoch_diff - 1577836800,
            false => panic!("Time calculated is before 2020!"),
        };

        // get difference in days
        let mut epoch_difference_days = epoch_difference / 86400;

        // get remaining seconds (less than a day)
        let second_remaining = epoch_difference - epoch_difference_days * 86400;

        // loop over years to find current
        let mut year: u64 = 2020;
        loop {
            // check for leap year
            let days_in_year = match year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                true => 366 as u64,
                false => 365 as u64,
            };

            if epoch_difference_days > days_in_year {
                year += 1;
                epoch_difference_days -= days_in_year;
            } else {
                break;
            }
        }

        // days in each month differ due to leap year
        let month_days = utils::get_month_calendar(year as u16);

        let mut month_index: usize = 0;
        loop {
            if epoch_difference_days > month_days[month_index] {
                epoch_difference_days -= month_days[month_index];
                month_index += 1;
            } else {
                break;
            }
        }

        // month index is zero based
        let mut month = month_index + 1;

        // day might cause month and year to rollover
        let day = match epoch_difference_days + 1 > month_days[month_index] {
            true => {
                if month == 12 {
                    year += 1;
                }
                month = (month + 1) % 12;
                1
            }
            false => epoch_difference_days + 1,
        };

        // calculate hours minutes and seconds from remaining time
        let hours = second_remaining / 3600;
        let minutes = (second_remaining - hours * 3600) / 60;
        let seconds = second_remaining - hours * 3600 - minutes * 60;

        DateTime {
            year: year as u16,
            month: month as u8,
            day: day as u8,
            hours: hours as u8,
            minutes: minutes as u8,
            seconds: seconds as u8,
        }
    }

    // get a datetime struct representing the current time in utc
    fn curr_datetime() -> DateTime {
        let epoch_difference = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => n.as_secs(),
            Err(_) => panic!("SystemTime is before Unix Epoch (Jan 1, 1970)!"),
        };
        DateTime::from_unix(epoch_difference)
    }
}

impl FromStr for DateTime {
    type Err = DateTimeError;

    // parse DateTime from string in format YYYY_MM_DD
    fn from_str(date: &str) -> Result<Self, Self::Err> {
        // check length
        if date.len() != 10 {
            return Err(DateTimeError::StringLength);
        }

        // check if delimeters are the same
        if &date.chars().nth(4).unwrap_or('a') != &date.chars().nth(7).unwrap_or('b') {
            return Err(DateTimeError::InconsistentDelimiter);
        }

        // parse year
        let year = u16::from_str(&date[0..4])?;

        // parse month
        let month = u8::from_str(&date[5..7])?;
        if month > 12 {
            return Err(DateTimeError::OutOfBounds);
        }

        // parse day
        let day = u8::from_str(&date[8..10])?;

        // days in each month differ due to leap year
        let month_days = utils::get_month_calendar(year);
        if day > month_days[(month as usize) - 1] as u8 {
            return Err(DateTimeError::OutOfBounds);
        }

        Ok(DateTime {
            year,
            month,
            day,
            hours: 0,
            minutes: 0,
            seconds: 0,
        })
    }
}

mod utils {

    // get an array of day counts for a month in the given year
    pub fn get_month_calendar(year: u16) -> [u64; 12] {
        // check for leap year
        if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_unix_format() {
        assert_eq!(
            "[2020-01-01 00:00:00]",
            DateTime::from_unix(1577836800).format()
        );
        assert_eq!(
            "[4040-06-03 00:00:01]",
            DateTime::from_unix(65336198401).format()
        );
        assert_eq!(
            "[4040-02-29 03:15:00]",
            DateTime::from_unix(65328002100).format()
        );
        assert_eq!(
            "[2021-12-31 23:59:59]",
            DateTime::from_unix(1640995199).format()
        );
        assert_eq!(
            "[2022-01-01 00:00:00]",
            DateTime::from_unix(1640995200).format()
        );
        assert_eq!(
            "[2020-02-29 23:59:59]",
            DateTime::from_unix(1583020799).format()
        );
        assert_eq!(
            "[2020-03-01 00:00:00]",
            DateTime::from_unix(1583020800).format()
        );
    }

    #[test]
    fn test_from_unix_date() {
        assert_eq!("2020_01_01", DateTime::from_unix(1577836800).format_date());
        assert_eq!("4040_06_03", DateTime::from_unix(65336198401).format_date());
        assert_eq!("4040_02_29", DateTime::from_unix(65328002100).format_date());
        assert_eq!("2021_12_31", DateTime::from_unix(1640995199).format_date());
        assert_eq!("2022_01_01", DateTime::from_unix(1640995200).format_date());
        assert_eq!("2020_02_29", DateTime::from_unix(1583020799).format_date());
        assert_eq!("2020_03_01", DateTime::from_unix(1583020800).format_date());
    }

    #[test]
    fn test_check_date() {
        assert_eq!(true, check_date(curr_datestamp().as_str()));
        assert_eq!(true, check_date("2020_02_29"));
        assert_eq!(false, check_date("2019_02_29"));
        assert_eq!(false, check_date("2020_02_30"));
        assert_eq!(false, check_date("2020_13_31"));
        assert_eq!(false, check_date("2045_12_45"));
        assert_eq!(true, check_date("2020/02/29"));
        assert_eq!(false, check_date("2020_02"));
        assert_eq!(false, check_date("some string"));
        assert_eq!(false, check_date("2020*02&29"));
    }
}
