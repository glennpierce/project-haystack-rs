//! Tokenizer that converts a zinc string form into a series of `Token`s.
use nom::{
  branch::alt,
  bytes::complete::{escaped, take_while_m_n, is_a, tag, take_while1},
  character::{is_digit},
  character::complete::{newline, space0, space1, multispace0, multispace1, alphanumeric1, char, one_of, digit1, alpha1},
  combinator::{complete, peek, recognize, map, opt},
  error::{ErrorKind},
  number::complete::double,
  multi::{many1, separated_list},
  sequence::{delimited, preceded, tuple, terminated, separated_pair},
  Err, IResult
};

use escape8259::{escape, unescape};

use std::collections::{HashMap};

use chrono::{DateTime, Local, FixedOffset, NaiveDateTime, TimeZone, Utc};
use dtparse::parse;
use dtparse::ParseError;

use crate::hval::{HVal};
use crate::token::*;

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

// fn val_map<F, I, O, E>(f: F) -> impl Fn(I) -> IResult<I, X, E>
// where
//     F: Fn(I) -> IResult<I, O, E>,
//     I: nom::InputTakeAtPosition,
//     X: Val,
//     <I as nom::InputTakeAtPosition>::Item: nom::AsChar + Clone,
//     E: nom::error::ParseError<I>,
// {
//     map(f, |v| { Val::new(Box::new(v) as Box<dyn HVal>) })(i)
// }

fn comma<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(tag(","), |_: &str| Token::Comma)(i)
}

fn comma_val<'a>(i: &'a str) -> IResult<&'a str, Val, (&'a str, ErrorKind)> {
    map(tag(","),
      |_: &str| 
        {
            Val::new(Box::new(Comma::new()) as Box<dyn HVal>)
        }
    )(i)
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
    map(alt((tag("T"), tag("F"))), |o: &str| if o == "F" { Token::Bool(false) } else { Token::Bool(true) }) (i)
}

fn inf<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {

    map(tuple((opt(char('-')), tag("INF"))), |(o, _): (std::option::Option<char>, &str)| if o.is_some(){ Token::INF_NEG } else { Token::INF }) (i)
}

fn nan<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(tag("NAN"), |_: &str| Token::NAN)(i)
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

fn quoted_string<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(quoted_string_s, |s: String| Token::EscapedString(s.to_string()))(i)
}

fn unicode_alpha0(i: &str) -> nom::IResult<&str, &str> {
    nom_unicode::complete::alpha0(i)
}

fn uri<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    let qs = preceded(tag("`"), unicode_alpha0);
    map(terminated(qs, tag("`")), |s: &str| Token::Uri(s.to_string()))(i)
}

fn negpos_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
 
    map(
        opt(alt((tag("+"), tag("-"), ))),
        |o: Option<&str>| o.unwrap_or("")
    )(i)
}


fn negpos_i32<'a>(i: &'a str) -> IResult<&'a str, i32, (&'a str, ErrorKind)> {
 
    map(
        alt((tag("+"), tag("-"), )),
        |s: &str| 
            {
                match s {
                    "-" => -1i32,
                    "+" => 1i32,
                    _ => 1,
                }
            }
    )(i)
}

// 2011-06-07
// YYYY-MM-DD
fn date_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple((digit1, char('-'), digit1, char('-'), digit1)) )(i)
}

fn date<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(date_s, |s: &str| Token::Date( dtparse::parse(s).unwrap().0.date() ) )(i)
}

fn hours_minutes_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple((digit1, char(':'), digit1)) )(i)
}

// hh:mm:ss
fn time_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple((digit1, char(':'), digit1, char(':'), digit1)) )(i)
}

// hh:mm:ss.FFFFFFFFF
fn time_with_subseconds<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple((time_s, opt(tuple((char('.'), digit1))) )))(i)
}

// 2012-09-29T14:56:18.277Z UTC
// 2012-09-29T14:56:18.277Z
// 2011-06-07T09:51:27-04:00 New_York
// 2011-06-07T09:51:27+06:00
// Z
// -04:00 New_York
// z zzzz
fn timeoffset_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    map(recognize(tuple( (negpos_s, hours_minutes_s ) ) ), |s: &str| s)(i)
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
    map(recognize(tuple( (alt((z_s, timeoffset_s)), opt(tuple((tag(" "), timezone_char))) ))),
        |s: &str| s )(i)
}

// 2011-06-07T09:51:27-04:00 New_York
// YYYY-MM-DD'T'hh:mm:ss.FFFFFFFFFz zzzz
fn datetime_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    map(recognize(tuple((date, char('T'), time_with_subseconds, timezone_s))), |s: &str| s)(i)
}

fn datetime<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(datetime_s, |s: &str| 
        {
            // First split off tz name at space
            let vec: Vec<&str> = s.split(' ').collect::<Vec<&str>>();

            let tmp: (NaiveDateTime, Option<FixedOffset>);

            if vec.len() > 1 {
                tmp = dtparse::parse(vec[0]).unwrap();
            }
            else {
                tmp = dtparse::parse(s).unwrap();
            }
    
            let dt = tmp.1.expect("Timezone is None").from_local_datetime(&tmp.0).unwrap();

            Token::DateTime(dt) 
        }
    )(i)
}

fn ident<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
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
      )
    )(i)
  }

fn zinc_id<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(complete(ident), |s: &str| Token::Id(s.into()))(i)
}

fn is_char_digit(chr: char) -> bool {
    return chr.is_ascii() && is_digit(chr as u8)
}

fn digits<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    take_while1(is_char_digit)(i)
}

// Number: 1, -34, 5.4, -5.4, 9.23, 74.2, 4,
fn simple_number_s<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {

    map(
        recognize(tuple( (opt(alt((char('-'), char('+')))), many1(digit1), opt(preceded(char('.'), many1(digit1)))) ) ),
        |s: &str| s)(i)
}

// Number: 1, -34, 5.4, -5.4, 9.23, 74.2, 4,
fn simple_number<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {

    map(
        recognize(tuple( (opt(alt((char('-'), char('+')))), many1(digit1), opt(preceded(char('.'), many1(digit1)))) ) ),
        |s: &str| Token::Number(s.parse::<f64>().unwrap(), "".into())
    )(i)
}

fn exponent<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple( ( alt((char('e'), char('E'))), simple_number  )) )(i)
}

fn number<'a>(i: &'a str) -> IResult<&'a str, f64, (&'a str, ErrorKind)> {
    map(
        recognize(tuple((simple_number, opt(exponent))) ),
        |s: &str| s.parse::<f64>().unwrap()
    )(i)
}

fn units<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    alphanumeric1(i)
}

fn number_with_unit<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(
        tuple((number, opt(units))),
        |t: (f64, Option<&str>)| Token::Number(t.0, t.1.unwrap_or(&"".to_string()).into())
    )(i)
}

// Number: 1, -34, 5.4, -5.4, 9.23, 74.2, 4, 5.4e-45, -5.4e-45, 67.3E7 INF -INF +INF NAN
fn zinc_number<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {

    alt( (number_with_unit, inf, nan) ) (i)
}

//println!("{:?}", zinc_ref(r#"@hisId"#));
// <ref>         := "@" <refChar>* [ " " <str> ]
// <refChar>     := <alpha> | <digit> | "_" | ":" | "-" | "." | "~"

fn ref_char<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    let allowed_chars: &str = "_:-.~abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    is_a(allowed_chars)(i)
}

// println!("{:?}", zinc_ref(r#"@hisId"#));
fn zinc_ref<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {

    map(
        tuple((tag("@"), ref_char, opt(preceded(multispace0, quoted_string_s)))),
        |t: (&str, &str, Option<String>)| {
            Token::Ref("@".to_string() + t.1, t.2)
        }
    )(i)
}

fn ver<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(
        separated_pair(tag("ver"), char(':'), quoted_string_s),
        |t: ( &str, String)| {
            Token::Ver(t.1.to_string())
        }
    )(i)
}

fn zinc_marker_tag<'a>(i: &'a str) -> IResult<&'a str, (Token, Option<Token>), (&'a str, ErrorKind)> {
    map(
        zinc_id,
        |t: Token| {
            (t, None)
        }
    )(i)
}

fn token<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    alt((zinc_ref, quoted_string, uri, datetime, date, zinc_number, bool, na, null, marker, remove))(i)
}

// fn scalar<'a>(i: &'a str) -> IResult<&'a str, Box<dyn HVal>, (&'a str, ErrorKind)> {
//     map(
//         token,
//         |t: Token| {
//             Box::new(Scaler::new(t.clone())) as Box<dyn HVal>
//         }   
//     )(i)
// }


// fn scalar<'a>(i: &'a str) -> IResult<&'a str, Box<dyn HVal>, (&'a str, ErrorKind)> {
//     map(
//         token,
//         |t: Token| {
//             Box::new(Scaler::new(t.clone())) as Box<dyn HVal>
//         }   
//     )(i)
// }


fn scalar<'a>(i: &'a str) -> IResult<&'a str, Val, (&'a str, ErrorKind)> {
    map(
        token,
        |t: Token| {
            Val::new(Box::new(Scaler::new(t.clone())) as Box<dyn HVal>)
        }   
    )(i)
}

// fn scalar<'a>(i: &'a str) -> IResult<&'a str, Val, (&'a str, ErrorKind)> {
//     map(
//         token,
//         |t: Token| {
//             Box::new(Scaler::new(t.clone())) as Box<dyn HVal>
//         }   
//     )(i)
// }

// "id:@hisId"
fn zinc_tag_pair<'a>(i: &'a str) -> IResult<&'a str, (Token, Option<Token>), (&'a str, ErrorKind)> {
    map(
        separated_pair(zinc_id, char(':'), token),
        |t: (Token, Token)| {
            (t.0, Some(t.1))
        }
    )(i)
}

// Tag(Box<Token>, Box<Option<Token>>)

fn zinc_tag<'a>(i: &'a str) -> IResult<&'a str, Tag, (&'a str, ErrorKind)> {
    map(
        alt((zinc_tag_pair, zinc_marker_tag)),
        |t: (Token, Option<Token>)| {
            //Token::Tag(Box::new(t.0), Box::new(t.1))
            Tag::new(t.0, t.1)
        }
    )(i)  
}

// id:@hisId projName:"test"
fn tags<'a>(i: &'a str) -> IResult<&'a str, Tags, (&'a str, ErrorKind)> {
    //terminated(separated_list(char(' '), scalar), opt(tag(",")))(i) 
    map(
        separated_list(char(' '), zinc_tag),
        |t: Vec<Tag>| {
            Tags::new(&t)
        }
    )(i)  
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
        delimited(spacey(tag("{")), tags, spacey(tag("}"))),
        |tags: Tags| {

            // Box::new(Val::new(t.clone())) as Box<dyn HVal>
            Val::new(Box::new(Dict::new_from_tags(&tags)) as Box<dyn HVal>)
        }
    )(i)  
}

fn list_of_vals<'a>(i: &'a str) -> IResult<&'a str, Vec<Val>, (&'a str, ErrorKind)> {
    terminated(separated_list(spacey(tag(",")), scalar), opt(tag(",")))(i) 
}

fn list<'a>(i: &'a str) -> IResult<&'a str, Val, (&'a str, ErrorKind)> {

    //delimited(spacey(tag("[")), list_of_vals, spacey(tag("]")))(i)
    
    map(
        delimited(spacey(tag("[")), list_of_vals, spacey(tag("]"))),
        |v: Vec<Val>| {
            //let tmp: Vec<Box<Token>> = v.into_iter().map(|x| Box::new(x)).collect();
            Val::new(Box::new(List::new(v)) as Box<dyn HVal>)
        }
    )(i)
}

fn col<'a>(i: &'a str) -> IResult<&'a str, Col, (&'a str, ErrorKind)> {
     
    map(
       tuple((zinc_id, space0, opt(tags))),
       |t: (Token,  _, Option<Tags>)| {

           let id: Token = t.0;
           let tags: Option<Tags> = t.2; 

           Col::new(id, tags)
       }
   )(i)  
}

fn cols<'a>(i: &'a str) -> IResult<&'a str, Cols, (&'a str, ErrorKind)> {
    map(
        separated_list(spacey(char(',')), col),
        |v: Vec<Col>| {
            println!("hmm {:?}", v);
            Cols::new(v)
        }
    )(i)
}

fn val<'a>(i: &'a str) -> IResult<&'a str, Val, (&'a str, ErrorKind)> {
    alt((sub_grid, list, dict, scalar))(i)
}

// pub struct Val {
//     pub token: Box<dyn HVal>,
// }

fn cell<'a>(i: &'a str) -> IResult<&'a str, Val, (&'a str, ErrorKind)> {
    map(
        alt((val, peek(comma_val))),
            |v: Val| {
                let s = v.to_string();

                match s.as_ref() {
                    "," => Val::new(Box::new(Scaler::new(Token::Null)) as Box<dyn HVal>),
                    _ => v // Val::new(Box::new(Scaler::new(t.clone())) as Box<dyn HVal>)
                }
            }  
        )(i)
}

fn row<'a>(i: &'a str) -> IResult<&'a str, Row, (&'a str, ErrorKind)> {
    //separated_list(spacey(char(',')), cell)(i)

    map(
        separated_list(spacey(char(',')), cell),
        |v: Vec<Val>| {
            // let tmp: Vec<Box<Token>> = v.into_iter().map(|x| Box::new(x)).collect();
            // Token::Row(tmp)

            Row::new(v)
        }
    )(i)  
}

// return Token::Rows(Vec<Box<Token>>),
fn rows<'a>(i: &'a str) -> IResult<&'a str, Rows, (&'a str, ErrorKind)> { 
    map(
        separated_list(spacey(nl), row),   // list of rows seperated by newline
        |v: Vec<Row>| {
            // let tmp: Vec<Box<Token>> = v.into_iter().map(|x| Box::new(x)).collect();
            // Token::Rows(tmp)
            Rows::new(v)
        }
    )(i)  
}

// ver:"3.0" projName:"test""
// GridMeta(Box<Token>, Option<Box<Token>>), 
fn grid_meta<'a>(i: &'a str) -> IResult<&'a str, GridMeta, (&'a str, ErrorKind)> {
    map(
        tuple((ver, space0, opt(tags))),
        |t: (Token,  _, Option<Tags>)| {
            GridMeta::new(t.0, t.2)
        }
    )(i)  
}

// <grid>        :=  <gridMeta> <cols> [<row>]*
pub fn grid<'a>(i: &'a str) -> IResult<&'a str, Grid, (&'a str, ErrorKind)> {
    map(
        tuple((grid_meta, multispace1, cols, multispace1, rows)),
        |t: (GridMeta,  _, Cols, _, Rows)| {
            //println!("------- {:?}", t);
            //Token::Grid(Box::new(t.0), Box::new(t.2), Box::new(t.4))
            Grid::new(t.0, t.2, t.4)
        }
    )(i)  
}

// <grid>        :=  "<<" <grid> ">>"
fn sub_grid<'a>(i: &'a str) -> IResult<&'a str, Val, (&'a str, ErrorKind)> {

    map(
        delimited(delimited(space0, tag("<<"), multispace0),
                  grid,
                  delimited(space0, tag(">>"), space0)), 
            |g: Grid| {
                Val::new(Box::new(g) as Box<dyn HVal>)
            }
    )(i)  
}

#[cfg(test)]
mod tests {
    use super::*;

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

        ($a:expr, $b:expr) => {

            {
                let mut tmp: String = "".into();

                let m = match $a {
                    Ok(v) => {

                        tmp = v.1.to_string();
                        tmp == $b.to_string()
                    },
                    Err(_) => false
                };

                if !m {
                    
                    println!("Left:\n{}", tmp);
                    println!("\n");
                    println!("Right:\n{}", $b.to_string());
                }

                assert!(m);
            }
        };
    }

    #[test]
    fn val_test() {
        use super::*;

        let comma = Val::new(Box::new(Token::Comma));

        assert_eq!(comma.to_string(), ",".to_string());
    }

    #[test]
    fn date_test() {
        use super::*;

        assert_eq!(time_s("23:33:07"), Ok(("", "23:33:07")));
        assert_eq!(time_with_subseconds("23:33:07.087642"), Ok(("", "23:33:07.087642")));
        assert_eq!(timezone_s("Z UTC"), Ok(("", "Z UTC")));
        assert_ne!(timezone_s("X UTC"), Ok(("", "X UTC")));
        assert_eq!(timezone_s("Z"), Ok(("", "Z")));
        assert_eq!(hours_minutes_s("03:00"), Ok(("", "03:00")));
        assert_eq!(timeoffset_s("03:00"), Ok(("", "03:00")));
        assert_eq!(timeoffset_s("+03:00"), Ok(("", "+03:00")));
        assert_eq!(timezone_s("+06:00"), Ok(("", "+06:00")));

        let mut dt = DateTime::parse_from_rfc3339("2012-09-29T14:56:18.277Z").unwrap();

        assert_eq!(datetime("2012-09-29T14:56:18.277Z"),
            Ok(("", Token::DateTime(dt))));

        dt = DateTime::parse_from_rfc3339("2011-06-07T09:51:27-04:00").unwrap();

        assert_eq!(datetime("2011-06-07T09:51:27-04:00 New_York"),
            Ok(("", Token::DateTime(dt))));
    }

    #[test]
    fn list_test() {
        use super::*;
        use Token::*;

        assert_eq!(
            zinc_number("32143m"),
            Ok(("", Token::Number(32143f64, "m".into())))
        );

        assert_nom_fn_eq!(list("[6,8,9]"), r#"Ok(("", List([Number(6.0, ""), Number(8.0, ""), Number(9.0, "")])))"#);
        assert_nom_fn_eq!(list("[ 6,  8, 9]"), r#"Ok(("", List([Number(6.0, ""), Number(8.0, ""), Number(9.0, "")])))"#);
        assert_nom_fn_eq!(list("[ 6,8,9,]"), r#"Ok(("", List([Number(6.0, ""), Number(8.0, ""), Number(9.0, "")])))"#);
        assert_nom_fn_eq!(list("[6m,8m,9m]"), r#"Ok(("", List([Number(6.0, "m"), Number(8.0, "m"), Number(9.0, "m")])))"#);
        assert_nom_fn_eq!(list("[6,T,F]"), r#"Ok(("", List([Number(6.0, ""), Bool(true), Bool(false)])))"#);
        assert_nom_fn_eq!(list("[6,T, F]"), r#"Ok(("", List([Number(6.0, ""), Bool(true), Bool(false)])))"#);
        assert_nom_fn_eq!(list("[6,R,M,F]"), r#"Ok(("", List([Number(6.0, ""), Remove, Marker, Bool(false)])))"#);
        assert_nom_fn_eq!(list("[6,NA,M,F]"), r#"Ok(("", List([Number(6.0, ""), NA, Marker, Bool(false)])))"#);
    }

    #[test]
    fn dict_test() {
        use super::*;
        use Token::*;

        assert_eq!(
            zinc_number("32143m"),
            Ok(("", Token::Number(32143f64, "m".into())))
        );

        assert_nom_fn_is_ok!(dict(r#"{id:@hisId projName:"test"}"#));
        assert_nom_fn_is_err!(dict(r#"{id:@hisId   projName:"test"}"#));
        
        assert_nom_fn_eq!(tags(r#"projName:"test" id:@hisId"#), r#"Ok(("", [Tag(Id("projName"), Some(EscapedString("test"))), Tag(Id("id"), Some(Ref("@hisId", None)))]))"#);
        
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

        assert_nom_fn_eq!(ver(r#"ver:"3.0" projName:"test""#), r#"Ok((" projName:\"test\"", Ver("3.0")))"#);
        assert_nom_fn_eq!(zinc_ref(r#"@hisId 4"#), r#"Ok((" 4", Ref("@hisId", None)))"#);
        assert_nom_fn_eq!(zinc_tag_pair(r#"id:@hisId"#), r#"Ok(("", (Id("id"), Some(Ref("@hisId", None)))))"#);
        assert_nom_fn_eq!(zinc_tag(r#"id:@hisId"#), r#"Ok(("", Tag(Id("id"), Some(Ref("@hisId", None)))))"#);
        assert_nom_fn_eq!(zinc_tag(r#"projName:"test""#), r#"Ok(("", Tag(Id("projName"), Some(EscapedString("test")))))"#);
        assert_nom_fn_eq!(tags(r#"id:@hisId"#), r#"Ok(("", [Tag(Id("id"), Some(Ref("@hisId", None)))]))"#);
        assert_nom_fn_eq!(tags(r#"projName:"test" \n"#), r#"Ok((" \\n", [Tag(Id("projName"), Some(EscapedString("test")))]))"#);
        assert_nom_fn_eq!(tags("projName:\"test\"\n"), r#"Ok(("\n", [Tag(Id("projName"), Some(EscapedString("test")))]))"#);
        assert_nom_fn_eq!(tags(r#"projName:"test" id:@hisId"#), r#"Ok(("", [Tag(Id("projName"), Some(EscapedString("test"))), Tag(Id("id"), Some(Ref("@hisId", None)))]))"#);
        assert_nom_fn_eq!(tags(r#"id:@hisId projName:"test""#), r#"Ok(("", [Tag(Id("id"), Some(Ref("@hisId", None))), Tag(Id("projName"), Some(EscapedString("test")))]))"#);
        assert_nom_fn_eq!(tags(r#"id:4 projName:"test""#), r#"Ok(("", [Tag(Id("id"), Some(Number(4.0, ""))), Tag(Id("projName"), Some(EscapedString("test")))]))"#);
        assert_nom_fn_eq!(tags("id:@hisId projName:\"test\""), r#"Ok(("", [Tag(Id("id"), Some(Ref("@hisId", None))), Tag(Id("projName"), Some(EscapedString("test")))]))"#);
        assert_nom_fn_eq!(parser("\n"), r#"Ok(("", '\n'))"#);
        assert_nom_fn_eq!(col("ts"), r#"Ok(("", Col(Id("ts"), Some([]))))"#);
        assert_nom_fn_eq!(col("dis dis:\"Equip Name\""), r#"Ok(("", Col(Id("dis"), Some([Tag(Id("dis"), Some(EscapedString("Equip Name")))]))))"#);
        assert_nom_fn_eq!(cols("ts,val"), r#"Ok(("", Cols([Col(Id("ts"), Some([])), Col(Id("val"), Some([]))])))"#);
        assert_nom_fn_eq!(cols("dis dis:\"Equip Name\",equip,siteRef,installed"), r#"Ok(("", Cols([Col(Id("dis"), Some([Tag(Id("dis"), Some(EscapedString("Equip Name")))])), Col(Id("equip"), Some([])), Col(Id("siteRef"), Some([])), Col(Id("installed"), Some([]))])))"#);
    }

    #[test]
    fn row_test() {
        use super::*;
        use Token::*;

        assert_nom_fn_eq!(row("1,2,4,5"),
            r#"Ok(("", Row([Number(1.0, ""), Number(2.0, ""), Number(4.0, ""), Number(5.0, "")])))"#);
      
        assert_nom_fn_eq!(row(r#"1,2,,5"#),
            r#"Ok(("", Row([Number(1.0, ""), Number(2.0, ""), Null, Number(5.0, "")])))"#);

        assert_nom_fn_eq!(row(r#"1 , 2, ,5"#),
            r#"Ok(("", Row([Number(1.0, ""), Number(2.0, ""), Null, Number(5.0, "")])))"#);

        assert_nom_fn_eq!(row(r#"1,,2,,5,"projName",8,,9"#),
            r#"Ok(("", Row([Number(1.0, ""), Null, Number(2.0, ""), Null, Number(5.0, ""), EscapedString("projName"), Number(8.0, ""), Null, Number(9.0, "")])))"#);
    }

    #[test]
    fn gridmeta_test() {
        use super::*;

        assert_nom_fn_eq!(grid_meta("ver:\"3.0\" projName:\"test\""), r#"Ok(("", GridMeta(Ver("3.0"), Some([Tag(Id("projName"), Some(EscapedString("test")))]))))"#);
        assert_nom_fn_eq!(grid_meta("ver:\"3.0\" projName:\"test\"\n"), r#"Ok(("\n", GridMeta(Ver("3.0"), Some([Tag(Id("projName"), Some(EscapedString("test")))]))))"#);
        assert_nom_fn_eq!(grid_meta("ver:\"3.0\" id:@hisId\n"), r#"Ok(("\n", GridMeta(Ver("3.0"), Some([Tag(Id("id"), Some(Ref("@hisId", None)))]))))"#);
        assert_nom_fn_eq!(grid_meta("ver:\"3.0\" projName:\"test\"\n"), r#"Ok(("\n", GridMeta(Ver("3.0"), Some([Tag(Id("projName"), Some(EscapedString("test")))]))))"#);
        assert_nom_fn_eq!(grid_meta("ver:\"3.0\" id:@hisId\n"), r#"Ok(("\n", GridMeta(Ver("3.0"), Some([Tag(Id("id"), Some(Ref("@hisId", None)))]))))"#);
    }

    #[test]
    fn grid_test() {
        use super::*;

        assert_nom_fn_eq!(grid("ver:\"3.0\"\nid,range\n@someTemp,\"2012-10-01\""),
          r#"Ok(("", Grid(GridMeta(Ver("3.0"), Some([])), Cols([Col(Id("id"), Some([])), Col(Id("range"), Some([]))]), Rows([Row([Ref("@someTemp", None), EscapedString("2012-10-01")])]))))"#);

        assert_nom_fn_eq!(grid_meta(r#"ver:"3.0"\n"#), r#"Ok(("\\n", GridMeta(Ver("3.0"), Some([]))))"#);

        assert_nom_fn_eq_no_remain_check!(grid_meta(r#"ver:"3.0"
                                    "#), r#"GridMeta(Ver("3.0"), Some([]))"#);

        assert_nom_fn_eq_no_remain_check!(grid_meta(r#"ver:"3.0"
                        type,val
                        "list",[1,2,3]"#),
                        r#"GridMeta(Ver("3.0"), Some([]))"#);

        assert_nom_fn_eq!(ver(r#"ver:"3.0" projName:"test"\n"#), r#"Ok((" projName:\"test\"\\n", Ver("3.0")))"#);

        assert_nom_fn_eq!(grid_meta(r#"ver:"3.0" projName:"test"\n"#), r#"Ok(("\\n", GridMeta(Ver("3.0"), Some([Tag(Id("projName"), Some(EscapedString("test")))]))))"#);

        assert_nom_fn_eq!(cols(r#"type,val"#), r#"Ok(("", Cols([Col(Id("type"), Some([])), Col(Id("val"), Some([]))])))"#);

        assert_nom_fn_eq!(row(r#""RTU-1",M,@153c-699a "HQ",2005-06-01\n"#), r#"Ok(("\\n", Row([EscapedString("RTU-1"), Marker, Ref("@153c-699a", Some("HQ")), Date(2005-06-01)])))"#);

        assert_nom_fn_eq_no_remain_check!(row(r#""RTU-1",M,@153c-699a "HQ",2005-06-01
                        "#),
                        r#"Row([EscapedString("RTU-1"), Marker, Ref("@153c-699a", Some("HQ")), Date(2005-06-01)])"#);

        assert_nom_fn_eq_no_remain_check!(row(r#""RTU-1",M,@153c-699a "HQ",2005-06-01
                                              "RTU-2",M,@153c-699a "HQ",1999-07-12"#),
                        r#"Row([EscapedString("RTU-1"), Marker, Ref("@153c-699a", Some("HQ")), Date(2005-06-01)])"#);

        assert_nom_fn_eq_no_remain_check!(row(r#""list",[1,2,3]"#), r#"Row([EscapedString("list"), List([Number(1.0, ""), Number(2.0, ""), Number(3.0, "")])])"#);

        assert_nom_fn_eq_no_remain_check!(rows(r#""RTU-1",M,@153c-699a "HQ",2005-06-01
        "RTU-2",M,@153c-699a "HQ",1999-07-12"#),
  r#"Rows([Row([EscapedString("RTU-1"), Marker, Ref("@153c-699a", Some("HQ")), Date(2005-06-01)]), Row([EscapedString("RTU-2"), Marker, Ref("@153c-699a", Some("HQ")), Date(1999-07-12)])])"#);

        assert_nom_fn_eq_no_remain_check!(grid_meta(r#"ver:"3.0"
                            type,val
                            "list",[1,2,3]"#), r#"GridMeta(Ver("3.0"), Some([]))"#);

        assert_nom_fn_eq_no_remain_check!(cols("type,val\n\"list\",[1,2,3]"), r#"Cols([Col(Id("type"), Some([])), Col(Id("val"), Some([]))])"#);

        
        assert_nom_fn_eq_no_remain_check!(grid(r#"ver:"3.0"
                type,val
                "list",[1,2,3]"#),
                r#"Grid(GridMeta(Ver("3.0"), Some([])), Cols([Col(Id("type"), Some([])), Col(Id("val"), Some([]))]), Rows([Row([EscapedString("list"), List([Number(1.0, ""), Number(2.0, ""), Number(3.0, "")])])]))"#);


        assert_nom_fn_eq_no_remain_check!(grid(r#"ver:"3.0"
            type,val
            "list",[1,2,3]
            "dict",{dis:"Dict!" foo}
            "grid",<<
            ver:"2.0"
            a,b
            1,2
            3,4
            >>
            "scalar","simple string""#),
                r#"Grid(GridMeta(Ver("3.0"), Some([])), Cols([Col(Id("type"), Some([])), Col(Id("val"), Some([]))]), Rows([Row([EscapedString("list"), List([Number(1.0, ""), Number(2.0, ""), Number(3.0, "")])]), Row([EscapedString("dict"), Dict({"dis": Some(Tag(Id("dis"), Some(EscapedString("Dict!")))), "foo": None})]), Row([EscapedString("grid"), Grid(GridMeta(Ver("2.0"), Some([])), Cols([Col(Id("a"), Some([])), Col(Id("b"), Some([]))]), Rows([Row([Number(1.0, ""), Number(2.0, "")]), Row([Number(3.0, ""), Number(4.0, "")]), Row([])]))]), Row([EscapedString("scalar"), EscapedString("simple string")])]))"#);


        assert_nom_fn_eq_no_remain_check!(grid(r#"ver:"3.0"
            val,type
            [1,2,3], "list"
            {dis:"Dict!" foo}, "dict"
            <<
            ver:"2.0"
            a,b
            1,2
            3,4
            >>, "grid"
            "scalar","simple string""#),
                    r#"Grid(GridMeta(Ver("3.0"), Some([])), Cols([Col(Id("val"), Some([])), Col(Id("type"), Some([]))]), Rows([Row([List([Number(1.0, ""), Number(2.0, ""), Number(3.0, "")]), EscapedString("list")]), Row([Dict({"dis": Some(Tag(Id("dis"), Some(EscapedString("Dict!")))), "foo": None}), EscapedString("dict")]), Row([Grid(GridMeta(Ver("2.0"), Some([])), Cols([Col(Id("a"), Some([])), Col(Id("b"), Some([]))]), Rows([Row([Number(1.0, ""), Number(2.0, "")]), Row([Number(3.0, ""), Number(4.0, "")]), Row([])])), EscapedString("grid")]), Row([EscapedString("scalar"), EscapedString("simple string")])]))"#);            
    }

    #[test]
    fn it_works() {
        use super::*;

        assert_eq!(
            inf("INF"),
            Ok(("", Token::INF))
        );

        assert_eq!(
            inf("-INF"),
            Ok(("", Token::INF_NEG))
        );

        assert_eq!(
            nan("NAN"),
            Ok(("", Token::NAN))
        );

        assert_eq!(
            nan("-NAN"),
            Err(nom::Err::Error(("-NAN", ErrorKind::Tag)))
        );

        assert_eq!(
            simple_number("32143"),
            Ok(("", Token::Number(32143f64, "".into())))
        );
        assert_eq!(
            simple_number("2"),
            Ok(("", Token::Number(2.0f64, "".into())))
        );
        assert_eq!(
            simple_number("32143.25"),
            Ok(("", Token::Number(32143.25f64, "".into())))
        );
        assert_eq!(
            simple_number("-0.125"),
            Ok(("", Token::Number(-0.125f64, "".into())))
        );
        assert_eq!(
            simple_number("+674.96"),
            Ok(("", Token::Number(674.96f64, "".into())))
        );

        assert_eq!(
            number("1"),
            Ok(("", 1f64))
        );

        assert_eq!(
            number("-56"),
            Ok(("", -56f64))
        );

        assert_eq!(
            number("-34"),
            Ok(("", -34f64))
        );

        assert_eq!(
            number("5.4"),
            Ok(("", 5.4f64))
        );

        assert_eq!(
            number("-5.4"),
            Ok(("", -5.4f64))
        );

        assert_eq!(
            number("9.23"),
            Ok(("", 9.23f64))
        );

        assert_eq!(
            number("5.4e-45"),
            Ok(("", 5.4e-45f64))
        );

        assert_eq!(
            number("-5.4e-45"),
            Ok(("", -5.4e-45f64))
        );

        assert_eq!(
            number("67.3E7"),
            Ok(("", 67.3E7f64))
        );

        assert_eq!(
            zinc_number("1"),
            Ok(("", Token::Number(1f64, "".into())))
        );

        assert_eq!(
            zinc_number("5.4"),
            Ok(("", Token::Number(5.4f64, "".into())))
        );

        assert_eq!(
            zinc_number("-5.4"),
            Ok(("", Token::Number(-5.4f64, "".into())))
        );

        assert_eq!(
            zinc_number("-5.4e-45"),
            Ok(("", Token::Number(-5.4e-45f64, "".into())))
        );

        assert_eq!(
            zinc_number("67.3E7"),
            Ok(("", Token::Number(67.3E7f64, "".into())))
        );

        assert_eq!(
            zinc_number("INF"),
            Ok(("", Token::INF))
        );

        assert_eq!(
            zinc_number("-INF"),
            Ok(("", Token::INF_NEG))
        );

        assert_eq!(
            zinc_number("NAN"),
            Ok(("", Token::NAN))
        );

        assert_eq!(
            zinc_number("-NAN"),
            Err(nom::Err::Error(("-NAN", ErrorKind::Tag)))
        );

        assert_eq!(
            zinc_number("-5.4e-45Kg"),
            Ok(("", Token::Number(-5.4e-45f64, "Kg".into())))
        );

        assert_eq!(
            comma(","),
            Ok(("", Token::Comma))
        );

        assert_eq!(
            comma(","),
            Ok(("", Token::Comma))
        );

        assert_eq!(
            null("N"),
            Ok(("", Token::Null))
        );

        assert_ne!(
            null("n"),
            Ok(("", Token::Null))
        );

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

        let d = Dict::new(&vec![Tag::new(Token::EscapedString("haystackVersion".into()), Some(Token::EscapedString("3.0".into()))),
                                Tag::new(Token::EscapedString("serverTime".into()), Some(Token::DateTime(now))),
                                Tag::new(Token::EscapedString("tz".into()), Some(Token::EscapedString("UTC".into()))),
                               ]);

        println!("{}", d.to_zinc());
    }

    #[test]
    fn about_uri() {
    
        let grid_metadata = GridMeta::new(Token::Ver("3.0".into()), None);

        println!("{:?}", grid_meta(&grid_metadata.to_zinc()));

        let now: DateTime<FixedOffset> = DateTime::<FixedOffset>::from(Utc::now());

        // Response: single row grid with following columns:
        let cols_obj = Cols::new(vec![Col::new(Token::Id("serverTime".into()), None),
                                  Col::new(Token::Id("tz".into()), None),
                                 ]);

        println!("{:?}", cols(&cols_obj.to_zinc()));

        let row = Row::new(vec![Val::new(Box::new(Token::DateTime(now))),
                                Val::new(Box::new(Token::EscapedString("UTC".into())))]);

        let grid_obj = Grid::new(grid_metadata, cols_obj, Rows::new(vec![row]));

        println!("{:?}", grid_obj.to_zinc());
        println!("{}", grid_obj.to_zinc());

        let s = grid_obj.to_zinc();


        println!("{:?}", grid(&s));

        assert!(grid(&s).is_ok()); 

    }
}