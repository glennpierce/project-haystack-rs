//! Implementation of the shunting-yard algorithm for converting an infix expression to an
//! expression in reverse Polish notation (RPN).
//!
//! See the Wikipedia articles on the [shunting-yard algorithm][shunting] and on [reverse Polish
//! notation][RPN] for more details.
//!
//! [RPN]: https://en.wikipedia.org/wiki/Reverse_Polish_notation
//! [shunting]: https://en.wikipedia.org/wiki/Shunting-yard_algorithm
use std;
use std::fmt;
use crate::token::Token;
use crate::filter_tokenizer::{FilterToken, Operation, tokenize};

#[derive(Debug, Clone, Copy)]
enum Associativity {
    Left,
    Right,
    NA,
}

/// An error produced by the shunting-yard algorightm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RPNError {
    /// An extra left parenthesis was found.
    MismatchedLParen(usize),
    /// An extra right parenthesis was found.
    MismatchedRParen(usize),
    /// Comma that is not separating function arguments.
    UnexpectedComma(usize),
    /// Too few operands for some operator.
    NotEnoughOperands(usize),
    /// Too many operands reported.
    TooManyOperands,
}

impl fmt::Display for RPNError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            RPNError::MismatchedLParen(i) => {
                write!(f, "Mismatched left parenthesis at token {}.", i)
            }
            RPNError::MismatchedRParen(i) => {
                write!(f, "Mismatched right parenthesis at token {}.", i)
            }
            RPNError::UnexpectedComma(i) => write!(f, "Unexpected comma at token {}", i),
            RPNError::NotEnoughOperands(i) => write!(f, "Missing operands at token {}", i),
            RPNError::TooManyOperands => {
                write!(f, "Too many operands left at the end of expression.")
            }
        }
    }
}

impl std::error::Error for RPNError {
    fn description(&self) -> &str {
        match *self {
            RPNError::MismatchedLParen(_) => "mismatched left parenthesis",
            RPNError::MismatchedRParen(_) => "mismatched right parenthesis",
            RPNError::UnexpectedComma(_) => "unexpected comma",
            RPNError::NotEnoughOperands(_) => "missing operands",
            RPNError::TooManyOperands => "too many operands left at the end of expression",
        }
    }
}

/// Returns the operator precedence and associativity for a given token.
fn prec_assoc(token: &FilterToken) -> (u32, Associativity) {

    use Operation::*;
    use Token::*;
    use FilterToken::*;
    match *token {
        Binary(op) => match op {
            Or => (1, Associativity::Left),
            And => (2, Associativity::Left),
            Equals => (3, Associativity::Left),
            LessThan | MoreThan | LessThanEquals | MoreThanEquals => (4, Associativity::Left),
            Has => (5, Associativity::Left),
            _ => {
                println!("{:?}", op);
                unimplemented!()
            },
        },
        Unary(op) => match op {
            Not => (6, Associativity::NA),
            _ => unimplemented!(),
        },
        _ => (0, Associativity::NA),
    }
}

/// Converts a tokenized infix expression to reverse Polish notation.
///
/// # Failure
///
/// Returns `Err` if the input expression is not well-formed.
/// In RPN, the numbers and operators are listed one after another, and an operator always acts on the most recent numbers in the list.
pub fn to_rpn(input: &[FilterToken]) -> Result<Vec<FilterToken>, RPNError> {

    use FilterToken::*;

    let mut output = Vec::with_capacity(input.len());
    let mut stack = Vec::with_capacity(input.len());

    for (index, token) in input.iter().enumerate() {
        let token = token.clone();
        match token {
            FilterToken::Val(_) => output.push(token),
            FilterToken::Name(_) => output.push(token),
            FilterToken::Bool(_) => output.push(token),
            FilterToken::Unary(_) => stack.push((index, token)),
            FilterToken::Binary(_) => {
                let pa1 = prec_assoc(&token);
                while !stack.is_empty() {
                    let pa2 = prec_assoc(&stack.last().unwrap().1);
                    match (pa1, pa2) {
                        ((i, Associativity::Left), (j, _)) if i <= j => {
                            output.push(stack.pop().unwrap().1);
                        }
                        ((i, Associativity::Right), (j, _)) if i < j => {
                            output.push(stack.pop().unwrap().1);
                        }
                        _ => {
                            break;
                        }
                    }
                }
                stack.push((index, token))
            }
            FilterToken::LParen => stack.push((index, token)),
            FilterToken::RParen => {
                let mut found = false;
                while let Some((_, t)) = stack.pop() {
                    match t {
                        FilterToken::LParen => {
                            found = true;
                            break;
                        },
                        _ => output.push(t),
                    }
                }
                if !found {
                    return Err(RPNError::MismatchedRParen(index));
                }
            },
        }
    }

    while let Some((index, token)) = stack.pop() {
        match token {
            Unary(_) | Binary(_) => output.push(token),
            LParen => return Err(RPNError::MismatchedLParen(index)),
            _ => panic!("Unexpected token on stack."),
        }
    }

    // verify rpn
    let mut n_operands = 0isize;
    for (index, token) in output.iter().enumerate() {
        match *token {
            FilterToken::Val(_) => n_operands += 1,
            FilterToken::Name(_) => n_operands += 1,
            FilterToken::Bool(_) => n_operands += 1,
            FilterToken::Unary(_) => (),
            FilterToken::Binary(_) => n_operands -= 1,
            _ => panic!("Nothing else should be here"),
        }
        if n_operands <= 0 {
            return Err(RPNError::NotEnoughOperands(index));
        }
    }

    if n_operands > 1 {
        return Err(RPNError::TooManyOperands);
    }

    output.shrink_to_fit();
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use FilterToken::*;

    #[test]
    fn test_to_rpn() {

        assert_eq!(to_rpn(&[FilterToken::Name("elec".to_string()), Binary(Operation::And), FilterToken::Name("heat".to_string())]),
            Ok(vec![FilterToken::Name("elec".to_string()), FilterToken::Name("heat".to_string()), Binary(Operation::And)]));

        // equip and siteRef->geoCity == "Chicago"
        // The way to read the above expression is match an entity if:

        // it has equip tag
        // and it has a siteRef tag which is a Ref
        // and what the siteRef tag points to has the geoCity tag
        // and that the site's geoCity tag is equal to "Chicago"

        assert_eq!(to_rpn(&tokenize("equip and siteRef->geoCity->dis == \"Chicago\"").unwrap()),
            Ok(vec![FilterToken::Name("equip".to_string()), FilterToken::Name("siteRef".to_string()),
                    FilterToken::Name("geoCity".to_string()), Binary(Operation::Has),
                    FilterToken::Name("dis".to_string()), Binary(Operation::Has),
                    FilterToken::Val(Token::EscapedString("Chicago".to_string())), Binary(Operation::Equals), Binary(Operation::And)]));

        // In RPN, the numbers and operators are listed one after another, and an operator always acts on the most recent numbers in the list.

        // push equip on stack
        // push siteRef on stack
        // push geoCity on stack     stack = [geoCity, siteRef, equip]
        // Apply binary has to  siteRef has geoCity . Add geoCity if exists     stack = [geoCity, equip]
        // push dis to stack                            stack = [dis, geoCity, equip]
        // Apply binary has to  geoCity has dis. Add dis if exists    stack = [dis, equip]
        // push EscapedString("Chicago") to stack   stack = [EscapedString("Chicago"), dis, equip]
        // Apply equals to dis == EscapedString("Chicago")  

        // assert_eq!(
        //     to_rpn(&[Number(1.), Binary(Plus), Number(2.)]),
        //     Ok(vec![Number(1.), Number(2.), Binary(Plus)])
        // );
        // assert_eq!(
        //     to_rpn(&[Unary(Minus), Number(1.), Binary(Pow), Number(2.)]),
        //     Ok(vec![Number(1.), Number(2.), Binary(Pow), Unary(Minus)])
        // );
        // assert_eq!(
        //     to_rpn(&[Number(1.), Unary(Fact), Binary(Pow), Number(2.)]),
        //     Ok(vec![Number(1.), Unary(Fact), Number(2.), Binary(Pow)])
        // );
        // assert_eq!(
        //     to_rpn(&[
        //         Number(1.),
        //         Unary(Fact),
        //         Binary(Div),
        //         LParen,
        //         Number(2.),
        //         Binary(Plus),
        //         Number(3.),
        //         RParen,
        //         Unary(Fact)
        //     ]),
        //     Ok(vec![
        //         Number(1.),
        //         Unary(Fact),
        //         Number(2.),
        //         Number(3.),
        //         Binary(Plus),
        //         Unary(Fact),
        //         Binary(Div)
        //     ])
        // );
        // assert_eq!(
        //     to_rpn(&[
        //         Number(3.),
        //         Binary(Minus),
        //         Number(1.),
        //         Binary(Times),
        //         Number(2.)
        //     ]),
        //     Ok(vec![
        //         Number(3.),
        //         Number(1.),
        //         Number(2.),
        //         Binary(Times),
        //         Binary(Minus)
        //     ])
        // );
        // assert_eq!(
        //     to_rpn(&[
        //         LParen,
        //         Number(3.),
        //         Binary(Minus),
        //         Number(1.),
        //         RParen,
        //         Binary(Times),
        //         Number(2.)
        //     ]),
        //     Ok(vec![
        //         Number(3.),
        //         Number(1.),
        //         Binary(Minus),
        //         Number(2.),
        //         Binary(Times)
        //     ])
        // );
        // assert_eq!(
        //     to_rpn(&[
        //         Number(1.),
        //         Binary(Minus),
        //         Unary(Minus),
        //         Unary(Minus),
        //         Number(2.)
        //     ]),
        //     Ok(vec![
        //         Number(1.),
        //         Number(2.),
        //         Unary(Minus),
        //         Unary(Minus),
        //         Binary(Minus)
        //     ])
        // );
        // assert_eq!(
        //     to_rpn(&[Var("x".into()), Binary(Plus), Var("y".into())]),
        //     Ok(vec![Var("x".into()), Var("y".into()), Binary(Plus)])
        // );

        // assert_eq!(
        //     to_rpn(&[
        //         Func("max".into(), None),
        //         Func("sin".into(), None),
        //         Number(1f64),
        //         RParen,
        //         Comma,
        //         Func("cos".into(), None),
        //         Number(2f64),
        //         RParen,
        //         RParen
        //     ]),
        //     Ok(vec![
        //         Number(1f64),
        //         Func("sin".into(), Some(1)),
        //         Number(2f64),
        //         Func("cos".into(), Some(1)),
        //         Func("max".into(), Some(2))
        //     ])
        // );

        // assert_eq!(to_rpn(&[Binary(Plus)]), Err(RPNError::NotEnoughOperands(0)));
        // assert_eq!(
        //     to_rpn(&[Func("f".into(), None), Binary(Plus), RParen]),
        //     Err(RPNError::NotEnoughOperands(0))
        // );
        // assert_eq!(
        //     to_rpn(&[Var("x".into()), Number(1.)]),
        //     Err(RPNError::TooManyOperands)
        // );
        // assert_eq!(to_rpn(&[LParen]), Err(RPNError::MismatchedLParen(0)));
        // assert_eq!(to_rpn(&[RParen]), Err(RPNError::MismatchedRParen(0)));
        // assert_eq!(
        //     to_rpn(&[Func("sin".into(), None)]),
        //     Err(RPNError::MismatchedLParen(0))
        // );
        // assert_eq!(to_rpn(&[Comma]), Err(RPNError::UnexpectedComma(0)));
        // assert_eq!(
        //     to_rpn(&[Func("f".into(), None), Comma]),
        //     Err(RPNError::MismatchedLParen(0))
        // );
        // assert_eq!(
        //     to_rpn(&[Func("f".into(), None), LParen, Comma, RParen]),
        //     Err(RPNError::UnexpectedComma(2))
        // );

    }
}
