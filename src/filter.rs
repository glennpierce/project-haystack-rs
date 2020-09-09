use std::f64::consts;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;
use std::str::FromStr;
use std::pin::Pin;
use std::future::Future;

use crate::filter_shunting_yard::to_rpn;
use std;
use std::fmt;
use filter_tokenizer::{tokenize, FilterToken};
use chrono::{DateTime, Utc};

use std::collections::HashMap;

use crate::*;
use crate::error::*;
use crate::token::Token;

type ContextHashMap<K, V> = HashMap<K, V>;

/// Representation of a parsed expression.
///
/// The expression is internally stored in the [reverse Polish notation (RPN)][RPN] as a sequence
/// of `Token`s.
///
/// Methods `bind`, `bind_with_context`, `bind2`, ... can be used to create  closures from
/// the expression that then can be passed around and used as any other `Fn` closures.
///
/// ```rust
/// let func = "x^2".parse::<evaluator::Filter>().unwrap().bind("x").unwrap();
/// let r = Some(2.).map(func);
/// assert_eq!(r, Some(4.));
/// ```
///
/// [RPN]: https://en.wikipedia.org/wiki/Reverse_Polish_notation
/// 
/// 

#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    rpn: Vec<FilterToken>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StackValue {
    Token(Token),
    Name(String, Vec<(String, String)>)
}

impl fmt::Display for StackValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StackValue::Token(v) => write!(f, "{}", v),
            StackValue::Name(n, v) => write!(f, "{:?}: {:?}", n, v)
            }
    }
}

pub fn eval_str<F>(expr: &str, f: F) -> Result<StackValue, FilterError> 
where F: Fn(&str) -> (String, Vec<(String, String)>) {

    let mut stack : Vec<StackValue> = Vec::with_capacity(16);

    let tokens = tokenize(expr)?;
    let rpn = to_rpn(&tokens)?;

    for token in &rpn {

        match *token {
            FilterToken::Val(ref n) => {
                stack.push(StackValue::Token(n.clone()));
            },
            FilterToken::Bool(f) => {
                stack.push(StackValue::Token(Token::Bool(f)));
            },
            // FilterToken::Name(ref n) => {
            //     if let Some(v) = ctx.get_aliases_values(*n) {
            //         stack.push(StackValue::Values(v));
            //     } else {
            //         return Err(FilterError::UnknownVariable(n.to_string()));
            //     }
            // },
            FilterToken::Binary(op) => {
                let right_stack_value: StackValue = stack.pop().unwrap();
                let left_stack_value: StackValue = stack.pop().unwrap();
                
                // match (left_stack_value, right_stack_value) {

                    // (StackValue::Value(left), StackValue::Value(right)) => {
                    //     let r = match op {
                    //         Plus => left + right,
                    //         Minus => left - right,
                    //         Times => left * right,
                    //         Div => left / right,
                    //         Rem => left % right,
                    //         Pow => left.powf(right),
                    //         _ => {
                    //             return Err(FilterError::EvalError(format!(
                    //                 "Unimplemented binary operation: {:?}",
                    //                 op
                    //             )));
                    //         }
                    //     };

                    //     stack.push(StackValue::Value(r));
                    // },
                    // (StackValue::Value(left), StackValue::Values(right)) => {

                        // let r = match op {
                        //     Plus => apply_function_to_timevalue_vector(right, left, &|a, b| a + b),
                        //     Minus => apply_function_to_timevalue_vector(right, left, &|a, b| b - a),
                        //     Times => apply_function_to_timevalue_vector(right, left, &|a, b| b * a),
                        //     Div => apply_function_to_timevalue_vector(right, left, &|a, b| b / a),
                        //     Rem => apply_function_to_timevalue_vector(right, left, &|a, b| b % a),
                        //     Pow =>  apply_function_to_timevalue_vector(right, left, &|a, b| b.powf(a)),
                        //     _ => {
                        //         return Err(FilterError::EvalError(format!(
                        //             "Unimplemented binary operation: {:?}",
                        //             op
                        //         )));
                        //     }
                        // }?;

                        // stack.push(StackValue::Values(r));
                    // }
                    // (StackValue::Values(left), StackValue::Value(right)) => {
                        
                        // let r = match op {
                        //     Plus => apply_function_to_timevalue_vector(left, right, &|a, b| a + b),
                        //     Minus => apply_function_to_timevalue_vector(left, right, &|a, b| a - b),
                        //     Times => apply_function_to_timevalue_vector(left, right, &|a, b| a * b),
                        //     Div => apply_function_to_timevalue_vector(left, right, &|a, b| a / b),
                        //     Rem => apply_function_to_timevalue_vector(left, right, &|a, b| a % b),
                        //     Pow => apply_function_to_timevalue_vector(left, right, &|a, b| a.powf(b)),
                        //     _ => {
                        //         return Err(FilterError::EvalError(format!(
                        //             "Unimplemented binary operation: {:?}",
                        //             op
                        //         )));
                        //     }
                        // }?;

                        // stack.push(StackValue::Values(r));

                    // }
                    // (StackValue::Values(left), StackValue::Values(right)) => {
                        // let r = match op {
                        //     Plus =>  apply_function_to_timevalue_vectors(left, right, &|a, b| a + b),
                        //     Minus =>  apply_function_to_timevalue_vectors(left, right, &|a, b| a - b),
                        //     Times =>  apply_function_to_timevalue_vectors(left, right, &|a, b| a * b),
                        //     Div =>  apply_function_to_timevalue_vectors(left, right, &|a, b| a / b),
                        //     Rem =>  apply_function_to_timevalue_vectors(left, right, &|a, b| a % b),
                        //     Pow =>  apply_function_to_timevalue_vectors(left, right, &|a, b| a.powf(b)),
                        //     _ => {
                        //         return Err(FilterError::EvalError(format!(
                        //             "Unimplemented binary operation: {:?}",
                        //             op
                        //         )));
                        //     }
                        // }?;

                        // stack.push(StackValue::Values(r));
                //    }
                // };
            }
            FilterToken::Unary(op) => {

                let x_stack_value: StackValue = stack.pop().unwrap();

                // match x_stack_value {
                    // StackValue::Value(x) => {
                    //     let r = match op {
                    //         Plus => x,
                    //         Minus => -x,
                    //         _ => {
                    //             return Err(FilterError::EvalError(format!(
                    //                 "Unimplemented unary operation: {:?}",
                    //                 op
                    //             )));
                    //         }
                    //     };

                    //     stack.push(StackValue::Value(r));
                    // },
                    // StackValue::Values(x) => {

                    //     let r = match op {
                    //         Plus =>  Ok(x),
                    //         Minus => apply_function_to_timevalue_vector(x, 0.0, &|a, _b| -a),
                    //         _ => {
                    //             return Err(FilterError::EvalError(format!(
                    //                 "Unimplemented binary operation: {:?}",
                    //                 op
                    //             )));
                    //         }
                    //     }?;

                    //     stack.push(StackValue::Values(r));
                    // }
                // };


            },
            _ => return Err(FilterError::EvalError(format!("Unrecognized token: {:?}", token))),
        }
    }

    let r = stack.pop().expect("Stack is empty, this is impossible.");
    if !stack.is_empty() {
        return Err(FilterError::EvalError(format!(
            "There are still {} items on the stack.",
            stack.len()
        )));
    }
    
    Ok(r)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval() {
        // assert_eq!(eval_str("2 + 3"), Ok(StackValue::Value(5.)));
        // assert_eq!(eval_str("2 + (3 + 4)"), Ok(StackValue::Value(9.)));
        // assert_eq!(eval_str("-2 + (4 - 1)"), Ok(StackValue::Value(1.)));
        // assert_eq!(eval_str("-2^(4 - 3)"), Ok(StackValue::Value(-2.)));
        // assert_eq!(eval_str("-2^(4 - 3) * (3 + 4)"), Ok(StackValue::Value(-14.)));
        // assert_eq!(eval_str("a + 3"), Err(Error::UnknownVariable("a".into())));
        // assert_eq!(eval_str("round(sin (pi) * cos(0))"), Ok(StackValue::Value(0.)));
        // assert_eq!(eval_str("round( sqrt(3^2 + 4^2)) "), Ok(StackValue::Value(5.)));
        // assert_eq!(
        //     eval_str("sin(1.) + cos(2.)"),
        //     Ok(StackValue::Value((1f64).sin() + (2f64).cos()))
        // );
        // assert_eq!(eval_str("10 % 9"), Ok(StackValue::Value(10f64 % 9f64)));

        //assert_eq!(eval_str("[611371] + [611372]"), Ok(StackValue::Value(5.)));
    }

    #[test]
    fn test_builtins() {
       // assert_eq!(eval_str("atan2(1.,2.)"), Ok(StackValue::Value((1f64).atan2(2.))));
    }
}
