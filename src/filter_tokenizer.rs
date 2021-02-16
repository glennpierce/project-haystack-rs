//! Tokenizer that converts a zinc string form into a series of `Token`s.
use nom::{
    branch::alt,
    bytes::complete::{is_a, tag},
    character::complete::{alphanumeric1, char, digit1, multispace0, multispace1, newline, one_of, space0},
    combinator::{complete, map, opt, peek, recognize},
    error::ErrorKind,
    multi::{many1, separated_list},
    sequence::{delimited, preceded, separated_pair, terminated, tuple}, IResult,
};

use chrono::{Date, DateTime, Datelike, FixedOffset, NaiveDateTime, TimeZone, Utc};

use crate::hval::HVal;
use crate::token::*;
use crate::error::FilterTokenParseError;
use crate::zinc_tokenizer::{number_with_unit, zinc_ref, quoted_string, time_with_subseconds, uri, date, zinc_id};

fn filter_bool<'a>(i: &'a str) -> IResult<&'a str, Token, (&'a str, ErrorKind)> {
    map(alt((tag("true"), tag("false"))), |o: &str| {
        if o == "false" {
            Token::Bool(false)
        } else {
            Token::Bool(true)
        }
    })(i)
}

// <val>        :=  <bool> | <ref> | <str> | <uri> |
//                  <number> | <date> | <time>
// <bool>       := "true" or "false"
// <number>     := same as Zinc (keywords not supported INF, -INF, NaN)             
// <ref>        := same as Zinc                                                     
// <str>        := same as Zinc                                                       
// <uri>        := same as Zinc                                    
// <date>       := same as Zinc                                 
// <time>       := same as Zinc                   
fn filter_val<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    map(alt((
        zinc_ref,
        quoted_string,
        uri,
        date,
        time_with_subseconds,
        number_with_unit,
        filter_bool,
        //bool,
        //zinc_id,      // This is what I added. Not in spec as <name> seems to be undefined  Going to use FilterToken::Id to represent this
    )), |t: Token| {
        
        match &t {
            
            Token::Bool(b) => FilterToken::Val(Token::Bool(*b)),
            Token::Number(num, units) => FilterToken::Val(t),
            //Token::Id(val) => FilterToken::Name(val),
            Token::Ref(val, display) => FilterToken::Val(t),
            Token::EscapedString(val) => FilterToken::Val(t),
            Token::Date(val) => FilterToken::Val(t),
            Token::Uri(val) => FilterToken::Val(t),
            Token::Time(val) => FilterToken::Val(t),
            _ => unreachable!(),
        }

    })(i)
}

// equipRef->siteRef->dis
// equipRef has siteRef which has a dis tag
// <path>       :=  <name> ("->" <name>)*
// fn name<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
//     map( 
//         zinc_id, 
//             |t: Token| {
    
//                 match t {
                    
//                     Token::Id(val) => FilterToken::Path(vec![Box::new(t)]),
//                     _ => unreachable!(),
//                 }
//             }
//     )(i)
// }

fn name_path<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    map( 
        zinc_id, |t: Token| { FilterToken::Path(vec![t]) }
    )(i)
}

fn name_path2<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    map( 
        zinc_id, |t: Token| { FilterToken::Path(vec![t]) }
    )(i)
}

fn name_path_list<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    map( 
        separated_list(tag("->"), zinc_id), 
            |v: Vec<Token>| 
                { 
                    let tmp: Vec<Token> = v.iter().map(|i| i.clone()).collect();
                    FilterToken::Path(tmp)
                }
    )(i)
}

fn name_path_list2<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    map( 
        separated_list(tag("->"), zinc_id), 
            |v: Vec<Token>| 
                { 
                    let tmp: Vec<Token> = v.iter().map(|i| i.clone()).collect();
                    FilterToken::Path(tmp)
                }
    )(i)
}

fn path<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    alt(( name_path_list, name_path ) ) (i)
}

fn path2<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    alt(( name_path_list2, name_path2 ) ) (i)
}

// fn cmp_op<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
//     // one_of matches one of the characters we give it
//     let (i, t) = alt((tag("=="), tag("!="), tag("<"), tag("<="), tag(">"), tag(">=")))(i)?;
  
//     Ok((
//       i,
//       match t {
//         "==" => FilterToken::Binary(Operation::Equals),
//         "!=" => FilterToken::Binary(Operation::NotEquals),
//         "<" => FilterToken::Binary(Operation::LessThan),
//         "<=" => FilterToken::Binary(Operation::LessThanEquals),
//         ">" => FilterToken::Binary(Operation::MoreThan),
//         ">=" => FilterToken::Binary(Operation::MoreThanEquals),
//         _ => unreachable!(),
//       },
//     ))
// }

fn cmp_op<'a>(i: &'a str) -> IResult<&'a str, Operation, (&'a str, ErrorKind)> {
    // one_of matches one of the characters we give it
    let (i, t) = alt((tag("=="), tag("!="), tag("<"), tag("<="), tag(">"), tag(">=")))(i)?;
  
    Ok((
      i,
      match t {
        "==" => Operation::Equals,
        "!=" => Operation::NotEquals,
        "<" => Operation::LessThan,
        "<=" => Operation::LessThanEquals,
        ">" => Operation::MoreThan,
        ">=" => Operation::MoreThanEquals,
        _ => unreachable!(),
      },
    ))
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

// fn cmp<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

//     map(tuple((multispacey(path), cmp_op, multispacey(filter_val))), 
//         |t| {
//             FilterToken::Compare(Box::new(t.0), Box::new(t.1), Box::new(t.2))
//         }
//     )(i)
// }

fn cmp<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    map(tuple((multispacey(path), cmp_op, multispacey(filter_val))), 
        |t| {
            FilterToken::Compare(Box::new(t.0), t.1, Box::new(t.2))
        }
    )(i)
}

fn cmp2<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    map(multispacey(cmp_op), 
        |t| {
            FilterToken::Binary(t)
        }
    )(i)
}

fn and<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    map(tag("and"), |o: &str| { FilterToken::Binary(Operation::And)})(i)
}

fn or<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    map(tag("or"), |o: &str| { FilterToken::Binary(Operation::Or)})(i)
}

fn and_or<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    
    alt((and, or))(i)
}

fn and_or_cmp<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    
    alt((cmp2, and, or))(i)
}

fn not<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    map(tag("not"), |o: &str| { FilterToken::Unary(Operation::Not)})(i)
}

fn term<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    alt((cmp, not, path))(i)
}

fn term2<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    alt((not, path2))(i)
}

// fn filter<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    
//     separated_pair(term, and_or, term),

//     alt((path, not, cmp))(i)
// }

/// Expression tokens.
#[derive(Debug, PartialEq, Clone)]
pub enum FilterToken {
    /// Binary operation.
    Binary(Operation),
    /// Unary operation.
    Unary(Operation),

    /// Left parenthesis.
    LParen,
    /// Right parenthesis.
    RParen,

    Compare(Box<FilterToken>, Operation, Box<FilterToken>),

    //Bool(bool),
    //Name(String),
    Path(Vec<Token>),   // Vector of id types
    Val(Token),
}




#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Operation {
    Or,
    And,
    Not,
    // Has,
    Equals,
    NotEquals,
    LessThan,
    LessThanEquals,
    MoreThan,
    MoreThanEquals,
}



/// Continuing the trend of starting from the simplest piece and building up,
/// we start by creating a parser for the built-in operator functions.
/// "=" | "!=" | "<" | "<=" | ">" | ">="
// fn binop<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
//     // one_of matches one of the characters we give it
//     let (i, t) = alt((tag("or"), tag("and"), tag("->"), tag("=="), tag("!="), tag("<"), tag("<="), tag(">"), tag(">=")))(i)?;
  
//     Ok((
//       i,
//       match t {
//         "or" => FilterToken::Binary(Operation::Or),
//         "and" => FilterToken::Binary(Operation::And),
//         "->" => FilterToken::Binary(Operation::Has),
//         "==" => FilterToken::Binary(Operation::Equals),
//         "!=" => FilterToken::Binary(Operation::NotEquals),
//         "<" => FilterToken::Binary(Operation::LessThan),
//         "<=" => FilterToken::Binary(Operation::LessThanEquals),
//         ">" => FilterToken::Binary(Operation::MoreThan),
//         ">=" => FilterToken::Binary(Operation::MoreThanEquals),
//         _ => unreachable!(),
//       },
//     ))
//   }
  
fn lparen<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    map(tag("("), |_: &str| FilterToken::LParen)(i)
}

fn rparen<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    map(tag(")"), |_: &str| FilterToken::RParen)(i)
}

fn lexpr<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    delimited(
          multispace0,
          alt((lparen, term)),
          multispace0
    )(i)
}

fn lexpr2<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    delimited(
          multispace0,
          alt((lparen, filter_val, term2)),
          multispace0
    )(i)
}

fn after_rexpr<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    delimited(
          multispace0,
          alt((and_or, filter_val, rparen)),
          multispace0
    )(i)
}

fn after_rexpr2<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    delimited(
          multispace0,
          alt((cmp2, and_or, filter_val, rparen)),
          multispace0
    )(i)
}

fn after_rexpr_no_paren<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    delimited(multispace0, and_or, multispace0)(i)
}

fn after_rexpr_no_paren2<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    delimited(multispace0, alt((cmp2, and, or, filter_val)), multispace0)(i)
}

#[derive(Debug, Clone, Copy)]
enum TokenizerState {
    // accept any token that is an expression from the left: var, num, (, negpos
    LExpr,
    // accept any token that needs an expression on the left: fact, binop, ), comma
    AfterRExpr,
}

#[derive(Debug, Clone, Copy)]
enum ParenState {
    Subexpr,
}

/// Tokenize a given mathematical expression.
///
/// The parser should return `Ok` only if the expression is well-formed.
///
/// # Failure
///
/// Returns `Err` if the expression is not well-formed.
pub fn tokenize(input: &str) -> Result<Vec<FilterToken>, FilterTokenParseError> {
    let mut state: TokenizerState = TokenizerState::LExpr;
    // number of function arguments left
    let mut paren_stack = vec![];

    let mut res = vec![];

    let mut s = input;

    while !s.is_empty() {

        // println!("s: {:?},   state: {:?}", s, state);

        let r = match (state, paren_stack.last()) {
            (TokenizerState::AfterRExpr, None) => after_rexpr_no_paren(s),
            (TokenizerState::AfterRExpr, Some(&ParenState::Subexpr)) => after_rexpr(s),
            (TokenizerState::LExpr, _) => lexpr(s),
        };

        // println!("r: {:?}", r);

        match r {
            Ok((rest, t)) => {

                match t {
                    FilterToken::LParen => {
                        paren_stack.push(ParenState::Subexpr);
                    }
                    FilterToken::RParen => {
                        paren_stack.pop().expect("The paren_stack is empty!");
                    }
                    FilterToken::Val(_) | FilterToken::Path(_) | FilterToken::Compare(_, _, _) => {
                        state = TokenizerState::AfterRExpr;
                    }
                    FilterToken::Binary(_) => {
                        state = TokenizerState::LExpr;
                    }
                    _ => {}
                }
                res.push(t);
                s = rest;
            }
            Err(e) => {
        
                //match e {
                //    Err::Error((value, _)) => {
               //         return Err(FilterTokenParseError::UnexpectedStrToken(value.to_string()));
                //    },
                //    _ => (),
              //  }

                println!("Tokenize {:?}", e);

                return Err(FilterTokenParseError::UnknownFilterTokenParseError);
            }
            // Error(Err::Position(_, p)) => {
            //     let (i, _) = slice_to_offsets(input, p);
            //     return Err(FilterTokenParseError::UnexpectedToken(i));
            // }
            // _ => {
            //     panic!("Unexpected parse result when parsing `{}` at `{}`: {:?}", input, s, r);
            // }
        }

    }

    match state {
        TokenizerState::LExpr => {
            Err(FilterTokenParseError::MissingArgument)
        },

        _ => {
            if !paren_stack.is_empty() {
                return Err(FilterTokenParseError::MissingRParen(paren_stack.len() as i32));
            }

            return Ok(res);
        }
    }


}

// New way to tokensise. Stop tokenising binary ops as one unit and returning FilterToken::Compare
// Its not flexible.
pub fn tokenize2(input: &str) -> Result<Vec<FilterToken>, FilterTokenParseError> {
    let mut state: TokenizerState = TokenizerState::LExpr;
    // number of function arguments left
    let mut paren_stack = vec![];

    let mut res = vec![];

    let mut s = input;

    while !s.is_empty() {

        // println!("s: {:?},  state: {:?}  paren_stack: {:?}", s, state, paren_stack);

        let r = match (state, paren_stack.last()) {
            (TokenizerState::AfterRExpr, None) => after_rexpr_no_paren2(s),
            (TokenizerState::AfterRExpr, Some(&ParenState::Subexpr)) => after_rexpr2(s),
            (TokenizerState::LExpr, _) => lexpr2(s),
        };

        // println!("r: {:?}", r);

        match r {
            Ok((rest, t)) => {

                match t {
                    FilterToken::LParen => {
                        paren_stack.push(ParenState::Subexpr);
                    }
                    FilterToken::RParen => {
                        paren_stack.pop().expect("The paren_stack is empty!");
                    }
                    FilterToken::Val(_) | FilterToken::Path(_) => {
                        state = TokenizerState::AfterRExpr;
                    }
                    FilterToken::Binary(_) => {
                        state = TokenizerState::LExpr;
                    }
                    _ => {}
                }
                res.push(t);
                s = rest;
            }
            Err(e) => {
        
                //match e {
                //    Err::Error((value, _)) => {
               //         return Err(FilterTokenParseError::UnexpectedStrToken(value.to_string()));
                //    },
                //    _ => (),
              //  }

                println!("Tokenize {:?}", e);

                return Err(FilterTokenParseError::UnknownFilterTokenParseError);
            }
            // Error(Err::Position(_, p)) => {
            //     let (i, _) = slice_to_offsets(input, p);
            //     return Err(FilterTokenParseError::UnexpectedToken(i));
            // }
            // _ => {
            //     panic!("Unexpected parse result when parsing `{}` at `{}`: {:?}", input, s, r);
            // }
        }

    }

    match state {
        TokenizerState::LExpr => {
            Err(FilterTokenParseError::MissingArgument)
        },

        _ => {
            if !paren_stack.is_empty() {
                return Err(FilterTokenParseError::MissingRParen(paren_stack.len() as i32));
            }

            return Ok(res);
        }
    }


}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! id_to_token {
        ($a:expr) => {
            Token::Id($a.to_string())
        };
    }

    macro_rules! id_to_path {
        ($a:expr) => {
            FilterToken::Path(vec![Token::Id($a.to_string())])
        };
    }

    #[test]
    fn basic_tests() {

        use super::FilterToken::*;

        assert_eq!(
            and_or("or"),
            Ok(("", FilterToken::Binary(Operation::Or)))
        );
        assert_eq!(
            path("abc32"),
            Ok(("", FilterToken::Path(vec![Token::Id("abc32".to_string())])))
        );
        assert_eq!(
            lparen("("),
            Ok(("", FilterToken::LParen))
        );
        assert_eq!(
            rparen(")"),
            Ok(("", FilterToken::RParen))
        );
        assert_eq!(
            path("siteRef->cityName->houseName"),
            Ok(("", FilterToken::Path(vec![id_to_token!("siteRef"), id_to_token!("cityName"), id_to_token!("houseName")])))
        );
        assert_eq!(
            path("siteRef"),
            Ok(("", FilterToken::Path(vec![id_to_token!("siteRef")])))
        );
    }

    #[test]
    fn test_lexpr() {

        // and can't be at from so should be interpreted as name
        assert_eq!(
            lexpr("and elec and heat "),
            Ok(("elec and heat ", FilterToken::Path(vec![id_to_token!("and")])))
        );

        println!("{:?}", lexpr("(heat or water)"));

    }

    #[test]
    fn test_tokenize() {
        use super::Operation::*;
        use super::FilterToken::*;

        assert_eq!(tokenize("not elec and water"), Ok(vec![
            Unary(Not),
            id_to_path!("elec"),
            Binary(And),
            id_to_path!("water")
        ]));

        assert_eq!(tokenize("not elec"), Ok(vec![
            Unary(Not),
            id_to_path!("elec"),
        ]));

        assert_eq!(tokenize("elec and heat"), Ok(vec![
            id_to_path!("elec"),
            Binary(And),
            id_to_path!("heat"),
        ]));

        assert_eq!(tokenize("elecandheat"), Ok(vec![
            id_to_path!("elecandheat"),
        ]));

        assert_eq!(tokenize("elec or heat"), Ok(vec![
            id_to_path!("elec"),
            Binary(Or),
            id_to_path!("heat"),
        ]));

        assert_eq!(tokenize("elec->heat"), Ok(vec![
            FilterToken::Path(vec![id_to_token!("elec"), id_to_token!("heat")])
        ]));

        assert_eq!(tokenize("elec and (heat or water)"), Ok(vec![

            id_to_path!("elec"),
            Binary(And),
            LParen,
            id_to_path!("heat"),
            Binary(Or), 
            id_to_path!("water"),
            RParen
       ]));

        assert_eq!(tokenize("equip and siteRef->geoCity"), Ok(vec![

            id_to_path!("equip"),
            Binary(And),
            FilterToken::Path(vec![id_to_token!("siteRef"), id_to_token!("geoCity")]),
        ]));

        assert_eq!(tokenize("geoCity == \"Chicago\""), Ok(
            vec![
                Compare(Box::new(id_to_path!("geoCity")),
                        Operation::Equals,
                        Box::new( FilterToken::Val( Token::EscapedString("Chicago".to_string()) ) )
                    )
            ]
        ));

        assert_eq!(tokenize("equip and siteRef->geoCity->dis == \"Chicago\""), Ok(
            vec![
                id_to_path!("equip"),
                Binary(And),

                Compare(
                        Box::new(FilterToken::Path(vec![id_to_token!("siteRef"), id_to_token!("geoCity"), id_to_token!("dis")])),
                        Operation::Equals,
                        Box::new( FilterToken::Val( Token::EscapedString("Chicago".to_string()) ) )
                    )
            ]
        ));

        assert_eq!(tokenize("equip and siteRef->geoCity->carnego_number_of_bedrooms > 5"), Ok(
            vec![
                id_to_path!("equip"),
                Binary(And),

                Compare(
                        Box::new(FilterToken::Path(vec![id_to_token!("siteRef"), id_to_token!("geoCity"), id_to_token!("carnego_number_of_bedrooms")])),
                        Operation::MoreThan,
                        Box::new( FilterToken::Val( Token::Number(ZincNumber::new(5.0), "".to_string()) ) )
                    )
            ]
        ));

        // println!("{:?}", tokenize("equip and \"Chicago\" == siteRef->geoCity->dis"));

        // assert_eq!(tokenize("equip and \"Chicago\" == siteRef->geoCity->dis"), Ok(vec![
        //     FilterToken::Name("equip".to_string()),
        //     Binary(And),
        //     FilterToken::Val(Token::EscapedString("Chicago".to_string())),
        //     Binary(Equals),
        //     FilterToken::Name("siteRef".to_string()),
        //     Binary(Has),
        //     FilterToken::Name("geoCity".to_string()),
        //     Binary(Has),
        //     FilterToken::Name("dis".to_string()),
        // ]));

    }

    #[test]
    fn test_tokenize2() {
        use super::Operation::*;
        use super::FilterToken::*;
        use super::Token;

        assert_eq!(tokenize2("siteRef->geoCity->dis == \"Chicago\""), Ok(
            vec![
                Path([id_to_token!("siteRef"), id_to_token!("geoCity"), id_to_token!("dis")].to_vec()),
                Binary(Equals),
                Val(Token::EscapedString("Chicago".to_string()))
            ]
        ));

        assert_eq!(tokenize2("equip and siteRef->geoCity->dis == \"Chicago\""), Ok(
            vec![
                id_to_path!("equip"),
                Binary(And),
                Path([id_to_token!("siteRef"), id_to_token!("geoCity"), id_to_token!("dis")].to_vec()),
                Binary(Equals),
                Val(Token::EscapedString("Chicago".to_string()))
            ]
        ));

        assert_eq!(tokenize2("equip or siteRef->dis == \"Chicago\""), Ok(
            vec![
                id_to_path!("equip"),
                Binary(Or),
                Path([id_to_token!("siteRef"), id_to_token!("dis")].to_vec()),
                Binary(Equals),
                Val(Token::EscapedString("Chicago".to_string()))
            ]
        ));


        assert_eq!(tokenize2("siteRef->dis != \"Chicago\" or heat"), Ok(
            vec![
                Path([id_to_token!("siteRef"), id_to_token!("dis")].to_vec()),
                Binary(NotEquals),
                Val(Token::EscapedString("Chicago".to_string())),
                Binary(Or),
                id_to_path!("heat"),
            ]
        ));

        assert_eq!(tokenize2("\"Chicago\" == siteRef->dis or heat"), Ok(
            vec![
                Val(Token::EscapedString("Chicago".to_string())),
                Binary(Equals),
                Path([id_to_token!("siteRef"), id_to_token!("dis")].to_vec()),
                Binary(Or),
                id_to_path!("heat"),
            ]
        ));

        println!("{:?}", tokenize2("equip or siteRef->dis == \"Chicago\""));
    }

    #[test]
    fn basic_tests2() {

        use super::FilterToken::*;

        assert_eq!(
            path2("siteRef->cityName->houseName"),
            Ok(("", FilterToken::Path(vec![id_to_token!("siteRef"), id_to_token!("cityName"), id_to_token!("houseName")])))
        );
        assert_eq!(
            path("siteRef"),
            Ok(("", FilterToken::Path(vec![id_to_token!("siteRef")])))
        );
        assert_eq!(
            path("siteRef->geoCity->dis"),
            Ok(("", FilterToken::Path(vec![id_to_token!("siteRef"), id_to_token!("geoCity"), id_to_token!("dis")])))
        );
    }

    #[test]
    fn test_lexpr2() {

        // and can't be at from so should be interpreted as name
        assert_eq!(
            lexpr("equip and siteRef->geoCity->dis == \"Chicago\""),
            Ok(("and siteRef->geoCity->dis == \"Chicago\"", FilterToken::Path(vec![id_to_token!("equip")])))
        );

        assert_eq!(
            lexpr2("equip and siteRef->geoCity->dis == \"Chicago\""),
            Ok(("and siteRef->geoCity->dis == \"Chicago\"", FilterToken::Path(vec![id_to_token!("equip")])))
        );

        println!("{:?}", lexpr2("equip and siteRef->geoCity->dis == \"Chicago\""));

    }
}