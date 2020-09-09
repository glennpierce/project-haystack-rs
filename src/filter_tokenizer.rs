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


fn filter_bool<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    map(alt((tag("true"), tag("false"))), |o: &str| {
        if o == "false" {
            FilterToken::Bool(false)
        } else {
            FilterToken::Bool(true)
        }
    })(i)
}


// equipRef->siteRef->dis
// equipRef has siteRef which has a dis tag
// <path>       :=  <name> ("->" <name>)*
fn name<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    map( 
        zinc_id, 
            |t: Token| {
    
                match t {
                    
                    Token::Id(val) => FilterToken::Name(val),
                    _ => unreachable!(),
                }
            }
    )(i)
}

fn not<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    map(tag("not"), |_: &str| FilterToken::Unary(Operation::Not))(i)
}

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

    Bool(bool),
    Name(String),
    Val(Token),
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
        //bool,
        //zinc_id,      // This is what I added. Not in spec as <name> seems to be undefined  Going to use FilterToken::Id to represent this
    )), |t: Token| {
        
        match &t {
            
            //Token::Bool(b) => FilterToken::Bool(*b),
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


#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Operation {
    Or,
    And,
    Not,
    Has,
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
fn binop<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    // one_of matches one of the characters we give it
    let (i, t) = alt((tag("or"), tag("and"), tag("->"), tag("=="), tag("!="), tag("<"), tag("<="), tag(">"), tag(">=")))(i)?;
  
    Ok((
      i,
      match t {
        "or" => FilterToken::Binary(Operation::Or),
        "and" => FilterToken::Binary(Operation::And),
        "->" => FilterToken::Binary(Operation::Has),
        "==" => FilterToken::Binary(Operation::Equals),
        "!=" => FilterToken::Binary(Operation::NotEquals),
        "<" => FilterToken::Binary(Operation::LessThan),
        "<=" => FilterToken::Binary(Operation::LessThanEquals),
        ">" => FilterToken::Binary(Operation::MoreThan),
        ">=" => FilterToken::Binary(Operation::MoreThanEquals),
        _ => unreachable!(),
      },
    ))
  }
  
fn lparen<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    map(tag("("), |_: &str| FilterToken::LParen)(i)
}

fn rparen<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {
    map(tag(")"), |_: &str| FilterToken::RParen)(i)
}

fn lexpr<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    delimited(
          multispace0,
          alt((name, filter_bool, filter_val, lparen)),
          multispace0
    )(i)
}

fn after_rexpr<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    delimited(
          multispace0,
          alt((not, binop, rparen)),
          multispace0
    )(i)
}

fn after_rexpr_no_paren<'a>(i: &'a str) -> IResult<&'a str, FilterToken, (&'a str, ErrorKind)> {

    delimited(multispace0, alt((not, binop)), multispace0)(i)
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

        let r = match (state, paren_stack.last()) {
            (TokenizerState::AfterRExpr, None) => after_rexpr_no_paren(s),
            (TokenizerState::AfterRExpr, Some(&ParenState::Subexpr)) => after_rexpr(s),
            (TokenizerState::LExpr, _) => lexpr(s),
        };

        println!("r: {:?}", r);

        match r {
            Ok((rest, t)) => {

                match t {
                    FilterToken::LParen => {
                        paren_stack.push(ParenState::Subexpr);
                    }
                    FilterToken::RParen => {
                        paren_stack.pop().expect("The paren_stack is empty!");
                    }
                    FilterToken::Val(_) | FilterToken::Name(_) | FilterToken::Bool(_) => {
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

                return Err(FilterTokenParseError::UnknownError);
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


// fn spacey<F, I, O, E>(f: F) -> impl Fn(I) -> IResult<I, O, E>
// where
//     F: Fn(I) -> IResult<I, O, E>,
//     I: nom::InputTakeAtPosition,
//     <I as nom::InputTakeAtPosition>::Item: nom::AsChar + Clone,
//     E: nom::error::ParseError<I>,
// {
//     delimited(space0, f, space0)
// }

// fn multispacey<F, I, O, E>(f: F) -> impl Fn(I) -> IResult<I, O, E>
// where
//     F: Fn(I) -> IResult<I, O, E>,
//     I: nom::InputTakeAtPosition,
//     <I as nom::InputTakeAtPosition>::Item: nom::AsChar + Clone,
//     E: nom::error::ParseError<I>,
// {
//     delimited(multispace0, f, multispace0)
// }

#[cfg(test)]
mod tests {
    use super::*;
 
    #[test]
    fn basic_tests() {

        assert_eq!(
            binop("or"),
            Ok(("", FilterToken::Binary(Operation::Or)))
        );
        assert_eq!(
            name("abc32"),
            Ok(("", FilterToken::Name("abc32".to_string())))
        );
        assert_eq!(
            lparen("("),
            Ok(("", FilterToken::LParen))
        );
        assert_eq!(
            rparen(")"),
            Ok(("", FilterToken::RParen))
        );
    }

    #[test]
    fn test_lexpr() {

        assert_eq!(
            lexpr("and elec and heat "),
            Ok(("and elec and heat ", FilterToken::Binary(Operation::And)))
        );
        

        // println!("{:?}", number("+(3--2) "));

        // assert_eq!(
        //     lexpr("+(3--2) "),
        //     Ok(("+(3--2) ", FilterToken::Binary(Operation::Plus)))
        // );

    }

    // #[test]
    // fn test_var() {
    //     for &s in ["abc", "U0", "_034", "a_be45EA", "aAzZ_"].iter() {
    //         assert_eq!(
    //             var(s),
    //             Ok(("", FilterToken::Var(s.into())))
    //         );
    //     }

    //     assert_eq!(var(""), Err(Err::Error(("", nom::error::ErrorKind::OneOf))));
    //     assert_eq!(var("0"), Err(Err::Error(("0", nom::error::ErrorKind::OneOf))));
    // }

    #[test]
    fn test_tokenize() {
        use super::Operation::*;
        use super::FilterToken::*;

        assert_eq!(tokenize("elec and heat"), Ok(vec![
            FilterToken::Name("elec".to_string()),
            Binary(And),
            FilterToken::Name("heat".to_string()),
        ]));

        assert_eq!(tokenize("elecandheat"), Ok(vec![
            FilterToken::Name("elecandheat".to_string()),
        ]));

        assert_eq!(tokenize("elec or heat"), Ok(vec![
            FilterToken::Name("elec".to_string()),
            Binary(Or),
            FilterToken::Name("heat".to_string()),
        ]));

        assert_eq!(tokenize("elec->heat"), Ok(vec![
            FilterToken::Name("elec".to_string()),
            Binary(Has),
            FilterToken::Name("heat".to_string()),
        ]));

        assert_eq!(tokenize("elec -> heat"), Ok(vec![
            FilterToken::Name("elec".to_string()),
            Binary(Has),
            FilterToken::Name("heat".to_string()),
        ]));

        assert_eq!(tokenize("equip and siteRef->geoCity"), Ok(vec![
            FilterToken::Name("equip".to_string()),
            Binary(And),
            FilterToken::Name("siteRef".to_string()),
            Binary(Has),
            FilterToken::Name("geoCity".to_string()),
        ]));

        assert_eq!(tokenize("geoCity == \"Chicago\""), Ok(vec![
            FilterToken::Name("geoCity".to_string()),
            Binary(Equals),
            FilterToken::Val(Token::EscapedString("Chicago".to_string())),
        ]));

        assert_eq!(tokenize("elec and (heat or water)"), Ok(vec![
            FilterToken::Name("elec".to_string()),
            Binary(And),
            LParen,
            FilterToken::Name("heat".to_string()),
            Binary(Or),
            FilterToken::Name("water".to_string()),
            RParen,
        ]));

        assert_eq!(tokenize("equip and siteRef->geoCity->dis == \"Chicago\""), Ok(vec![
            FilterToken::Name("equip".to_string()),
            Binary(And),
            FilterToken::Name("siteRef".to_string()),
            Binary(Has),
            FilterToken::Name("geoCity".to_string()),
            Binary(Has),
            FilterToken::Name("dis".to_string()),
            Binary(Equals),
            FilterToken::Val(Token::EscapedString("Chicago".to_string())),
        ]));

        assert_eq!(tokenize("equip and \"Chicago\" == siteRef->geoCity->dis"), Ok(vec![
            FilterToken::Name("equip".to_string()),
            Binary(And),
            FilterToken::Val(Token::EscapedString("Chicago".to_string())),
            Binary(Equals),
            FilterToken::Name("siteRef".to_string()),
            Binary(Has),
            FilterToken::Name("geoCity".to_string()),
            Binary(Has),
            FilterToken::Name("dis".to_string()),
        ]));

        // assert_eq!(tokenize("a"), Ok(vec![Var("a".into())]));

        // assert_eq!(
        //     tokenize("2 +(3--2) "),
        //     Ok(vec![
        //         Number(2f64),
        //         Binary(Plus),
        //         LParen,
        //         Number(3f64),
        //         Binary(Minus),
        //         Unary(Minus),
        //         Number(2f64),
        //         RParen
        //     ])
        // );

        // assert_eq!(
        //     tokenize("-2^ ab0 *12 - C_0"),
        //     Ok(vec![
        //         Unary(Minus),
        //         Number(2f64),
        //         Binary(Pow),
        //         Var("ab0".into()),
        //         Binary(Times),
        //         Number(12f64),
        //         Binary(Minus),
        //         Var("C_0".into()),
        //     ])
        // );

        // assert_eq!(
        //     tokenize("-sin(pi * 3)^ cos(2) / Func2(x, f(y), z) * _buildIN(y)"),
        //     Ok(vec![
        //         Unary(Minus),
        //         Func("sin".into(), None),
        //         Var("pi".into()),
        //         Binary(Times),
        //         Number(3f64),
        //         RParen,
        //         Binary(Pow),
        //         Func("cos".into(), None),
        //         Number(2f64),
        //         RParen,
        //         Binary(Div),
        //         Func("Func2".into(), None),
        //         Var("x".into()),
        //         Comma,
        //         Func("f".into(), None),
        //         Var("y".into()),
        //         RParen,
        //         Comma,
        //         Var("z".into()),
        //         RParen,
        //         Binary(Times),
        //         Func("_buildIN".into(), None),
        //         Var("y".into()),
        //         RParen,
        //     ])
        // );

        // assert_eq!(
        //     tokenize("2 % 3"),
        //     Ok(vec![Number(2f64), Binary(Rem), Number(3f64)])
        // );

        // assert_eq!(
        //     tokenize("1 + 3! + 1"),
        //     Ok(vec![
        //         Number(1f64),
        //         Binary(Plus),
        //         Number(3f64),
        //         Unary(Fact),
        //         Binary(Plus),
        //         Number(1f64)
        //     ])
        // );

        // assert_eq!(tokenize("()"), Err(FilterTokenParseError::UnexpectedStrToken(")".to_string())));

        // assert_eq!(tokenize(""), Err(FilterTokenParseError::MissingArgument));
        // assert_eq!(tokenize("2)"), Err(FilterTokenParseError::UnexpectedStrToken(")".to_string())));
        // assert_eq!(tokenize("2^"), Err(FilterTokenParseError::MissingArgument));
        // assert_eq!(tokenize("(((2)"), Err(FilterTokenParseError::MissingRParen(2)));
        // assert_eq!(tokenize("f(2,)"), Err(FilterTokenParseError::UnexpectedStrToken(")".to_string())));
        // assert_eq!(tokenize("f(,2)"), Err(FilterTokenParseError::UnexpectedStrToken(",2)".to_string())));
    }
}