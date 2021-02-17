//! Tokenizer that converts a zinc string form into a series of `Token`s.
use nom::{
    branch::alt,
    bytes::complete::{is_a, tag},
    character::complete::{alphanumeric1, char, digit1, multispace0, multispace1, newline, one_of, space0, space1},
    combinator::{complete, map, opt, peek, recognize},
    error::ErrorKind,
    multi::{many1, separated_list},
    sequence::{delimited, preceded, separated_pair, terminated, tuple}, IResult,
};

use chrono::{Date, DateTime, Datelike, FixedOffset, NaiveTime, NaiveDateTime, TimeZone, Utc};

use crate::hval::HVal;
use crate::token::*;

/// let parser = delimited(tag("abc"), tag("|"), tag("efg"));
///
/// assert_eq!(parser("abc|efg"), Ok(("", "|")));
/// assert_eq!(parser("abc|efghij"), Ok(("hij", "|")));
/// assert_eq!(parser(""), Err(Err::Error(("", ErrorKind::Tag))));
/// assert_eq!(parser("123"), Err(Err::Error(("123", ErrorKind::Tag))));
/// ```
// pub fn space_after<I, O1, O2, O3, E: ParseError<I>, F, G, H>(first: F, sep: G, second: H) -> impl Fn(I) -> IResult<I, O2, E>
// where
//   F: Fn(I) -> IResult<I, O1, E>,
//   G: Fn(I) -> IResult<I, O2, E>,
//   H: Fn(I) -> IResult<I, O3, E>,
// {
//   move |input: I| {
//     let (input, _) = first(input)?;
//     let (input, o2) = sep(input)?;
//     second(input).map(|(i, _)| (i, o2))
//   }
// }

// fn space_after<F, I, O, E>(f: F) -> impl Fn(I) -> IResult<I, O, E>
// where
//     F: Fn(I) -> IResult<I, O, E>,
//     I: nom::InputTakeAtPosition,
//     <I as nom::InputTakeAtPosition>::Item: nom::AsChar + Clone,
//     E: nom::error::ParseError<I>,
// {
//       move |input: I| {
//             let (input, _) = f(input)?;
//             let (input, o2) = sep(input)?;
//             second(input).map(|(i, _)| (i, o2))
//       }
// }

fn spacey<F, I, O, E>(f: F) -> impl Fn(I) -> IResult<I, O, E>
where
    F: Fn(I) -> IResult<I, O, E>,
    I: nom::InputTakeAtPosition,
    <I as nom::InputTakeAtPosition>::Item: nom::AsChar + Clone,
    E: nom::error::ParseError<I>,
{
    delimited(space0, f, space0)
}

fn multispacey<F, I, O, E>(f: F) -> impl Fn(I) -> IResult<I, O, E>
where
    F: Fn(I) -> IResult<I, O, E>,
    I: nom::InputTakeAtPosition,
    <I as nom::InputTakeAtPosition>::Item: nom::AsChar + Clone,
    E: nom::error::ParseError<I>,
{
    delimited(multispace0, f, multispace0)
}

// fn comma<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
//     map(tag(","), |_: &str| Token::Comma)(i)
// }

fn comma_val<'a>(i: &'a str) -> IResult<&'a str, Val, (&'a str, ErrorKind)> {
    map(tag(","), |_: &str| {
        Val::new(Box::new(Comma::new()) as Box<dyn HVal>)
    })(i)
}

fn null<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(tag("N"), |_: &str| Token::Null)(i)
}

fn marker<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(tag("M"), |_: &str| Token::Marker)(i)
}

fn remove<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(tag("R"), |_: &str| Token::Remove)(i)
}

fn na<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(tag("NA"), |_: &str| Token::NA)(i)
}

fn nl<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(newline, |_: char| Token::NL)(i)
}

fn bool<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(alt((tag("T"), tag("F"))), |o: &str| {
        if o == "F" {
            Token::Bool(false)
        } else {
            Token::Bool(true)
        }
    })(i)
}

fn inf<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(
        tuple((opt(char('-')), tag("Inf"))),
        |(o, _): (std::option::Option<char>, &str)| {
            if o.is_some() {
                Token::InfNeg
            } else {
                Token::Inf
            }
        },
    )(i)
}

fn nan<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(tag("NaN"), |_: &str| Token::NaN)(i)
}

fn in_quotes(buf: &str) -> IResult<&str, String> {
    let mut ret = String::new();
    let mut skip_delimiter = false;
    for (i, ch) in buf.char_indices() {
        if ch == '\\' && !skip_delimiter {
            skip_delimiter = true;
        } else if ch == '"' && !skip_delimiter {
            return Ok((&buf[i..], ret));
        } else {
            ret.push(ch);
            skip_delimiter = false;
        }
    }
    Err(nom::Err::Incomplete(nom::Needed::Unknown))
}

fn quoted_string_s<'a>(i: &'a str) -> IResult<&'a str, String, (&'a str, ErrorKind)> {
    let qs = preceded(tag("\""), in_quotes);
    terminated(qs, tag("\""))(i)
}

pub fn quoted_string<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(quoted_string_s, |s: String| {
        Token::EscapedString(s.to_string())
    })(i)
}

fn unicode_alpha0(i: &str) -> nom::IResult<&str, &str> {
    nom_unicode::complete::alpha0(i)
}

pub fn uri<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    let qs = preceded(tag("`"), unicode_alpha0);
    map(terminated(qs, tag("`")), |s: &str| {
        Token::Uri(s.to_string())
    })(i)
}

fn negpos_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    map(opt(alt((tag("+"), tag("-")))), |o: Option<&str>| {
        o.unwrap_or("")
    })(i)
}

// fn negpos_i32<'a>(i: &'a str) -> IResult<&'a str, i32, (&'a str, ErrorKind)> {
//     map(alt((tag("+"), tag("-"))), |s: &str| match s {
//         "-" => -1i32,
//         "+" => 1i32,
//         _ => 1,
//     })(i)
// }

// 2011-06-07
// YYYY-MM-DD
pub fn date_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple((digit1, char('-'), digit1, char('-'), digit1)))(i)
}

pub fn date<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(date_s, |s: &str| {
        Token::Date(dtparse::parse(s).unwrap().0.date())
    })(i)
}

// "{date},{date}"
// Ie 2011-06-07, 2011-06-08
fn date_range_s<'a>(i: &'a str) -> IResult<&'a str, (&'a str, &'a str), (&'a str, ErrorKind)> {
    separated_pair(date_s, char(','), date_s)(i)
}


// let dt = tmp
// .1
// .expect("Timezone is None")
// .from_local_datetime(&tmp.0)
// .unwrap();

fn naive_datetime_to_fixed_offset(dt: NaiveDateTime) -> DateTime::<FixedOffset> {
    let u = DateTime::<Utc>::from_utc(dt, Utc);
    DateTime::<FixedOffset>::from(u)
}

fn date_range<'a>(i: &'a str) -> IResult<&'a str, (Token, Token), (&'a str, ErrorKind)> {
    map(date_range_s, |t: (&str, &str)| {
        (
            Token::DateTime(naive_datetime_to_fixed_offset(dtparse::parse(t.0).unwrap().0.date().and_hms(0,0,0))),
            Token::DateTime(naive_datetime_to_fixed_offset(dtparse::parse(t.1).unwrap().0.date().and_hms(0,0,0)))
        )
    })(i)
}

fn hours_minutes_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple((digit1, char(':'), digit1)))(i)
}

// hh:mm:ss
fn time_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple((digit1, char(':'), digit1, char(':'), digit1)))(i)
}

// hh:mm:ss.FFFFFFFFF
fn time_with_subseconds_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple((time_s, opt(tuple((char('.'), digit1))))))(i)
}

pub fn time_with_subseconds<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(time_with_subseconds_s, |s: &str| {
        Token::Time(NaiveTime::parse_from_str(s, "%H:%M:%S%.f").expect("Failed to parse"))
    })(i)
}


// 2012-09-29T14:56:18.277Z UTC
// 2012-09-29T14:56:18.277Z
// 2011-06-07T09:51:27-04:00 New_York
// 2011-06-07T09:51:27+06:00
// Z
// -04:00 New_York
// z zzzz
fn timeoffset_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    map(recognize(tuple((negpos_s, hours_minutes_s))), |s: &str| s)(i)
}

fn z_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    map(alt((tag("Z"), tag("z"))), |s: &str| s)(i)
}

fn timezone_char<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    let allowed_chars: &str = "_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    is_a(allowed_chars)(i)
}

//+06:00
//-06:00
fn timezone_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    map(
        recognize(tuple((
            alt((z_s, timeoffset_s)),
            opt(tuple((tag(" "), timezone_char))),
        ))),
        |s: &str| s,
    )(i)
}

// 2011-06-07T09:51:27-04:00 New_York
// YYYY-MM-DD'T'hh:mm:ss.FFFFFFFFFz zzzz
fn datetime_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    map(
        recognize(tuple((date, char('T'), alt((time_with_subseconds_s, time_s)), timezone_s))),
        |s: &str| s,
    )(i)
}

fn str_to_datetime_token(s: &str) -> Token {
    // First split off tz name at space
    let vec: Vec<&str> = s.split(' ').collect::<Vec<&str>>();

    let tmp: (NaiveDateTime, Option<FixedOffset>);

    if vec.len() > 1 {
        tmp = dtparse::parse(vec[0]).unwrap();
    } else {
        tmp = dtparse::parse(s).unwrap();
    }

    let dt = tmp
        .1
        .expect("Timezone is None")
        .from_local_datetime(&tmp.0)
        .unwrap();

    Token::DateTime(dt)
}

fn datetime<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(datetime_s, |s: &str| {
        str_to_datetime_token(s)
    })(i)
}

// "{dateTime},{dateTime}"
// Ie 2011-06-07T09:51:27-04:00, 2011-06-09T09:51:27-04:00
fn datetime_range_s<'a>(i: &'a str) -> IResult<&'a str, (&'a str, &'a str), (&'a str, ErrorKind)> {
    separated_pair(datetime_s, char(','), datetime_s)(i)
}

fn datetime_range<'a>(i: &'a str) -> IResult<&'a str, (Token, Token), (&'a str, ErrorKind)> {
    map(datetime_range_s, |t: (&str, &str)| {
        (str_to_datetime_token(t.0), str_to_datetime_token(t.1))
    }
    )(i)
}

fn utc_date_floor(dt: Date<Utc>) -> DateTime::<FixedOffset> {
    let midnight = dt.and_hms(0,0,0);
    DateTime::<FixedOffset>::from(midnight)
}

fn range_lastfiveminutes<'a>(i: &'a str) -> IResult<&'a str, (Token, Token), (&'a str, ErrorKind)> {
    map(tag("lastfiveminutes"), |_: &str| {
        (
            Token::DateTime(DateTime::<FixedOffset>::from(Utc::now() - chrono::Duration::minutes(5))),
            Token::DateTime(DateTime::<FixedOffset>::from(Utc::now()))
        )
        }
    )(i)
}

fn range_lasthour<'a>(i: &'a str) -> IResult<&'a str, (Token, Token), (&'a str, ErrorKind)> {
    map(tag("lasthour"), |_: &str| {
        (
            Token::DateTime(DateTime::<FixedOffset>::from(Utc::now() - chrono::Duration::minutes(60))),
            Token::DateTime(DateTime::<FixedOffset>::from(Utc::now()))
        )
        }
    )(i)
}

fn range_today<'a>(i: &'a str) -> IResult<&'a str, (Token, Token), (&'a str, ErrorKind)> {
    map(tag("today"), |_: &str| (Token::DateTime(utc_date_floor(Utc::today())), Token::DateTime(DateTime::<FixedOffset>::from(Utc::now()))))(i)
}

fn range_yesterday<'a>(i: &'a str) -> IResult<&'a str, (Token, Token), (&'a str, ErrorKind)> {
    map(tag("yesterday"), |_: &str| {
        (
            Token::DateTime(utc_date_floor(Utc::today() - chrono::Duration::days(1))),
            Token::DateTime(utc_date_floor(Utc::today()))
        )
        }
    )(i)
}

fn range_thisweek<'a>(i: &'a str) -> IResult<&'a str, (Token, Token), (&'a str, ErrorKind)> {
    
    map(tag("thisweek"), |_: &str| {
            let weekday: chrono::Weekday = Utc::today().weekday();
            let number_of_days_from_sunday: u32 = weekday.num_days_from_sunday();
        
            (Token::DateTime(utc_date_floor(Utc::today() - chrono::Duration::days(number_of_days_from_sunday as i64))),
             Token::DateTime(DateTime::<FixedOffset>::from(Utc::now()))
            )
        }
    )(i)
}

fn range_thismonth<'a>(i: &'a str) -> IResult<&'a str, (Token, Token), (&'a str, ErrorKind)> {
    map(tag("thismonth"), |_: &str| {
        (
            Token::DateTime(utc_date_floor(Utc::today().with_day(1).unwrap())),
            Token::DateTime(DateTime::<FixedOffset>::from(Utc::now()))
        )
    }
    )(i)
}

fn range_thisyear<'a>(i: &'a str) -> IResult<&'a str, (Token, Token), (&'a str, ErrorKind)> {
    map(tag("thisyear"), |_: &str| {
        (Token::DateTime(utc_date_floor(Utc::today().with_day(1).unwrap().with_month(1).unwrap())),
         Token::DateTime(DateTime::<FixedOffset>::from(Utc::now()))
        )
    }
    )(i)
}

pub fn date_range_to_token<'a>(i: &'a str) -> IResult<&'a str, (Token, Token), (&'a str, ErrorKind)> {
    alt((
        range_today,
        range_yesterday,
        range_thisweek,
        range_thismonth,
        range_thisyear,
        range_lastfiveminutes,
        range_lasthour,
        datetime_range,
        map(datetime_s, |s: &str| (str_to_datetime_token(s), Token::DateTime(DateTime::<FixedOffset>::from(Utc::now())))),
        date_range,
        map(date_s, |s: &str| {
            
            (
                Token::DateTime(naive_datetime_to_fixed_offset(dtparse::parse(s).unwrap().0.date().and_hms(0,0,0))),
                Token::DateTime(DateTime::<FixedOffset>::from(Utc::now()))
            )
            }
        ),
    ))(i)
}

pub fn ident<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    let remaining_chars: &str = "_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let first_chars: &str = "abcdefghijklmnopqrstuvwxyz";
    // Returns whole strings matched by the given parser.
    recognize(
        // Runs the first parser, if succeeded then runs second, and returns the second result.
        // Note that returned ok value of `preceded()` is ignored by `recognize()`.
        preceded(
            // Parses a single character contained in the given string.
            one_of(first_chars),
            // Parses the longest slice consisting of the given characters
            opt(is_a(remaining_chars)),
        ),
    )(i)
}

pub fn zinc_id<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(complete(ident), |s: &str| Token::Id(s.into()))(i)
}

// fn is_char_digit(chr: char) -> bool {
//     return chr.is_ascii() && is_digit(chr as u8);
// }

// fn digits<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
//     take_while1(is_char_digit)(i)
// }

// Number: 1, -34, 5.4, -5.4, 9.23, 74.2, 4,
// fn simple_number_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
//     map(
//         recognize(tuple((
//             opt(alt((char('-'), char('+')))),
//             many1(digit1),
//             opt(preceded(char('.'), many1(digit1))),
//         ))),
//         |s: &str| s,
//     )(i)
// }

// Number: 1, -34, 5.4, -5.4, 9.23, 74.2, 4,
fn simple_number<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(
        recognize(tuple((
            opt(alt((char('-'), char('+')))),
            many1(digit1),
            opt(preceded(char('.'), many1(digit1))),
        ))),
        |s: &str| Token::Number(ZincNumber::new(s.parse::<f64>().unwrap()), "".into()),
    )(i)
}

fn exponent<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple((alt((char('e'), char('E'))), simple_number)))(i)
}

fn number<'a>(i: &'a str) -> IResult<&'a str, f64, (&'a str, ErrorKind)> {
    map(
        recognize(tuple((simple_number, opt(exponent)))),
        |s: &str| s.parse::<f64>().unwrap(),
    )(i)
}

fn units<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    alphanumeric1(i)
}

pub fn number_with_unit<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(tuple((number, opt(units))), |t: (f64, Option<&str>)| {
        Token::Number(ZincNumber::new(t.0), t.1.unwrap_or(&"".to_string()).into())
    })(i)
}

// Number: 1, -34, 5.4, -5.4, 9.23, 74.2, 4, 5.4e-45, -5.4e-45, 67.3E7 Inf -Inf +Inf NaN
fn zinc_number<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    alt((number_with_unit, inf, nan))(i)
}

//println!("{:?}", zinc_ref(r#"@hisId"#));
// <ref>         := "@" <refChar>* [ " " <str> ]
// <refChar>     := <alpha> | <digit> | "_" | ":" | "-" | "." | "~"

fn ref_char<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    let allowed_chars: &str = "_:-.~abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    is_a(allowed_chars)(i)
}

// println!("{:?}", zinc_ref(r#"@hisId"#));
pub fn zinc_ref<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(
        tuple((
            tag("@"),
            ref_char,
            opt(preceded(multispace0, quoted_string_s)),
        )),
        |t: (&str, &str, Option<String>)| Token::Ref("@".to_string() + t.1, t.2),
    )(i)
}

fn ver<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(
        separated_pair(tag("ver"), char(':'), quoted_string_s),
        |t: (&str, String)| Token::Ver(t.1.to_string()),
    )(i)
}

pub fn token<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    alt((
        zinc_ref,
        quoted_string,
        uri,
        datetime,
        date,
        zinc_number,
        bool,
        na,
        null,
        marker,
        remove,
    ))(i)
}

fn scalar<'a>(i: &'a str) -> IResult<&'a str, Val, (&'a str, ErrorKind)> {
    map(token, |t: Token| {
        Val::new(Box::new(t.clone()) as Box<dyn HVal>)
    })(i)
}

fn zinc_marker_tag<'a>(
    i: &'a str,
) -> IResult<&'a str, (Token, Option<Val>), (&'a str, ErrorKind)> {
    map(zinc_id, |t: Token| (t, None))(i)
}

// "id:@hisId"
fn zinc_tag_pair<'a>(i: &'a str) -> IResult<&'a str, (Token, Option<Val>), (&'a str, ErrorKind)> {
    map(
        separated_pair(zinc_id, char(':'), val),
        |t: (Token, Val)| (t.0, Some(t.1)),
    )(i)
}

fn zinc_tag<'a>(i: &'a str) -> IResult<&'a str, Tag, (&'a str, ErrorKind)> {
    map(
        alt((zinc_tag_pair, zinc_marker_tag)),
        |t: (Token, Option<Val>)| {
            //Token::Tag(Box::new(t.0), Box::new(t.1))
            Tag::new_from_val(t.0, t.1)
        },
    )(i)
}

// id:@hisId projName:"test"
fn tags<'a>(i: &'a str) -> IResult<&'a str, Tags, (&'a str, ErrorKind)> {
    //terminated(separated_list(char(' '), scalar), opt(tag(",")))(i)
    map(separated_list(space1, zinc_tag), |t: Vec<Tag>| {
        Tags::new(&t)
    })(i)
}


// fn scalar<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
//     alt((zinc_ref, quoted_string, uri, datetime, date, zinc_number, bool, na, null, marker, remove))(i)
// }

// <dict>        :=  "{" <tags> "}"
// returns Dict(HashMap<String, Option<Token>>),
// fn dict<'a>(i: &'a str) -> IResult<&'a str, Box<dyn HVal>, (&'a str, ErrorKind)> {

//     map(
//         delimited(spacey(tag("{")), tags, spacey(tag("}"))),
//         |tags: Tags| {

//             // Box::new(Val::new(t.clone())) as Box<dyn HVal>
//             Box::new(Dict::new_from_tags(&tags)) as Box<dyn HVal>
//         }
//     )(i)
// }

// <tag>         :=  <tagMarker> | <tagPair>
// <tagMarker>   :=  <id>  // val is assumed to be Marker
// <tagPair>     :=  <id> ":" <val>

// dict(r#""{dis:"Dict!" foo}"#)
fn dict<'a>(i: &'a str) -> IResult<&'a str, Val, (&'a str, ErrorKind)> {
    map(
        delimited(tuple((tag("{"), multispace0)), tags, tuple((multispace0, tag("}")))),
        |tags: Tags| {
            // Box::new(Val::new(t.clone())) as Box<dyn HVal>
            Val::new(Box::new(Dict::new_from_tags(&tags)) as Box<dyn HVal>)
        },
    )(i)
}

fn list_of_vals<'a>(i: &'a str) -> IResult<&'a str, Vec<Val>, (&'a str, ErrorKind)> {
    terminated(separated_list(spacey(tag(",")), scalar), opt(tag(",")))(i)
}

fn list<'a>(i: &'a str) -> IResult<&'a str, Val, (&'a str, ErrorKind)> {
    //delimited(spacey(tag("[")), list_of_vals, spacey(tag("]")))(i)
    map(
        delimited(tuple((tag("["), multispace0)), list_of_vals, tuple((multispace0, tag("]")))),
        |v: Vec<Val>| {
            //let tmp: Vec<Box<Token>> = v.into_iter().map(|x| Box::new(x)).collect();
            Val::new(Box::new(List::new(v)) as Box<dyn HVal>)
        },
    )(i)
}

fn col<'a>(i: &'a str) -> IResult<&'a str, Col, (&'a str, ErrorKind)> {
    map(
        tuple((zinc_id, space0, opt(tags))),
        |t: (Token, _, Option<Tags>)| {
            let id: Token = t.0;
            let tags: Option<Tags> = t.2;

            Col::new(id, tags)
        },
    )(i)
}

fn cols<'a>(i: &'a str) -> IResult<&'a str, Cols, (&'a str, ErrorKind)> {
    map(separated_list(spacey(char(',')), col), |v: Vec<Col>| {
        Cols::new(v)
    })(i)
}

fn val<'a>(i: &'a str) -> IResult<&'a str, Val, (&'a str, ErrorKind)> {
    alt((sub_grid, list, dict, scalar))(i)
}

fn cell<'a>(i: &'a str) -> IResult<&'a str, Val, (&'a str, ErrorKind)> {
    map(alt((val, peek(comma_val))), |v: Val| {
        let s = v.to_string();

        match s.as_ref() {
            "," => Val::new(Box::new(Token::Null) as Box<dyn HVal>),
            _ => v, 
        }
    })(i)
}

fn row<'a>(i: &'a str) -> IResult<&'a str, Row, (&'a str, ErrorKind)> {
    //separated_list(spacey(char(',')), cell)(i)

    map(separated_list(spacey(char(',')), cell), |v: Vec<Val>| {
        // let tmp: Vec<Box<Token>> = v.into_iter().map(|x| Box::new(x)).collect();
        // Token::Row(tmp)

        Row::new(v)
    })(i)
}

// return Token::Rows(Vec<Box<Token>>),
fn rows<'a>(i: &'a str) -> IResult<&'a str, Rows, (&'a str, ErrorKind)> {
    map(
        separated_list(spacey(nl), row), // list of rows seperated by newline
        |v: Vec<Row>| {
            // Each row must end in nl we pop this here
            let mut tmp = v.clone();
            tmp.pop();
            Rows::new(tmp)
        },
    )(i)
}

// ver:"3.0" projName:"test""
// GridMeta(Box<Token>, Option<Box<Token>>),
fn grid_meta<'a>(i: &'a str) -> IResult<&'a str, GridMeta, (&'a str, ErrorKind)> {
    map(
        tuple((ver, space0, opt(tags))),
        |t: (Token, _, Option<Tags>)| GridMeta::new(t.0, t.2),
    )(i)
}

// <grid>        :=  <gridMeta> <cols> [<row>]*
pub fn grid<'a>(i: &'a str) -> IResult<&'a str, Grid, (&'a str, ErrorKind)> {
    map(
        tuple((grid_meta, multispace1, cols, multispace0, opt(rows), multispace0)),
        |t: (GridMeta, _, Cols, _, Option<Rows>, _)| {
            let rows: Rows = t.4.unwrap_or(Rows::new(vec![]));
            Grid::new(t.0, t.2, rows)
        },
    )(i)
}

// <grid>        :=  "<<" <grid> ">>"
fn sub_grid<'a>(i: &'a str) -> IResult<&'a str, Val, (&'a str, ErrorKind)> {
    map(
        delimited(
            delimited(space0, tag("<<"), multispace0),
            grid,
            delimited(space0, tag(">>"), space0),
        ),
        |g: Grid| Val::new(Box::new(g) as Box<dyn HVal>),
    )(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! row {
        ( $( $x:expr ),* ) => {
            {
                let mut temp_vec = Vec::new();
                $(
                    temp_vec.push(Val::new_from_token($x));
                )*
                Row::new(temp_vec)
            }
        };
    }

    macro_rules! number {
        ( $x:expr ) => {
            {
                Token::Number(ZincNumber::new($x), "".to_string())
            }
        };
    }

    macro_rules! assert_nom_fn_eq {
        ($a:expr, $b:expr) => {
            assert_eq!(format!("{:?}", $a), $b);
        };
    }

    macro_rules! assert_nom_fn_is_ok {
        ($a:expr) => {
            assert!(format!("{:?}", $a).starts_with("Ok"));
        };
    }

    macro_rules! assert_nom_fn_is_err {
        ($a:expr) => {
            assert!(format!("{:?}", $a).starts_with("Err"));
        };
    }

    macro_rules! assert_nom_fn_eq_no_remain_check {
        ($a:expr, $b:expr) => {{
            let mut tmp: String = "".into();

            let m = match $a {
                Ok(v) => {
                    tmp = v.1.to_string();
                    tmp == $b.to_string()
                }
                Err(_) => false,
            };

            if !m {
                println!("Left:\n{}", tmp);
                println!("\n");
                println!("Right:\n{}", $b.to_string());
            }

            assert!(m);
        }};
    }

    #[test]
    fn val_test() {
        use super::*;

        let comma = Val::new(Box::new(Token::Comma));

        assert_eq!(comma.to_string(), ",".to_string());
    }

    #[test]
    fn date_range_to_token_test() {
        use super::*;

        let t = DateTime::<FixedOffset>::from(Utc::now());

        //assert_eq!(date_today("today"), Ok(("", Token::DateTime(t))));
        //assert_eq!(date_yesterday("yesterday"), Ok(("", Token::DateTime(t))));
        //assert_eq!(date_thisweek("thisweek"), Ok(("", Token::DateTime(t))));
        //assert_eq!(date_thismonth("thismonth"), Ok(("", Token::DateTime(t))));
        //assert_eq!(range_thisyear("thisyear"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));
        
       // assert_eq!(date_range_to_token("thisweek"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));
       // assert_eq!(date_range_to_token("yesterday"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));
       // assert_eq!(date_range_to_token("thisyear"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));
       // assert_eq!(date_range_to_token("2020-08-02,2020-08-06"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));
       // assert_eq!(date_range_to_token("2020-08-11T15:35:24.677428186+00:00,2020-08-12T12:35:24.677428186+00:00"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));
       // assert_eq!(date_range_to_token("2020-08-11T15:35:24.677428186+00:00"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));
       // assert_eq!(date_range_to_token("2020-08-11"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));

       //println!("{:?}", date_range_to_token("2020-08-11"));
       
       println!("{:?}", date_range_to_token("lastfiveminutes"));
       println!("{:?}", date_range_to_token("yesterday"));
       println!("{:?}", time_s("11:30:00"));
       println!("{:?}", time_with_subseconds_s("11:30:00.677428186"));
       println!("{:?}", datetime_s("2020-09-02T11:30:00+00:00"));
       println!("{:?}", datetime_range("2020-09-02T11:30:00+00:00,2020-09-02T12:30:00+00:00"));
       println!("{:?}", date_range_to_token("2020-09-02T11:30:00+00:00"));


    }

    #[test]
    fn date_test() {
        use super::*;

        let t = DateTime::<FixedOffset>::from(Utc::now());

        //assert_eq!(date_today("today"), Ok(("", Token::DateTime(t))));
        //assert_eq!(date_yesterday("yesterday"), Ok(("", Token::DateTime(t))));
        //assert_eq!(date_thisweek("thisweek"), Ok(("", Token::DateTime(t))));
        //assert_eq!(date_thismonth("thismonth"), Ok(("", Token::DateTime(t))));
        //assert_eq!(range_thisyear("thisyear"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));
        
       // assert_eq!(date_range_to_token("thisweek"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));
       // assert_eq!(date_range_to_token("yesterday"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));
       // assert_eq!(date_range_to_token("thisyear"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));
       // assert_eq!(date_range_to_token("2020-08-02,2020-08-06"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));
       // assert_eq!(date_range_to_token("2020-08-11T15:35:24.677428186+00:00,2020-08-12T12:35:24.677428186+00:00"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));
       // assert_eq!(date_range_to_token("2020-08-11T15:35:24.677428186+00:00"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));
       // assert_eq!(date_range_to_token("2020-08-11"), Ok(("", (Token::DateTime(t), Token::DateTime(t)))));

        // fn date_today<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
        //     map(tag("today"), |_: &str| Token::DateTime(utc_date_floor(Utc::today())))(i)
        // }
        
        // fn date_yesterday<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
        //     map(tag("yesterday"), |_: &str| Token::DateTime(utc_date_floor(Utc::today() - chrono::Duration::days(1))))(i)
        // }
        
        // fn date_thisweek<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
            
        //     map(tag("thisweek"), |_: &str| {
        //             let weekday: chrono::Weekday = Utc::today().weekday();
        //             let number_of_days_from_sunday: u32 = weekday.num_days_from_sunday();
                
        //             Token::DateTime(utc_date_floor(Utc::today() - chrono::Duration::days(number_of_days_from_sunday as i64)))
        //         }
        //     )(i)
        // }
        
        // fn date_thismonth<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
        //     map(tag("thismonth"), |_: &str| Token::DateTime(utc_date_floor(Utc::today().with_day(1).unwrap())))(i)
        // }
        
        // fn date_thisyear<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
        //     map(tag("thisyear"), |_: &str| Token::DateTime(utc_date_floor(Utc::today().with_month(1).unwrap())))(i)
        // }






        assert_eq!(date_s("07-03-1978"), Ok(("", "07-03-1978")));
        assert_eq!(
            date_range_s("07-03-1978,07-04-1978"),
            Ok(("", ("07-03-1978", "07-04-1978")))
        );

        assert_eq!(time_s("23:33:07"), Ok(("", "23:33:07")));
        assert_eq!(
            time_with_subseconds_s("23:33:07.087642"),
            Ok(("", "23:33:07.087642"))
        );
        assert_eq!(timezone_s("Z UTC"), Ok(("", "Z UTC")));
        assert_ne!(timezone_s("X UTC"), Ok(("", "X UTC")));
        assert_eq!(timezone_s("Z"), Ok(("", "Z")));
        assert_eq!(hours_minutes_s("03:00"), Ok(("", "03:00")));
        assert_eq!(timeoffset_s("03:00"), Ok(("", "03:00")));
        assert_eq!(timeoffset_s("+03:00"), Ok(("", "+03:00")));
        assert_eq!(timezone_s("+06:00"), Ok(("", "+06:00")));

        let mut dt = DateTime::parse_from_rfc3339("2012-09-29T14:56:18.277Z").unwrap();

        assert_eq!(
            datetime("2012-09-29T14:56:18.277Z"),
            Ok(("", Token::DateTime(dt)))
        );

        dt = DateTime::parse_from_rfc3339("2011-06-07T09:51:27-04:00").unwrap();

        assert_eq!(
            datetime("2011-06-07T09:51:27-04:00 New_York"),
            Ok(("", Token::DateTime(dt)))
        );

        assert_eq!(
            datetime_range_s("2011-06-07T09:51:27-04:00,2011-06-09T09:51:27-04:00"),
            Ok((
                "",
                ("2011-06-07T09:51:27-04:00", "2011-06-09T09:51:27-04:00")
            ))
        );
    }

    #[test]
    fn list_test() {
        use super::*;
        

        assert_eq!(
            zinc_number("32143m"),
            Ok(("", Token::Number(ZincNumber::new(32143f64), "m".into())))
        );

        assert_nom_fn_eq!(
            list("[6,8,9]"),
            r#"Ok(("", List([Number(6.0, ""), Number(8.0, ""), Number(9.0, "")])))"#
        );
        assert_nom_fn_eq!(
            list("[ 6,  8, 9]"),
            r#"Ok(("", List([Number(6.0, ""), Number(8.0, ""), Number(9.0, "")])))"#
        );
        assert_nom_fn_eq!(
            list("[ 6,8,9,]"),
            r#"Ok(("", List([Number(6.0, ""), Number(8.0, ""), Number(9.0, "")])))"#
        );
        assert_nom_fn_eq!(
            list("[6m,8m,9m]"),
            r#"Ok(("", List([Number(6.0, "m"), Number(8.0, "m"), Number(9.0, "m")])))"#
        );
        assert_nom_fn_eq!(
            list("[6,T,F]"),
            r#"Ok(("", List([Number(6.0, ""), Bool(true), Bool(false)])))"#
        );
        assert_nom_fn_eq!(
            list("[6,T, F]"),
            r#"Ok(("", List([Number(6.0, ""), Bool(true), Bool(false)])))"#
        );
        assert_nom_fn_eq!(
            list("[6,R,M,F]"),
            r#"Ok(("", List([Number(6.0, ""), Remove, Marker, Bool(false)])))"#
        );
        assert_nom_fn_eq!(
            list("[6,NA,M,F]"),
            r#"Ok(("", List([Number(6.0, ""), NA, Marker, Bool(false)])))"#
        );
    }

    #[test]
    fn tags_test() {
        use super::*;
        
        println!("tags {:?}", tags(r#"projName:"test" id:@hisId"#));
        println!("tags {:?}", tags(r#"dis:"Dict!" foo"#));
        println!("tags {:?}", tags(r#"dis:"ict" foo:7"#));
        println!("tags {:?}", tags(r#"ids:@hisId"#));
        println!("tags {:?}", tags(r#"ids:[9,8,9,3]"#));
        println!("tags {:?}", tags(r#"ids:[@hisId1,@hisId2,@hisId3]"#));
        println!("tags {:?}", tags("id:[@619261,@619262] action:\"tags\"")); 
    }

    #[test]
    fn dict_test() {
        use super::*;
        

        assert_eq!(
            zinc_number("32143m"),
            Ok(("", Token::Number(ZincNumber::new(32143f64), "m".into())))
        );

        assert_nom_fn_is_ok!(dict(r#"{id:@hisId projName:"test"}"#));
        assert_nom_fn_is_err!(dict(r#"{id:@hisId   projName:"test"}"#));

        assert_nom_fn_eq!(
            tags(r#"projName:"test" id:@hisId"#),
            r#"Ok(("", [Tag(Id("projName"), Some(EscapedString("test"))), Tag(Id("id"), Some(Ref("@hisId", None)))]))"#
        );
        println!("tags {:?}", tags(r#"projName:"test" id:@hisId"#));

        //dis:"Dict!" foo
        println!("tags {:?}", tags(r#"dis:"Dict!" foo"#));
        println!("tags {:?}", tags(r#"dis:"ict" foo:7"#));

        //

        //println!("{:?}", dict(r#""{dis:"Dict!" foo:7}"#));
        println!("{:?}", dict(r#"{dis:"Dict!" foo}"#));
        assert_nom_fn_is_err!(dict(r#"dict",{dis:"Dict!" foo}"#));

        println!("{:?}", row(r#""dict",{dis:"Dict!" foo}"#));

        //assert_nom_fn_eq
    }

    #[test]
    fn cols_test() {
        use super::*;

        fn parser(input: &str) -> IResult<&str, char> {
            newline(input)
        }

        assert_nom_fn_eq!(
            ver(r#"ver:"3.0" projName:"test""#),
            r#"Ok((" projName:\"test\"", Ver("3.0")))"#
        );
        assert_nom_fn_eq!(
            zinc_ref(r#"@hisId 4"#),
            r#"Ok((" 4", Ref("@hisId", None)))"#
        );
        assert_nom_fn_eq!(
            zinc_tag_pair(r#"id:@hisId"#),
            r#"Ok(("", (Id("id"), Some(Ref("@hisId", None)))))"#
        );
        assert_nom_fn_eq!(
            zinc_tag(r#"id:@hisId"#),
            r#"Ok(("", Tag(Id("id"), Some(Ref("@hisId", None)))))"#
        );
        assert_nom_fn_eq!(
            zinc_tag(r#"projName:"test""#),
            r#"Ok(("", Tag(Id("projName"), Some(EscapedString("test")))))"#
        );
        assert_nom_fn_eq!(
            tags(r#"id:@hisId"#),
            r#"Ok(("", [Tag(Id("id"), Some(Ref("@hisId", None)))]))"#
        );
        assert_nom_fn_eq!(
            tags(r#"projName:"test" \n"#),
            r#"Ok((" \\n", [Tag(Id("projName"), Some(EscapedString("test")))]))"#
        );
        assert_nom_fn_eq!(
            tags("projName:\"test\"\n"),
            r#"Ok(("\n", [Tag(Id("projName"), Some(EscapedString("test")))]))"#
        );
        assert_nom_fn_eq!(
            tags(r#"projName:"test" id:@hisId"#),
            r#"Ok(("", [Tag(Id("projName"), Some(EscapedString("test"))), Tag(Id("id"), Some(Ref("@hisId", None)))]))"#
        );
        assert_nom_fn_eq!(
            tags(r#"id:@hisId projName:"test""#),
            r#"Ok(("", [Tag(Id("id"), Some(Ref("@hisId", None))), Tag(Id("projName"), Some(EscapedString("test")))]))"#
        );
        assert_nom_fn_eq!(
            tags(r#"id:4 projName:"test""#),
            r#"Ok(("", [Tag(Id("id"), Some(Number(4.0, ""))), Tag(Id("projName"), Some(EscapedString("test")))]))"#
        );
        assert_nom_fn_eq!(
            tags("id:@hisId projName:\"test\""),
            r#"Ok(("", [Tag(Id("id"), Some(Ref("@hisId", None))), Tag(Id("projName"), Some(EscapedString("test")))]))"#
        );
        assert_nom_fn_eq!(parser("\n"), r#"Ok(("", '\n'))"#);
        assert_nom_fn_eq!(col("ts"), r#"Ok(("", Col(Id("ts"), Some([]))))"#);
        assert_nom_fn_eq!(
            col("dis dis:\"Equip Name\""),
            r#"Ok(("", Col(Id("dis"), Some([Tag(Id("dis"), Some(EscapedString("Equip Name")))]))))"#
        );
        assert_nom_fn_eq!(
            cols("ts,val"),
            r#"Ok(("", Cols([Col(Id("ts"), Some([])), Col(Id("val"), Some([]))])))"#
        );
        assert_nom_fn_eq!(
            cols("dis dis:\"Equip Name\",equip,siteRef,installed"),
            r#"Ok(("", Cols([Col(Id("dis"), Some([Tag(Id("dis"), Some(EscapedString("Equip Name")))])), Col(Id("equip"), Some([])), Col(Id("siteRef"), Some([])), Col(Id("installed"), Some([]))])))"#
        );
    }

    #[test]
    fn row_test() {
        use super::*;
        
        assert_eq!(
            row("1,2,4,5"),
            Ok(("", row![number!(1.0), number!(2.0), number!(4.0), number!(5.0)]))
        );

        assert_eq!(
            row("1,2,,5"),
            Ok(("", row![number!(1.0), number!(2.0), Token::Null, number!(5.0)]))
        );

        assert_eq!(
            row("1 , 2, ,5"),
            Ok(("", row![number!(1.0), number!(2.0), Token::Null, number!(5.0)]))
        );

        assert_eq!(
            row(r#"1,,2,,5,"projName",8,,9"#),
            Ok(("", row![number!(1.0), Token::Null, number!(2.0), Token::Null, number!(5.0),
                         Token::EscapedString("projName".to_string()), number!(8.0), Token::Null, number!(9.0)]))
        );
    }

    #[test]
    fn gridmeta_test() {
        use super::*;

        assert_nom_fn_eq!(
            grid_meta("ver:\"3.0\" projName:\"test\""),
            r#"Ok(("", GridMeta(Ver("3.0"), Some([Tag(Id("projName"), Some(EscapedString("test")))]))))"#
        );
        assert_nom_fn_eq!(
            grid_meta("ver:\"3.0\" projName:\"test\"\n"),
            r#"Ok(("\n", GridMeta(Ver("3.0"), Some([Tag(Id("projName"), Some(EscapedString("test")))]))))"#
        );
        assert_nom_fn_eq!(
            grid_meta("ver:\"3.0\" id:@hisId\n"),
            r#"Ok(("\n", GridMeta(Ver("3.0"), Some([Tag(Id("id"), Some(Ref("@hisId", None)))]))))"#
        );
        assert_nom_fn_eq!(
            grid_meta("ver:\"3.0\" projName:\"test\"\n"),
            r#"Ok(("\n", GridMeta(Ver("3.0"), Some([Tag(Id("projName"), Some(EscapedString("test")))]))))"#
        );
        assert_nom_fn_eq!(
            grid_meta("ver:\"3.0\" id:@hisId\n"),
            r#"Ok(("\n", GridMeta(Ver("3.0"), Some([Tag(Id("id"), Some(Ref("@hisId", None)))]))))"#
        );
        assert_nom_fn_eq!(
            grid_meta("ver:\"3.0\" id:[@619261,@619262] action:\"tags\"\n"),
            r#"Ok(("\n", GridMeta(Ver("3.0"), Some([Tag(Id("id"), Some(List([Ref("@619261", None), Ref("@619262", None)]))), Tag(Id("action"), Some(EscapedString("tags")))]))))"#
        );
    }

    #[test]
    fn grid_test() {
        use super::*;

        assert_nom_fn_eq!(
            grid("ver:\"3.0\"\nid,range\n@someTemp,\"2012-10-01\"\n"),
            r#"Ok(("", Grid(GridMeta(Ver("3.0"), Some([])), Cols([Col(Id("id"), Some([])), Col(Id("range"), Some([]))]), Rows([Row([Ref("@someTemp", None), EscapedString("2012-10-01")])]))))"#
        );

        assert_nom_fn_eq!(
            grid("ver:\"3.0\" id:@619265 action:\"tags\"\n"),
            r#"Ok(("", Grid(GridMeta(Ver("3.0"), Some([Tag(Id("id"), Some(Ref("@619265", None))), Tag(Id("action"), Some(EscapedString("tags")))])), Cols([]), Rows([]))))"#
        );

        assert_nom_fn_eq!(
            cols("params\n"),
            r#"Ok(("\n", Cols([Col(Id("params"), Some([]))])))"#
        );

        assert_nom_fn_eq!(
            grid("ver:\"3.0\" id:@619265 action:\"tags\"\nparams\n"),
            r#"Ok(("", Grid(GridMeta(Ver("3.0"), Some([Tag(Id("id"), Some(Ref("@619265", None))), Tag(Id("action"), Some(EscapedString("tags")))])), Cols([Col(Id("params"), Some([]))]), Rows([]))))"#
        );

        // zero rows get returned here as each row needs to end with a nl
        assert_nom_fn_eq!(
            grid("ver:\"3.0\" id:@619265 action:\"tags\"\nparams\n[]"),
            r#"Ok(("", Grid(GridMeta(Ver("3.0"), Some([Tag(Id("id"), Some(Ref("@619265", None))), Tag(Id("action"), Some(EscapedString("tags")))])), Cols([Col(Id("params"), Some([]))]), Rows([]))))"#
        );

        assert_nom_fn_eq!(
            grid("ver:\"3.0\" id:@619265 action:\"tags\"\nparams\n[]\n"),
            r#"Ok(("", Grid(GridMeta(Ver("3.0"), Some([Tag(Id("id"), Some(Ref("@619265", None))), Tag(Id("action"), Some(EscapedString("tags")))])), Cols([Col(Id("params"), Some([]))]), Rows([Row([List([])])]))))"#
        );

        // assert_nom_fn_eq!(
        //     grid("ver:\"3.0\" id:@619265 action:\"tags\"\nparams\n"),
        //     r#"Ok(("", GridMeta(Ver("3.0"), Some([Tag(Id("id"), Some(Ref("@619265", None))), Tag(Id("action"), Some(EscapedString("tags")))]))))"#
        // );

        // println!("{:?}", grid(r#"ver:"3.0" id:@619265 action:"tags"\n"#));

        // assert_nom_fn_eq!(
        //     grid(r#"ver:"3.0" id:@619265 action:"tags"\nparams\n"#),
        //     r#"Ok(("\\n", GridMeta(Ver("3.0"), Some([Tag(Id("id"), Some(Ref("@619265", None))), Tag(Id("action"), Some(EscapedString("tags")))]))))"#
        // );

        assert_nom_fn_eq!(
            grid_meta(r#"ver:"3.0"\n"#),
            r#"Ok(("\\n", GridMeta(Ver("3.0"), Some([]))))"#
        );

        assert_nom_fn_eq_no_remain_check!(
            grid_meta(
                r#"ver:"3.0"
                                    "#
            ),
            r#"GridMeta(Ver("3.0"), Some([]))"#
        );

        assert_nom_fn_eq_no_remain_check!(
            grid_meta(
                r#"ver:"3.0"
                        type,val
                        "list",[1,2,3]"#
            ),
            r#"GridMeta(Ver("3.0"), Some([]))"#
        );

        assert_nom_fn_eq!(
            ver(r#"ver:"3.0" projName:"test"\n"#),
            r#"Ok((" projName:\"test\"\\n", Ver("3.0")))"#
        );

        assert_nom_fn_eq!(
            grid_meta(r#"ver:"3.0" projName:"test"\n"#),
            r#"Ok(("\\n", GridMeta(Ver("3.0"), Some([Tag(Id("projName"), Some(EscapedString("test")))]))))"#
        );

        assert_nom_fn_eq!(
            grid_meta(r#"ver:"3.0" id:@619265 action:"tags"\n"#),
            r#"Ok(("\\n", GridMeta(Ver("3.0"), Some([Tag(Id("id"), Some(Ref("@619265", None))), Tag(Id("action"), Some(EscapedString("tags")))]))))"#
        );

        assert_nom_fn_eq!(
            cols(r#"type,val"#),
            r#"Ok(("", Cols([Col(Id("type"), Some([])), Col(Id("val"), Some([]))])))"#
        );

        assert_nom_fn_eq!(
            row(r#""RTU-1",M,@153c-699a "HQ",2005-06-01\n"#),
            r#"Ok(("\\n", Row([EscapedString("RTU-1"), Marker, Ref("@153c-699a", Some("HQ")), Date(2005-06-01)])))"#
        );

        assert_nom_fn_eq_no_remain_check!(
            row(r#""RTU-1",M,@153c-699a "HQ",2005-06-01
                        "#),
            r#"Row([EscapedString("RTU-1"), Marker, Ref("@153c-699a", Some("HQ")), Date(2005-06-01)])"#
        );

        assert_nom_fn_eq_no_remain_check!(
            row(r#""RTU-1",M,@153c-699a "HQ",2005-06-01
                                              "RTU-2",M,@153c-699a "HQ",1999-07-12"#),
            r#"Row([EscapedString("RTU-1"), Marker, Ref("@153c-699a", Some("HQ")), Date(2005-06-01)])"#
        );

        assert_nom_fn_eq_no_remain_check!(
            row(r#""list",[1,2,3]"#),
            r#"Row([EscapedString("list"), List([Number(ZincNumber { number: 1.0 }, ""), Number(ZincNumber { number: 2.0 }, ""), Number(ZincNumber { number: 3.0 }, "")])])"#
        );

        assert_nom_fn_eq_no_remain_check!(
            rows(
                r#""RTU-1",M,@153c-699a "HQ",2005-06-01
                   "RTU-2",M,@153c-699a "HQ",1999-07-12
                   "#
            ),
            r#"Rows([Row([EscapedString("RTU-1"), Marker, Ref("@153c-699a", Some("HQ")), Date(2005-06-01)]), Row([EscapedString("RTU-2"), Marker, Ref("@153c-699a", Some("HQ")), Date(1999-07-12)])])"#
        );

        assert_nom_fn_eq_no_remain_check!(
            grid_meta(
                r#"ver:"3.0"
                            type,val
                            "list",[1,2,3]"#
            ),
            r#"GridMeta(Ver("3.0"), Some([]))"#
        );

        assert_nom_fn_eq_no_remain_check!(
            cols("type,val\n\"list\",[1,2,3]"),
            r#"Cols([Col(Id("type"), Some([])), Col(Id("val"), Some([]))])"#
        );

        assert_nom_fn_eq_no_remain_check!(
            grid(
                r#"ver:"3.0"
                type,val
                "list",[1,2,3]
                "#
            ),
            r#"Grid(GridMeta(Ver("3.0"), Some([])), Cols([Col(Id("type"), Some([])), Col(Id("val"), Some([]))]), Rows([Row([EscapedString("list"), List([Number(ZincNumber { number: 1.0 }, ""), Number(ZincNumber { number: 2.0 }, ""), Number(ZincNumber { number: 3.0 }, "")])])]))"#
        );

        assert_nom_fn_eq_no_remain_check!(
            grid(
                r#"ver:"3.0"
            type,val
            "list",[1,2,3]
            "dict",{dis:"Dict!" foo}
            "grid",<<
            ver:"2.0"
            a,b
            1,2
            3,4
            >>
            "scalar","simple string"
            "#
            ),
            r#"Grid(GridMeta(Ver("3.0"), Some([])), Cols([Col(Id("type"), Some([])), Col(Id("val"), Some([]))]), Rows([Row([EscapedString("list"), List([Number(ZincNumber { number: 1.0 }, ""), Number(ZincNumber { number: 2.0 }, ""), Number(ZincNumber { number: 3.0 }, "")])]), Row([EscapedString("dict"), Dict({"dis": Some(Tag(Id("dis"), Some(EscapedString("Dict!")))), "foo": None})]), Row([EscapedString("grid"), Grid(GridMeta(Ver("2.0"), Some([])), Cols([Col(Id("a"), Some([])), Col(Id("b"), Some([]))]), Rows([Row([Number(ZincNumber { number: 1.0 }, ""), Number(ZincNumber { number: 2.0 }, "")]), Row([Number(ZincNumber { number: 3.0 }, ""), Number(ZincNumber { number: 4.0 }, "")])]))]), Row([EscapedString("scalar"), EscapedString("simple string")])]))"#
        );

        assert_nom_fn_eq_no_remain_check!(
            grid(
                r#"ver:"3.0"
            val,type
            [1,2,3], "list"
            {dis:"Dict!" foo}, "dict"
            <<
            ver:"2.0"
            a,b
            1,2
            3,4
            >>, "grid"
            "scalar","simple string"
            "#
            ),
            r#"Grid(GridMeta(Ver("3.0"), Some([])), Cols([Col(Id("val"), Some([])), Col(Id("type"), Some([]))]), Rows([Row([List([Number(ZincNumber { number: 1.0 }, ""), Number(ZincNumber { number: 2.0 }, ""), Number(ZincNumber { number: 3.0 }, "")]), EscapedString("list")]), Row([Dict({"dis": Some(Tag(Id("dis"), Some(EscapedString("Dict!")))), "foo": None}), EscapedString("dict")]), Row([Grid(GridMeta(Ver("2.0"), Some([])), Cols([Col(Id("a"), Some([])), Col(Id("b"), Some([]))]), Rows([Row([Number(ZincNumber { number: 1.0 }, ""), Number(ZincNumber { number: 2.0 }, "")]), Row([Number(ZincNumber { number: 3.0 }, ""), Number(ZincNumber { number: 4.0 }, "")])])), EscapedString("grid")]), Row([EscapedString("scalar"), EscapedString("simple string")])]))"#
        );
    }

    #[test]
    fn grid_his_read_test() {
        use super::*;

        assert_nom_fn_eq_no_remain_check!(
            grid(
                r#"ver:"3.0"
                id,range
                @someTemp,"2012-10-01,2012-10-03""#
            ),
            r#"Grid(GridMeta(Ver("3.0"), Some([])), Cols([Col(Id("id"), Some([])), Col(Id("range"), Some([]))]), Rows([Row([Ref("@someTemp", None), EscapedString("2012-10-01,2012-10-03")])]))"#
        );
    }

    #[test]
    fn grid_read_test() {
        use super::*;

        let s = "ver:\"3.0\"\nfilter\n\"carnego_campus == \\\"bryn_bragl\\\"\"\n";

        println!("s: {}", s);

        assert_nom_fn_eq_no_remain_check!(
            grid(s),
            r#"Grid(GridMeta(Ver("3.0"), Some([])), Cols([Col(Id("filter"), Some([]))]), Rows([Row([EscapedString("carnego_campus == \"bryn_bragl\"")])]))"#
        );
    }

    #[test]
    fn grid_read_rows_test() {
        use super::*;

        let s = "\"carnego_campus == \\\"bryn_bragl\\\"\"\n";

        println!("s: {}", s);

        assert_nom_fn_eq_no_remain_check!(
            rows(s),
            r#"Grid(GridMeta(Ver("3.0"), Some([])), Cols([Col(Id("id"), Some([])), Col(Id("range"), Some([]))]), Rows([Row([Ref("@someTemp", None), EscapedString("2012-10-01,2012-10-03")])]))"#
        );
    }

    #[test]
    fn it_works() {
        use super::*;

        assert_eq!(inf("Inf"), Ok(("", Token::Inf)));

        assert_eq!(inf("-Inf"), Ok(("", Token::InfNeg)));

        assert_eq!(nan("NaN"), Ok(("", Token::NaN)));

        assert_eq!(nan("-NaN"), Err(nom::Err::Error(("-NaN", ErrorKind::Tag))));

        assert_eq!(
            simple_number("32143"),
            Ok(("", Token::Number(ZincNumber::new(32143f64), "".into())))
        );
        assert_eq!(
            simple_number("2"),
            Ok(("", Token::Number(ZincNumber::new(2.0f64), "".into())))
        );
        assert_eq!(
            simple_number("32143.25"),
            Ok(("", Token::Number(ZincNumber::new(32143.25f64), "".into())))
        );
        assert_eq!(
            simple_number("-0.125"),
            Ok(("", Token::Number(ZincNumber::new(-0.125f64), "".into())))
        );
        assert_eq!(
            simple_number("+674.96"),
            Ok(("", Token::Number(ZincNumber::new(674.96f64), "".into())))
        );

        assert_eq!(number("1"), Ok(("", 1f64)));

        assert_eq!(number("-56"), Ok(("", -56f64)));

        assert_eq!(number("-34"), Ok(("", -34f64)));

        assert_eq!(number("5.4"), Ok(("", 5.4f64)));

        assert_eq!(number("-5.4"), Ok(("", -5.4f64)));

        assert_eq!(number("9.23"), Ok(("", 9.23f64)));

        assert_eq!(number("5.4e-45"), Ok(("", 5.4e-45f64)));

        assert_eq!(number("-5.4e-45"), Ok(("", -5.4e-45f64)));

        assert_eq!(number("67.3E7"), Ok(("", 67.3E7f64)));

        assert_eq!(zinc_number("1"), Ok(("", Token::Number(ZincNumber::new(1f64), "".into()))));

        assert_eq!(
            zinc_number("5.4"),
            Ok(("", Token::Number(ZincNumber::new(5.4f64), "".into())))
        );

        assert_eq!(
            zinc_number("-5.4"),
            Ok(("", Token::Number(ZincNumber::new(-5.4f64), "".into())))
        );

        assert_eq!(
            zinc_number("-5.4e-45"),
            Ok(("", Token::Number(ZincNumber::new(-5.4e-45f64), "".into())))
        );

        assert_eq!(
            zinc_number("67.3E7"),
            Ok(("", Token::Number(ZincNumber::new(67.3E7f64), "".into())))
        );

        assert_eq!(zinc_number("Inf"), Ok(("", Token::Inf)));

        assert_eq!(zinc_number("-Inf"), Ok(("", Token::InfNeg)));

        assert_eq!(zinc_number("NaN"), Ok(("", Token::NaN)));

        assert_eq!(
            zinc_number("-NaN"),
            Err(nom::Err::Error(("-NaN", ErrorKind::Tag)))
        );

        assert_eq!(
            zinc_number("-5.4e-45Kg"),
            Ok(("", Token::Number(ZincNumber::new(-5.4e-45f64), "Kg".into())))
        );

        assert_eq!(null("N"), Ok(("", Token::Null)));

        assert_ne!(null("n"), Ok(("", Token::Null)));

        assert_eq!(
            quoted_string("\"foo\nbar\""),
            Ok(("", Token::EscapedString("foo\nbar".into())))
        );

        assert_eq!(
            quoted_string("\"abc\""),
            Ok(("", Token::EscapedString("abc".into())))
        );

        assert_eq!(
            zinc_ref("@153c-699a \"HQ\""),
            Ok(("", Token::Ref("@153c-699a".into(), Some("HQ".into()))))
        );
    }

    #[test]
    fn test_uri() {
        assert_eq!(
            uri("`http://foo.com/f?q`"),
            Ok(("", Token::Uri("foo\nbar".into())))
        );
    }

    #[test]
    fn write_dict() {
        let now: DateTime<FixedOffset> = DateTime::<FixedOffset>::from(Utc::now());

        let d = Dict::new(&vec![
            Tag::new_from_token(
                Token::EscapedString("haystackVersion".into()),
                Token::EscapedString("3.0".into()),
            ),
            Tag::new_from_token(
                Token::EscapedString("serverTime".into()),
                Token::DateTime(now),
            ),
            Tag::new_from_token(
                Token::EscapedString("tz".into()),
                Token::EscapedString("UTC".into()),
            ),
        ]);

        println!("{}", d.to_zinc());
    }

    #[test]
    fn about_uri() {
        let grid_metadata = GridMeta::new(Token::Ver("3.0".into()), None);

        println!("{:?}", grid_meta(&grid_metadata.to_zinc()));

        let now: DateTime<FixedOffset> = DateTime::<FixedOffset>::from(Utc::now());

        // Response: single row grid with following columns:
        let cols_obj = Cols::new(vec![
            Col::new(Token::Id("serverTime".into()), None),
            Col::new(Token::Id("tz".into()), None),
        ]);

        println!("{:?}", cols(&cols_obj.to_zinc()));

        let row = Row::new(vec![
            Val::new(Box::new(Token::DateTime(now))),
            Val::new(Box::new(Token::EscapedString("UTC".into()))),
        ]);

        let grid_obj = Grid::new(grid_metadata, cols_obj, Rows::new(vec![row]));

        println!("{:?}", grid_obj.to_zinc());
        println!("{}", grid_obj.to_zinc());

        let s = grid_obj.to_zinc();

        println!("{:?}", grid(&s));

        assert!(grid(&s).is_ok());
    }

    #[test]
    fn grid_read_filter_test() {
        use super::*;

        assert_nom_fn_eq_no_remain_check!(
            grid(
                r#"ver:"3.0"
                filter,limit
                "point and siteRef==@siteA",1000"#
            ),
            r#"Grid(GridMeta(Ver("3.0"), Some([])), Cols([Col(Id("filter"), Some([])), Col(Id("limit"), Some([]))]), Rows([Row([EscapedString("point and siteRef==@siteA"), Number(ZincNumber { number: 1000.0 }, "")])]))"#
        );


    }

    #[test]
    fn token_test2() {
        use super::*;

        assert_ne!(
            token("elec"),
            Ok(("", Token::EscapedString("elec".into())))
        );

        assert_eq!(
            token("\"elec\""),
            Ok(("", Token::EscapedString("elec".into())))
        );

        assert_eq!(
            token("\"bryn_bragl.bb2012.supply_to_pkom4_meter.pkcom4_supply_power\""),
            Ok(("", Token::EscapedString("bryn_bragl.bb2012.supply_to_pkom4_meter.pkcom4_supply_power".into())))
        );
    }
}
