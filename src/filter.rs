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
use filter_tokenizer::{tokenize, FilterToken, Operation};
use chrono::{DateTime, Utc};

use std::collections::{HashSet, HashMap};

use crate::*;
use crate::error::*;
use crate::token::Token;

use array_tool::vec::Intersect;
use array_tool::uniques;

use itertools::Itertools;
use itertools::EitherOrBoth::{Both, Left, Right};


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

// pub trait HayStackTagManager: fmt::Debug
// {
//     fn get_tag_names_for_ref(ref_name: &str) -> &Vec<String>;

//     fn get_refs_with_tag_name(tag_name: &str) -> &Vec<(String, String, Token)>;
// }





//pub type Tag = (String, Token);
// Id, TagName, Value
pub type RefTag = (Token, String, Token);
pub type RefTags = Vec<RefTag>;


pub type HaystackTag = (String, Token);
pub type HaystackTags = Vec<HaystackTag>;

// pub type NameWithTags = (String, Vec<(String, String, Token)>);

#[derive(Debug, Clone, PartialEq)]
pub enum StackValue {
    Token(Token),
    //Name(RefTags)
    Refs(Vec<Token>)
}

impl fmt::Display for StackValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StackValue::Token(v) => write!(f, "{}", v),
            StackValue::Refs(v) => write!(f, "{:?}", v)
            }
    }
}

pub fn filter_eval_str(expr: &str, f: &dyn Fn() -> RefTags) -> Result<StackValue, FilterError> 
{
    let mut stack : Vec<StackValue> = Vec::with_capacity(16);

    // haystack_tag_name_store is haystack_tag_id -> (haystack_tag_value, id)
    // We do this for fater lookup of refs with tag
    let mut haystack_tag_name_store: HashMap<String, Vec<Token>> = HashMap::new();
    let mut haystack_ref_name_store: HashMap<String, HaystackTags> = HashMap::new();

    let values = f();

    for (id, haystack_tag_name, haystack_tag_value) in values.clone() {
        
        match haystack_ref_name_store.get_mut(&id.to_string()) {
            Some(vec_values) => {
                vec_values.push((haystack_tag_name, haystack_tag_value));
            },
            None => {
                haystack_ref_name_store.insert(id.to_string(), vec![(haystack_tag_name, haystack_tag_value)]);
            }
        }
        
    }

    for (id, haystack_tag_name, haystack_tag_value) in values {
        
        match haystack_tag_name_store.get_mut(&haystack_tag_name) {
            Some(vec_values) => {
                vec_values.push(id.clone());
            },
            None => {
                haystack_tag_name_store.insert(haystack_tag_name.clone(), vec![id.clone()]);
            }
        }
        
    }

    // for (id, haystack_tags) in values {
    //     for (haystack_tag, haystack_tag_value) in haystack_tags {

    //         match haystack_tag_name_store.get_mut(&haystack_tag) {
    //             Some(vec_values) => {
    //                 vec_values.push((haystack_tag_value, id.clone()));
    //             },
    //             None => {
    //                 haystack_tag_name_store.insert(haystack_tag, vec![(haystack_tag_value, id.clone())]);
    //             }
    //         }
    //     }
    // }

    let tokens = tokenize(expr)?;
    let rpn = to_rpn(&tokens)?;

    println!("\n\nrpn: {:?}", rpn);

    for token in &rpn {

        match *token {
            FilterToken::Val(ref n) => {
                stack.push(StackValue::Token(n.clone()));
            },
            FilterToken::Bool(f) => {
                stack.push(StackValue::Token(Token::Bool(f)));
            },
            FilterToken::Name(ref tag_name) => {   // Name here is a tag. We need to get all the refs with that tag

               // NameWithTags = (String, Vec<Tag>);
               // returns  Vec<(Token, String)>
               // stack.push(StackValue::Name((tag_name.clone(), haystack_tag_name_store.get(tag_name).unwrap_or(&vec![]).clone()) ));
               // stack.push(StackValue::Name(haystack_tag_name_store.get(tag_name).unwrap_or(&vec![]).clone()));

               // Return all ids with tag_name
               //let refs = haystack_tag_name_store.get(tag_name).unwrap_or(&vec![]).clone()

               //let filtered: RefTags = right.iter().filter(|x| site_refs.contains(&x.0) ).cloned().collect();

               stack.push(StackValue::Refs(haystack_tag_name_store.get(tag_name).unwrap_or(&vec![]).clone()));
            },
            FilterToken::Binary(op) => {

                println!("Binary op: Stack = {:?}", stack);

                let right_stack_value: StackValue = stack.pop().unwrap();
                let left_stack_value: StackValue = stack.pop().unwrap();
                
                // let right_stack_tags: Vec<String> = haystack_ref_name_store.get()

                match (left_stack_value, right_stack_value) {

                    (StackValue::Refs(left), StackValue::Refs(right)) => {
                        let r = match op {
                            Operation::And => {


                                // We only return where the haystack tag names match
                                //left.iter().map(|l| l.1[])
                                //let left_names: Vec<String> = left.iter().map(|i| i.0.clone()).collect();
                                //let right_names: Vec<String> = right.iter().map(|i| i.0.clone()).collect();

                                println!("AND left: {:?}", left);
                                println!("AND right: {:?}", right);

                                let mut v = left.intersect_if(right, |l, r| l == r);

                                v.sort();

                                v

                                // left: [("@1", "elec", EscapedString("elec")), ("@2", "elec", EscapedString("elec"))]
                                // right: [("@1", "heat", EscapedString("heat")), ("@3", "heat", EscapedString("heat"))]

                                // let mut merged: RefTags = vec![];

                                // for it in left.iter().zip_longest(right.iter()) {
                                //     match it {
                                //         Both(l, r) => {
                                //             if l.0 == r.0 {
                                //                 merged.push(l.clone());
                                //                 merged.push(r.clone());
                                //             }
                                //         },
                                //         Left(l) => (),
                                //         Right(r) => (),
                                //     }
                                // }

                                // merged
                            },
                            Operation::Or => {

                                // println!("left: {:?}", left);
                                // println!("right: {:?}", right);

                                let mut merged = left.clone();
                                merged.extend(right);

                                let hs = merged.iter().cloned().collect::<HashSet<Token>>();

                                let mut v: Vec<Token> = hs.into_iter().collect();
                                v.sort();
                                v
                            },
                        
                            Operation::Has => {

                                println!("left: {:?}", left);
                                println!("right: {:?}", right);

                                // It has a siteRef tag which is a Ref
                                // and what the siteRef tag points to has the geoCity tag
                                // left: [("@3", "siteRef", Ref("1", None)), ("@5", "siteRef", Ref("2", None))]
                                // right: [("@1", "geoCity", EscapedString("Chicago")), ("@4", "geoCity", EscapedString("London"))]

                                fn match_refs(token: Token) -> Option<String> {
                                    match &token {
                                        Token::Ref(r, display) => Some(token.clone().to_string()),
                                        _ => None
                                    }
                                }

                                // filter right to have only ids match siteRefs in left
                             //   let site_refs: Vec<String> = left.iter().flat_map(|x| match_refs(x.2.clone()) ).collect();
                            //    let filtered: RefTags = right.iter().filter(|x| site_refs.contains(&x.0) ).cloned().collect();

                                // Must now get all tags for this ref to return


                             //   println!("siteRefs: {:?}", site_refs);

                                //if left[0].2 == right[0]

                                // let mut merged = left.clone();
                                // merged.extend(right);

                            //    println!("filtered: {:?}", filtered);

                             //   filtered
                             vec![]

                            },
                            _ => {
                                return Err(FilterError::EvalError(format!(
                                    "Unimplemented binary operation: {:?}",
                                    op
                                )));
                            }
                        };

                        stack.push(StackValue::Refs(r));
                    },

                    _ => {
                        return Err(FilterError::EvalError("Unimplemented binary types".to_string()));
                    }
       
                 };

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
                match x_stack_value {

                    StackValue::Refs(x) => {

                        let r = match op {
                       
                            Operation::Not => {

                                // // The vec after not should all have the same haystack key
                                // // for example below they all have elec
                                // let key = x[0].1.clone();
                                // // x: [("@1", "elec", EscapedString("elec")), ("@3", "elec", EscapedString("elec")), ("@5", "elec", EscapedString("elec"))]
                                // let mut merged: RefTags = vec![];

                                // for (k, v) in haystack_tag_name_store.iter() {
                                //     if *k != key {
                                //         // RefTags
                                //         merged.extend(v.clone());
                                //     }
                                // }

                                // // stack.push(StackValue::Name(merged));
                                // merged

                                vec![]
                            },

                            _ => {
                                return Err(FilterError::EvalError(format!(
                                    "Unimplemented binary operation: {:?}",
                                    op
                                )));
                            }
                        };

                        stack.push(StackValue::Refs(r));
                    },

                    _ => {
                        return Err(FilterError::EvalError("Unimplemented binary types".to_string()));
                    }
       
                 };

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

        // pub type RefTag = (String, String, Token);
        // pub type RefTags = Vec<RefTag>;

        // In the real world the idea is to get all the refs with tags from a db or whatever.
        // Maybe inefficient but will do for now.
        fn get_tags() -> RefTags {
            vec![
                (Token::Ref("@1".to_string(), None), "dis".to_string(), Token::EscapedString("One".to_string())),
                (Token::Ref("@1".to_string(), None), "elec".to_string(), Token::EscapedString("elec".to_string())),
                (Token::Ref("@1".to_string(), None), "heat".to_string(), Token::EscapedString("heat".to_string())),
                (Token::Ref("@1".to_string(), None), "water".to_string(), Token::EscapedString("water".to_string())),
                (Token::Ref("@1".to_string(), None), "geoCity".to_string(), Token::EscapedString("Chicago".to_string())),

                (Token::Ref("@2".to_string(), None), "dis".to_string(), Token::EscapedString("Two".to_string())),
                
                (Token::Ref("@3".to_string(), None), "dis".to_string(), Token::EscapedString("Three".to_string())),
                (Token::Ref("@3".to_string(), None), "heat".to_string(), Token::EscapedString("heat".to_string())),   
                (Token::Ref("@3".to_string(), None), "elec".to_string(), Token::EscapedString("elec".to_string())),  
                (Token::Ref("@3".to_string(), None), "siteRef".to_string(), Token::Ref("1".to_string(), None)),       
                
                (Token::Ref("@4".to_string(), None), "dis".to_string(), Token::EscapedString("Four".to_string())),
                (Token::Ref("@4".to_string(), None), "heat".to_string(), Token::EscapedString("Four".to_string())),  
                (Token::Ref("@4".to_string(), None), "geoCity".to_string(), Token::EscapedString("London".to_string())),

                (Token::Ref("@5".to_string(), None), "dis".to_string(), Token::EscapedString("Five".to_string())),
                (Token::Ref("@5".to_string(), None), "heat".to_string(), Token::EscapedString("heat".to_string())),   
                (Token::Ref("@5".to_string(), None), "elec".to_string(), Token::EscapedString("elec".to_string())),  
                (Token::Ref("@5".to_string(), None), "water".to_string(), Token::EscapedString("water".to_string())),   
                (Token::Ref("@5".to_string(), None), "siteRef".to_string(), Token::Ref("2".to_string(), None)),         
                
            ]
        }

        println!("{:?}", filter_eval_str("elec and heat", &get_tags));
        println!("{:?}", filter_eval_str("elec or heat", &get_tags));
        // println!("{:?}", filter_eval_str("not elec", &get_tags));
        // println!("{:?}", filter_eval_str("not elec and water", &get_tags));
        // println!("{:?}", filter_eval_str("elec and siteRef->geoCity", &get_tags));


        // assert_eq!(filter_eval_str("2 + 3"), Ok(StackValue::Value(5.)));
        // assert_eq!(filter_eval_str("2 + (3 + 4)"), Ok(StackValue::Value(9.)));
        // assert_eq!(filter_eval_str("-2 + (4 - 1)"), Ok(StackValue::Value(1.)));
        // assert_eq!(filter_eval_str("-2^(4 - 3)"), Ok(StackValue::Value(-2.)));
        // assert_eq!(filter_eval_str("-2^(4 - 3) * (3 + 4)"), Ok(StackValue::Value(-14.)));
        // assert_eq!(filter_eval_str("a + 3"), Err(Error::UnknownVariable("a".into())));
        // assert_eq!(filter_eval_str("round(sin (pi) * cos(0))"), Ok(StackValue::Value(0.)));
        // assert_eq!(filter_eval_str("round( sqrt(3^2 + 4^2)) "), Ok(StackValue::Value(5.)));
        // assert_eq!(
        //     filter_eval_str("sin(1.) + cos(2.)"),
        //     Ok(StackValue::Value((1f64).sin() + (2f64).cos()))
        // );
        // assert_eq!(filter_eval_str("10 % 9"), Ok(StackValue::Value(10f64 % 9f64)));

        //assert_eq!(filter_eval_str("[611371] + [611372]"), Ok(StackValue::Value(5.)));
    }

    #[test]
    fn test_builtins() {
       // assert_eq!(filter_eval_str("atan2(1.,2.)"), Ok(StackValue::Value((1f64).atan2(2.))));
    }
}
