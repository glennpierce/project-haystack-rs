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


// pub enum FilterToken {
//     /// Binary operation.
//     Binary(Operation),
//     /// Unary operation.
//     Unary(Operation),

//     /// Left parenthesis.
//     LParen,
//     /// Right parenthesis.
//     RParen,

//     Bool(bool),
//     Name(String),
//     Val(Token),
// }

impl Filter {
    /// Evaluates the expression.
    pub fn eval(&self) -> Result<StackValue, FilterError> {
        self.eval_with_context(builtin())
    }

    /// Evaluates the expression with variables given by the argument.
    pub fn eval_with_context(&self, ctx: Context) -> Result<StackValue, FilterError> {

        let mut stack : Vec<StackValue> = Vec::with_capacity(16);

        for token in &self.rpn {

            match *token {
                FilterToken::Val(ref n) => {
                    stack.push(StackValue::Token(n.clone()));
                },
                FilterToken::Bool(f) => {
                    stack.push(StackValue::Token(Token::Bool(f)));
                },
                // FilterToken::Alias(ref n) => {
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
}


// pub fn eval(&self) -> Result<StackValue, Error> {
//     self.eval_with_context(builtin())
// }

pub fn eval_str_with_context<S: AsRef<str>>(expr: S, ctx: Context) -> Result<StackValue, FilterError> {
    let expr = Filter::from_str(expr.as_ref())?;
    expr.eval_with_context(ctx)
}

/// Evaluates a string with built-in constants and functions.
pub fn eval_str(expr: &str) -> Result<StackValue, FilterError> {
    eval_str_with_context(expr, builtin())
}

impl FromStr for Filter {
    type Err = FilterError;
    /// Constructs an expression by parsing a string.
    fn from_str(s: &str) -> Result<Self, Self::Err> {

        let tokens = tokenize(s)?;
        let rpn = to_rpn(&tokens)?;

        Ok(Filter { rpn: rpn })
    }
}

impl Deref for Filter {
    type Target = [FilterToken];

    fn deref(&self) -> &[FilterToken] {
        &self.rpn
    }
}

/// Function evaluation error.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FuncEvalError {
    TooFewArguments,
    TooManyArguments,
    NumberArgs(usize),
    UnknownFunction,
}

impl fmt::Display for FuncEvalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FuncEvalError::UnknownFunction => write!(f, "Unknown function"),
            FuncEvalError::NumberArgs(i) => write!(f, "Expected {} arguments", i),
            FuncEvalError::TooFewArguments => write!(f, "Too few arguments"),
            FuncEvalError::TooManyArguments => write!(f, "Too many arguments"),
        }
    }
}

impl std::error::Error for FuncEvalError {
    fn description(&self) -> &str {
        match *self {
            FuncEvalError::UnknownFunction => "unknown function",
            FuncEvalError::NumberArgs(_) => "wrong number of function arguments",
            FuncEvalError::TooFewArguments => "too few function arguments",
            FuncEvalError::TooManyArguments => "too many function arguments",
        }
    }
}

#[doc(hidden)]
pub fn max_array(xs: &[f64]) -> f64 {
    xs.iter().fold(::std::f64::NEG_INFINITY, |m, &x| m.max(x))
}

#[doc(hidden)]
pub fn min_array(xs: &[f64]) -> f64 {
    xs.iter().fold(::std::f64::INFINITY, |m, &x| m.min(x))
}

/// Returns the built-in constants and functions in a form that can be used as a `ContextProvider`.
#[doc(hidden)]
pub fn builtin() -> Context {
    // TODO: cache this (lazy_static)
    Context::new()
}

/// A structure for storing variables/constants and functions to be used in an expression.
///
/// # Example
///
/// ```rust
/// use evaluator::{eval_str_with_context, Context};
///
/// let mut ctx = Context::new(); // builtins
/// ctx.var("x", 3.)
///    .func("f", |x| 2. * x)
///    .funcn("sum", |xs| xs.iter().sum(), ..);
///
/// assert_eq!(eval_str_with_context("pi + sum(1., 2.) + f(x)", &ctx),
///            Ok(std::f64::consts::PI + 1. + 2. + 2. * 3.));
/// ```
//#[derive(Clone)]
pub struct Context {
    vars: ContextHashMap<String, f64>,
    aliases: ContextHashMap<String, Vec<(DateTime<Utc>, f64)>>,
}

impl Context {
    /// Creates a context with built-in constants and functions.
    pub fn new() -> Context {

            let mut ctx = Context::empty();
            ctx.var("pi", consts::PI);
            ctx.var("e", consts::E);

            ctx
    }

    /// Creates an empty contexts.
    pub fn empty() -> Context {
        Context {
            vars: ContextHashMap::default(),
            aliases: ContextHashMap::default(),
        }
    }

    fn get_var(&self, name: &str) -> Option<f64> {
        self.vars.get(name).cloned()
    }

    fn get_aliases_values(&self, id: i32) -> Option<Vec<(DateTime<Utc>, f64)>> {
        self.aliases.get(&id.to_string()).cloned()
    }

    /// Adds a new variable/constant.
    pub fn var<S: Into<String>>(&mut self, var: S, value: f64) -> &mut Self {
        self.vars.insert(var.into(), value);
        self
    }

    pub fn alias<S: Into<String>>(&mut self, alias: S, value: Vec<(DateTime<Utc>, f64)>) -> &mut Self {
        self.aliases.insert(alias.into(), value);
        self
    }
}

impl Default for Context {
    fn default() -> Self {
        Context::new()
    }
}

type GuardedFunc = Box<dyn Fn(&[f64]) -> Pin<Box<Result<f64, FuncEvalError>>> + 'static + Send>;

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
