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
use crate::token::Tag;

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
pub type RefTag = (Token, Vec<Tag>);
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




fn variant_eq<T>(a: &T, b: &T) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}



/*
struct IdTagManager {
    tags_to_ref_store: HashMap<String, Vec<Token>>,
    ref_to_tags_store: HashMap<Token, HaystackTags>,
}

impl IdTagManager {
    fn new() -> Self {
        IdTagManager {
            tags_to_ref_store: HashMap::new(),
            ref_to_tags_store: HashMap::new()
        }
    }

    // type RefTag = (Token, String, Token);
    fn update(&mut self, f: &dyn Fn() -> RefTags) {
        let values = f();

        for (id, haystack_tag_name, haystack_tag_value) in values.clone() {
            
            match self.ref_to_tags_store.get_mut(&id) {
                Some(vec_values) => {
                    vec_values.push((haystack_tag_name, haystack_tag_value));
                },
                None => {
                    self.ref_to_tags_store.insert(id, vec![(haystack_tag_name, haystack_tag_value)]);
                }
            }
            
        }

        for (id, haystack_tag_name, haystack_tag_value) in values {
            
            match self.tags_to_ref_store.get_mut(&haystack_tag_name) {
                Some(vec_values) => {
                    vec_values.push(id.clone());
                },
                None => {
                    self.tags_to_ref_store.insert(haystack_tag_name.clone(), vec![id.clone()]);
                }
            }
            
        }
    }

    fn get_tag_for_ref_with_tagname(&self, t: &Token, tagname: &str) -> Option<HaystackTag> {
        let tagname_string = tagname.to_string();
        let tags: &HaystackTags = self.ref_to_tags_store.get(t)?;
        let found = tags.iter().find(|&x| x.0 == tagname_string)?;
        Some(found.clone())
    }

    fn get_refs_containing_haystack_tag_name(&self, haystack_tag_name: &str) -> Vec<Token> {

        if self.tags_to_ref_store.contains_key(&haystack_tag_name.to_string()) {
            return self.tags_to_ref_store.get(&haystack_tag_name.to_string()).unwrap().clone();
        }

        vec![]
    }

    // fn get_all_ref_values_for_items_with_ref_tag_name() {

    // }

    /// Given a vector of Tag Names each defined by (Token::Id) returns the Token::Refs that have those tags
    fn tag_id_to_refs(&self, t: &Token) -> Option<Vec<Token>> {

        let tag_option: Option<String> = match t {
            Token::Id(id) => Some(id.clone()),
            _ => None
        };

        if tag_option.is_none() {
            return None;
        }

        let tag: String = tag_option.unwrap();

        if self.tags_to_ref_store.contains_key(&tag) {
            let tokens: &Vec<Token> = self.tags_to_ref_store.get(&tag).unwrap();
            let ref_tokens = filter_tokens_by_ref_type(&tokens);
            return Some(ref_tokens);
        }

        None
    }
}
*/

fn filter_tokens_by_ref_type(v: &Vec<Token>) -> Vec<Token> {
    fn match_refs(token: &Token) -> Option<Token> {
        match token {
            Token::Ref(r, display) => Some(token.clone()),
            _ => None
        }
    }

    v.iter().flat_map(|t| match_refs(t) ).collect()
}

pub fn contains_ref_with_id(v: &Vec<Tag>, id_token: &Token) -> bool
{
  //  print!("here: {:?}", id_token);

    let result = v.iter().filter(|&t| t.contains_ref_with_id(id_token)).peekable().peek().is_some();

  //  println!("  result: {:?}", result);

    result
}

pub fn find_ref_with_id(v: &Vec<Tag>, id_token: &Token) -> Option<Token>
{
  //  print!("here: {:?}", id_token);

    let result_option: Option<&Tag> = v.iter().find(|t| t.contains_ref_with_id(id_token));

    if result_option.is_none() {
        return None;
    }

    let result: &Tag = result_option.unwrap();

    let token_value: Option<Token> = result.get_value::<Token>();

 //   println!("  token_value: {:?}", token_value);

    token_value
}


fn filter_entities_by_token_name(entities: &RefTags, id: &Token) -> RefTags {
    entities.iter().filter(|&i| i.1.contains(&Tag::new_marker_from_token(id.clone()))).cloned().collect()
}

fn intersect_tags(v1: &Vec<Tag>, v2: &Vec<Tag>) -> bool {
    !v1.intersect(v2.clone()).is_empty()
}

fn filter_entities_by_possible_value_tokens(entities: &RefTags, tags: &Vec<Tag>) -> RefTags {

    //entities.iter().filter(|&i| i.1.contains(&Tag::new_marker_from_token(id.clone()))).cloned().collect()
    entities.iter().filter(|&i| intersect_tags(&i.1, tags)).cloned().collect()
}

fn ids_containing_tag(entities: &RefTags, token: &Token) -> RefTags {
    entities.iter().filter(|&i| i.1.contains(&Tag::new_marker_from_token(token.clone()))).cloned().collect()
}

fn refs_containing_tag(entities: &RefTags, token: &Token) -> RefTags {
    let mut refs: RefTags = vec![];

    for e in entities.iter() {
        let t: Option<Token> = find_ref_with_id(&e.1, &token);
                      
        //  (Token, Vec<Tag>)
        if t.is_some() {

            let token: Token = t.unwrap();

            println!("token: {:?}", token);

            // Here we have thecorrect ref but we want to grab it from the original
            // vec as that has all the correct tags etc  
            println!("entities: {:?}", entities);

            let ref_tag: Option<&RefTag> = entities.iter().find(|&i| i.0 == token );

            println!("ref_tag: {:?}", ref_tag);

            if ref_tag.is_some() {
                refs.push(ref_tag.unwrap().clone());
            }
        }
    }

    refs
}



// fn intersect_refs_against_ids(lhs: &RefTags, rhs: &RefTags, refs_token: &Token, ids_token: &Token) -> RefTags {

//     let refs: RefTags = refs_containing_tag(lhs, refs_token);
//     let ids: RefTags = ids_containing_tag(rhs, ids_token);

//     println!("lhs: {:?}   rhs: {:?}\n", refs_token, ids_token);
//     println!("refs: {:?}   ids: {:?}\n", refs, ids);

//     refs.intersect(ids)
// }

// fn get_refs(entities: &RefTags, refs_token: &Token, ids_token: &Token) -> RefTags {

//     let refs: RefTags = refs_containing_tag(lhs, refs_token);
//     let ids: RefTags = ids_containing_tag(rhs, ids_token);

//     println!("lhs: {:?}   rhs: {:?}\n", refs_token, ids_token);
//     println!("refs: {:?}   ids: {:?}\n", refs, ids);

//     refs.intersect(ids)
// }

pub fn filter_eval_str(expr: &str, f: &dyn Fn() -> RefTags) -> Result<StackValue, FilterError> 
{
    let mut stack : Vec<StackValue> = Vec::with_capacity(16);

    // haystack_tag_name_store is haystack_tag_id -> (haystack_tag_value, id)
    // We do this for fater lookup of refs with tag
    // let mut haystack_tag_name_store: HashMap<String, Vec<Token>> = HashMap::new();
    // let mut haystack_ref_name_store: HashMap<Token, HaystackTags> = HashMap::new();

    let values: RefTags = f();

    let tokens = tokenize(expr)?;
    let rpn = to_rpn(&tokens)?;

 //   println!("\n\nrpn: {:?}", rpn);

    //let mut last_name: Option<String> = None;

    'rpn_loop: for token in &rpn {

     //   println!("\nToken: {:?}", token);

        match *token {
            FilterToken::Val(ref n) => {
                stack.push(StackValue::Token(n.clone()));
            },
            FilterToken::Path(ref tags) => {   // Name here is a tag. We need to get all the refs with that tag

                // Ok path may be one tag name like vec![elec] 
                // Or may be a  ref specified from the first like vec![siteRef, geoCity]
                // All accept the last have to be a Ref
                // ie siteRef->geoCity
                // equipRef->siteRef->dis
                // Get all items with tag equipRef which points to items with tag siteRef which point to items with dis tag
                // Get all tags with tag siteRef which point to ids with tag geoCity

                let mut ids: RefTags = values.clone();
                let mut refs: RefTags = values.clone();

                // value of type `std::vec::Vec<(token::Token, std::vec::Vec<token::Tag>)>` 
                // cannot be built from `std::iter::Iterator<Item=&(token::Token, std::vec::Vec<token::Tag>)>`

                //let mut tag_stack = tags.clone();

                let mut tag_stack: Vec<Token> = tags.iter().rev().cloned().collect();

                // while tag_stack.len() > 0 {
                //     let tag_name = tag_stack.pop().unwrap().clone();
                //     println!("searching tagename: {:?}", tag_name);
                //     ids = ids.iter().filter(|&i| i.1.contains(&Tag::new_marker_from_token(tag_name.clone())) ).cloned().collect();
                //     println!("ids: {:?}\n\n", ids);
                // }

                // siteRef->equipRef->dis

                let tag_name = tag_stack.pop().unwrap().clone();

                println!("tag_name: {:?}", tag_name);

                ids = filter_entities_by_token_name(&ids, &tag_name);

                println!("ids: {:?}", ids);

                while tag_stack.len() > 0 {

                    

                    let tag_name = tag_stack.pop().unwrap().clone();

                    println!("tag_name: {:?}", tag_name);

                    let possible_tags: Vec<Tag> = ids.iter().flat_map(|x| {
                        match &x.0 {
                            Token::Ref(id, _) => Some(Tag::new_ref_from_token(&tag_name, &id)),
                            _ => None
                        }
                    }).collect();

                    println!("possible_tags: {:?}\n\n", possible_tags);

                    ids = filter_entities_by_possible_value_tokens(&ids, &possible_tags);
                    println!("filtered by poss ids: {:?}\n", ids);

                    // //ids = filter_entities_by_token(&ids, &tag_name, value: &Token);

                    // //println!("searching tagname: {:?}", tag_name);
                    // ids = ids_containing_tag(&ids, &tag_name);
                    
                    // refs = refs_containing_tag(&ids, &tag_name);

                   
                    // ids = ids.iter().filter(|&i| i.1.contains(&Tag::new_marker_from_token(tag_name.clone())) ).cloned().collect();
                    // println!("ids: {:?}\n\n", ids);
                }

                
//                 let ids: RefTags = ids_containing_tag(rhs, ids_token);
            

// //// assert_eq!(filter_eval_str("siteRef->equipRef->dis", &get_tags), Ok(refs!()));
//                 if tags.len() == 1 {
//                     let tag_name1 = tag_stack.pop().unwrap().clone();
//                     ids = ids.iter().filter(|&i| i.1.contains(&Tag::new_marker_from_token(tag_name1.clone()))).cloned().collect();
//                 }
//                 else if tags.len() >= 2 {

//                     // siteRef -> equipRef -> pointRef -> let tag_name1 = tag_stack.pop().unwrap().clone();
//                   //  let tag_name2 = tag_stack.pop().unwrap().clone(); //tag_name1 = tag_stack.pop().unwrap().clone();
//                   //  let tag_name2 = tag_stack.pop().unwrap().clone();
//                     // ((siteRef, equipRef), pointRef), dis

//                     //dis , equiprRef, SiteRef

//                     for i in tag_stack.windows(2) {

//                         //
//                        ids = intersect_refs_against_ids(&values, &values, &i[1], &i[0]);

//                        println!("---- {:?} {} {}\n\n", ids, i[0], i[1]);
//                     }
//                }
         

//                 stack.push(StackValue::Refs(ids.iter().map(|x| x.0.clone()).collect()));

            },
            FilterToken::Binary(op) => {

                println!("Binary op: Stack = {:?}", stack);

                let right_stack_value: StackValue = stack.pop().unwrap();
                let left_stack_value: StackValue = stack.pop().unwrap();
                
                // let right_stack_tags: Vec<String> = haystack_ref_name_store.get()

                match (left_stack_value.clone(), right_stack_value.clone()) {

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

             /*                   
                                println!("left_stack_value: {:?}", left_stack_value.clone());
                                println!("left: {:?}", left);
                                println!("right: {:?}", right);

                                // left: [Ref("@3", None), Ref("@5", None)]
                                // right: [Ref("@1", None), Ref("@4", None)]

                                // It has a siteRef tag which is a Ref
                                // and what the siteRef tag points to has the geoCity tag
                                // left: [("@3", "siteRef", Ref("1", None)), ("@5", "siteRef", Ref("2", None))]
                                // right: [("@1", "geoCity", EscapedString("Chicago")), ("@4", "geoCity", EscapedString("London"))]

                                fn match_refs(token: &Token) -> Option<Token> {
                                    match token {
                                        Token::Ref(r, display) => Some(token.clone()),
                                        _ => None
                                    }
                                }

                                // First get the Refs from vector of id refs
                                let mut refs: Vec<Token> = vec![];
                                
                                for (k, v) in haystack_ref_name_store.iter() {
                                    if left.contains(&k) {
                                        for (haystack_tag_name, haystack_tag_value) in v {
                                            if match_refs(haystack_tag_value).is_some() {
                                                refs.push(haystack_tag_value.clone());
                                            }
                                        }
                                    }
                                }

                                println!("refs: {:?}", refs);

                                let mut v = refs.intersect_if(right, |l, r| l == r);

                                v.sort();

                                println!("v: {:?}", v);

                                v

                                

                                

                             //   let notgated: Vec<Token> = haystack_ref_name_store.iter().flat_map(|&(k, v)| match_refs(k) ).collect();
                     

                                //  haystack_ref_name_store: HashMap<Token, HaystackTags> = HashMap::new();
                            //    let left_refs: Vec<Token> = left.iter().map(|t| match_refs(x.2.clone()) ).collect();

                                // filter right to have only ids match siteRefs in left
                               // let left_refs: Vec<String> = left.iter().flat_map(|x| match_refs(x.2.clone()) ).collect();
                           //    let left_refs: Vec<Token> = left.iter().map(|t| match_refs(x.2.clone()) ).collect();
                            //    let filtered: RefTags = right.iter().filter(|x| site_refs.contains(&x.0) ).cloned().collect();

                                // Must now get all tags for this ref to return


                             //   println!("siteRefs: {:?}", site_refs);

                                //if left[0].2 == right[0]

                                // let mut merged = left.clone();
                                // merged.extend(right);

                            //    println!("filtered: {:?}", filtered);

                             //   filtered
                         */
                        
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

                                // Get all the ids that are not in x

                                //let haystack_tags_for_ref: &HaystackTags = haystack_ref_name_store.get(&first_ref).expect("Expected Id");





                                // let notgated: Vec<Token> = haystack_ref_name_store.keys().filter(|k| !x.contains(k) ).cloned().collect();
                     

                                // println!("x: {:?}", x);

                                // notgated


                                vec![]


                                //let mut haystack_ref_name_store: HashMap<Token, HaystackTags> = HashMap::new();


                              //  let haystack_tags = haystack_tag_name_store.get(tag_name).unwrap_or(&vec![]).clone();

                                //haystack_tag_name_store.get(tag_name).unwrap_or(&vec![]).clone())

                                // // The vec after not should all have the same haystack key
                                // // for example below they all have elec

                                // if haystack_ref_name_store.len() == 0 {
                                //     return vec![];
                                // }

                            //    let first_ref = x[0].clone();

                            //    let haystack_tags_for_ref: &HaystackTags = haystack_ref_name_store.get(&first_ref).expect("Expected Id");

                            //    let key = haystack_tags_for_ref[0].0.clone();
      
                            //    println!("ket: {:?}", key);

                            //     let mut merged: Vec<Token> = vec![];

                                // for (k, v) in haystack_tag_name_store.iter() {
                                //     if *k != key {
                                //         merged.extend(v.clone());
                                //     }
                                // }

                                // for (id, _) in haystack_ref_name_store.iter() {
                                //     if *id != key {
                                //         merged.extend(v.clone());
                                //     }
                                // }

                             //   merged
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
            // FilterToken::Val(t) => {

            //     match &t {
            //         Token::EscapedString(val) => {

            //         },
            //         _ => {
            //             return Err(FilterError::EvalError("Unimplemented binary types".to_string()));
            //         }
            //     }
            // },
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

    macro_rules! refs {
        ( $( $x:expr ),* ) => {
            {
                let mut temp_vec = Vec::new();
                $(
                    temp_vec.push(Token::Ref($x.to_string(), None));
                )*
                StackValue::Refs(temp_vec)
            }
        };
    }

    #[test]
    fn test_eval() {

        // pub type RefTag = (String, String, Token);
        // pub type RefTags = Vec<RefTag>;

        // In the real world the idea is to get all the refs with tags from a db or whatever.
        // Maybe inefficient but will do for now.
        fn get_tags() -> RefTags {

            vec![

                (Token::Ref("@1".to_string(), None), vec![Tag::new_string("dis", "One"), Tag::new_string("elec", "elec"), Tag::new_string("heat", "heat"),
                                                          Tag::new_string("water", "water"), Tag::new_string("geoCity", "geoCity")]),

                (Token::Ref("@2".to_string(), None), vec![Tag::new_string("dis", "Two")]),

                (Token::Ref("@3".to_string(), None), vec![Tag::new_string("dis", "Three"), Tag::new_string("elec", "elec"), Tag::new_string("heat", "heat"),
                                                          Tag::new_ref("siteRef", "@1")]),

                (Token::Ref("@4".to_string(), None), vec![Tag::new_string("dis", "Four"), Tag::new_string("heat", "heat"), Tag::new_string("geoCity", "London"),
                                                          Tag::new_ref("equipRef", "@7")]),

                (Token::Ref("@5".to_string(), None), vec![Tag::new_string("dis", "Five"), Tag::new_string("elec", "elec"), Tag::new_string("heat", "heat"),
                                                          Tag::new_string("water", "water"), Tag::new_ref("siteRef", "@2")]),

                (Token::Ref("@6".to_string(), None), vec![Tag::new_string("dis", "Six"), Tag::new_ref("siteRef", "@4")]),
            
                (Token::Ref("@7".to_string(), None), vec![Tag::new_string("dis", "Seven")])
             
            ]
        }

    
        // assert_eq!(filter_eval_str("siteRef", &get_tags), Ok(refs!("@3", "@5", "@6")));
        // assert_eq!(filter_eval_str("siteRef->dis", &get_tags), Ok(refs!("@1", "@2", "@4")));
        // assert_eq!(filter_eval_str("siteRef->heat", &get_tags), Ok(refs!("@1", "@4")));
        // assert_eq!(filter_eval_str("siteRef->elec->dis", &get_tags), Ok(refs!()));
        // assert_eq!(filter_eval_str("siteRef->equipRef->dis", &get_tags), Ok(refs!()));
      
        //
        //
        // Entity has siteRef Tag that points to entity With equipRef which points to entity with dis tag
        // @6  -> @4 -> @7  // Should be 6
        println!("{:?}", filter_eval_str("siteRef->equipRef->dis", &get_tags));

        // println!("{:?}", filter_eval_str("elec and heat", &get_tags));
        // println!("{:?}", filter_eval_str("elec or heat", &get_tags));
        // println!("{:?}", filter_eval_str("not elec", &get_tags));
        // println!("{:?}", filter_eval_str("not elec and water", &get_tags));
        // println!("{:?}", filter_eval_str("not elec and heat", &get_tags));
        // println!("{:?}", filter_eval_str("elec and siteRef->geoCity", &get_tags));
        // println!("\n\n{:?}", filter_eval_str("elec and siteRef->geoCity == \"Chicago\"", &get_tags));
    }
}
