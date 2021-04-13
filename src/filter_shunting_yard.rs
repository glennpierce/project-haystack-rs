//! Implementation of the shunting-yard algorithm for converting an infix expression to an
//! expression in reverse Polish notation (RPN).
//!
//! See the Wikipedia articles on the [shunting-yard algorithm][shunting] and on [reverse Polish
//! notation][RPN] for more details.
//!
//! [RPN]: https://en.wikipedia.org/wiki/Reverse_Polish_notation
//! [shunting]: https://en.wikipedia.org/wiki/Shunting-yard_algorithm
// use std;
// use std::fmt;

use crate::error::*;
use crate::token::Token;
// use crate::filter_tokenizer::{FilterToken, Operation, tokenize, tokenize2};
use crate::filter_tokenizer::{FilterToken, Operation};

#[derive(Debug, Clone, Copy)]
enum Associativity {
    Left,
    Right,
    NA,
}


/// Returns the operator precedence and associativity for a given token.
fn prec_assoc(token: &FilterToken) -> (u32, Associativity) {

    use Operation::*;
    // use Token::*;
    use FilterToken::*;
    match *token {
        Binary(op) => match op {
            Or => (1, Associativity::Left),
            And => (2, Associativity::Left),
            Equals => (3, Associativity::Left),
            LessThan | MoreThan | LessThanEquals | MoreThanEquals => (4, Associativity::Left),
            // Has => (5, Associativity::Left),
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
            FilterToken::Path(_) => output.push(token),
            FilterToken::Compare(_, _, _) => output.push(token),
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
            FilterToken::Path(_) => n_operands += 1,
            FilterToken::Compare(_, _, _) => n_operands += 1,
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
    fn test_to_rpn() {


        assert_eq!(to_rpn(&tokenize("elec and heat").unwrap()),
            Ok(vec![id_to_path!("elec"), id_to_path!("heat"), Binary(Operation::And)]));

        // equip and siteRef->geoCity == "Chicago"
        // The way to read the above expression is match an entity if:

        // it has equip tag
        // and it has a siteRef tag which is a Ref
        // and what the siteRef tag points to has the geoCity tag
        // and that the site's geoCity tag is equal to "Chicago"

        assert_eq!(to_rpn(&tokenize("equip and siteRef->geoCity->dis == \"Chicago\"").unwrap()),
            Ok(vec![id_to_path!("equip"),
                    Compare(
                        Box::new(FilterToken::Path(vec![id_to_token!("siteRef"), id_to_token!("geoCity"), id_to_token!("dis")])),
                        Operation::Equals,
                        Box::new( FilterToken::Val( Token::EscapedString("Chicago".to_string()) ) )
                    )
                    ,
                    Binary(Operation::And)]));

        // In RPN, the numbers and operators are listed one after another, and an operator always acts on the most recent numbers in the list.

        // push equip on stack
        // push siteRef on stack
        // push geoCity on stack     stack = [geoCity, siteRef, equip]
        // Apply binary has to  siteRef has geoCity . Add geoCity if exists     stack = [geoCity, equip]
        // push dis to stack                            stack = [dis, geoCity, equip]
        // Apply binary has to  geoCity has dis. Add dis if exists    stack = [dis, equip]
        // push EscapedString("Chicago") to stack   stack = [EscapedString("Chicago"), dis, equip]
        // Apply equals to dis == EscapedString("Chicago")  

    }

    #[test]
    fn test_to_rpn2() {


        assert_eq!(to_rpn(&tokenize2("elec and heat").unwrap()),
            Ok(vec![id_to_path!("elec"), id_to_path!("heat"), Binary(Operation::And)]));

        // equip and siteRef->geoCity == "Chicago"
        // The way to read the above expression is match an entity if:

        // it has equip tag
        // and it has a siteRef tag which is a Ref
        // and what the siteRef tag points to has the geoCity tag
        // and that the site's geoCity tag is equal to "Chicago"

        assert_eq!(to_rpn(&tokenize2("equip and siteRef->geoCity->dis == \"Chicago\"").unwrap()),
            Ok(vec![id_to_path!("equip"),
                    FilterToken::Path(vec![id_to_token!("siteRef"), id_to_token!("geoCity"), id_to_token!("dis")]),
                    FilterToken::Val( Token::EscapedString("Chicago".to_string()) ),
                    Binary(Operation::Equals),
                    Binary(Operation::And)
                   ]));

        
        assert_eq!(to_rpn(&tokenize2("elec and heat and siteRef->geoCity->dis == \"Chicago\"").unwrap()),
        Ok(vec![id_to_path!("elec"),
                id_to_path!("heat"),
                Binary(Operation::And),
                FilterToken::Path(vec![id_to_token!("siteRef"), id_to_token!("geoCity"), id_to_token!("dis")]),
                FilterToken::Val( Token::EscapedString("Chicago".to_string()) ),
                Binary(Operation::Equals),
                Binary(Operation::And)
                ]));

        // In RPN, the numbers and operators are listed one after another, and an operator always acts on the most recent numbers in the list.

        // push equip on stack
        // push siteRef on stack
        // push geoCity on stack     stack = [geoCity, siteRef, equip]
        // Apply binary has to  siteRef has geoCity . Add geoCity if exists     stack = [geoCity, equip]
        // push dis to stack                            stack = [dis, geoCity, equip]
        // Apply binary has to  geoCity has dis. Add dis if exists    stack = [dis, equip]
        // push EscapedString("Chicago") to stack   stack = [EscapedString("Chicago"), dis, equip]
        // Apply equals to dis == EscapedString("Chicago")  

    }
}
