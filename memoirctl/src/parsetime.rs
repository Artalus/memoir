use anyhow::Result;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum ParseError {
    #[error("Parsed string cannot be empty")]
    EmptyString,
    #[error("No digits in string")]
    NoDigits,
    #[error("Invalid character #{index}: {value:?}")]
    InvalidCharacter { index: usize, value: char },
}

/// Parses string representation of time into amount of seconds.
/// ```
/// assert_eq!(parse_time("3600"), Ok(3600))
/// assert_eq!(parse_time("60m"), Ok(3600))
/// assert_eq!(parse_time("1h"), Ok(3600))
/// assert_eq!(parse_time("1d"), Ok(3600 * 24))
/// assert!(parse_time("nope").is_err())
/// assert!(parse_time("1z").is_err())
/// assert!(parse_time("1hh").is_err())
/// ```
pub fn parse_time(input: &str) -> Result<usize, ParseError> {
    if input.is_empty() {
        return Err(ParseError::EmptyString);
    }
    let mut number_until: Option<usize> = None;
    enum Units {
        Undetermined,
        Seconds,
        Minutes,
        Hours,
        Days,
    }
    let mut units = Units::Undetermined;
    for (i, c) in input.char_indices() {
        // nothing should be met after units were determined
        if matches!(units, Units::Undetermined) {
            match c {
                's' => {
                    units = Units::Seconds;
                    continue;
                }
                'm' => {
                    units = Units::Minutes;
                    continue;
                }
                'h' => {
                    units = Units::Hours;
                    continue;
                }
                'd' => {
                    units = Units::Days;
                    continue;
                }
                _ if c.is_numeric() => {
                    number_until = Some(i);
                    continue;
                }
                // anything unexpected should result in fallthrough
                _ => {}
            }
        }
        return Err(ParseError::InvalidCharacter { index: i, value: c });
    }
    let number_until = match number_until {
        None => {
            return Err(ParseError::NoDigits);
        }
        Some(n) => n,
    };
    let value: usize = input[..=number_until]
        .parse()
        .expect("Should always be able to parse by this point!");
    let multiplier = match units {
        Units::Seconds | Units::Undetermined => 1,
        Units::Minutes => 60,
        Units::Hours => 60 * 60,
        Units::Days => 60 * 60 * 24,
    };
    Ok(value * multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(parse_time("1"), Ok(1));
        assert_eq!(parse_time("20"), Ok(20));
        assert_eq!(parse_time("1s"), Ok(1));
        assert_eq!(parse_time("20s"), Ok(20));
        assert_eq!(parse_time("1m"), Ok(60));
        assert_eq!(parse_time("20m"), Ok(20 * 60));
        assert_eq!(parse_time("1h"), Ok(60 * 60));
        assert_eq!(parse_time("20h"), Ok(20 * 60 * 60));
        assert_eq!(parse_time("1d"), Ok(60 * 60 * 24));
        assert_eq!(parse_time("20d"), Ok(20 * 60 * 60 * 24));
    }
    #[test]
    fn it_fails() {
        assert_eq!(parse_time(""), Err(ParseError::EmptyString));
        assert_eq!(
            parse_time("input"),
            Err(ParseError::InvalidCharacter {
                index: 0,
                value: 'i',
            })
        );
        assert_eq!(
            parse_time("-1"),
            Err(ParseError::InvalidCharacter {
                index: 0,
                value: '-',
            })
        );
        assert_eq!(
            parse_time("1z"),
            Err(ParseError::InvalidCharacter {
                index: 1,
                value: 'z',
            })
        );
        assert_eq!(
            parse_time("2d6"),
            Err(ParseError::InvalidCharacter {
                index: 2,
                value: '6',
            })
        );
        assert_eq!(
            parse_time("2dd"),
            Err(ParseError::InvalidCharacter {
                index: 2,
                value: 'd',
            })
        );
        assert_eq!(
            parse_time("2hs"),
            Err(ParseError::InvalidCharacter {
                index: 2,
                value: 's',
            })
        );
    }
}
