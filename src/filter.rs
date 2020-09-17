
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

use itertools::{Itertools, EitherOrBoth};
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

pub type RefTag = (Token, Vec<Tag>);
pub type RefTags = Vec<RefTag>;

pub type HaystackTag = (String, Token);
pub type HaystackTags = Vec<HaystackTag>;

#[derive(Debug, Clone, PartialEq)]
pub enum StackValue {
    Token(Token),
    // Path((Vec<Token>, Vec<Token>))      // head, tails
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
    v.iter().filter(|&t| t.contains_ref_with_id(id_token)).peekable().peek().is_some()
}

pub fn find_ref_with_id(v: &Vec<Tag>, id_token: &Token) -> Option<Token>
{
    let result_option: Option<&Tag> = v.iter().find(|t| t.contains_ref_with_id(id_token));

    if result_option.is_none() {
        return None;
    }

    let result: &Tag = result_option.unwrap();

    result.get_value::<Token>()
}

fn ids_containing_tag(entities: &RefTags, token: &Token) -> Vec<Token> {
    entities.iter().filter(|&i| i.1.contains(&Tag::new_marker_from_token(token.clone()))).map(|x| x.0.clone()).collect()
}

fn refs_containing_tag(entities: &RefTags, token: &Token) ->  Vec<Token> {
    let mut refs: Vec<Token> = vec![];

    for e in entities.iter() {
        let t: Option<Token> = find_ref_with_id(&e.1, &token);
                      
        if t.is_some() {
            refs.push(t.unwrap().clone());
        }
    }

    refs
}

fn get_routes_for_path(values: &RefTags, tags: &Vec<Token>) -> Vec<Vec<(Token, Option<Token>)>> {

    let mut routes: Vec<Vec<(Token, Option<Token>)>> = vec![vec![]; tags.len()];

    for (index, tag) in tags.iter().enumerate() {

        let ids: Vec<Token> = ids_containing_tag(&values, tag);
        let refs: Vec<Option<Token>> = refs_containing_tag(&values, tag).iter().map(|x| Some(x.clone())).collect();

        let tmp: Vec<(Token, Option<Token>)>;
        
        let len_ids = ids.len();

        if refs.is_empty() {
            tmp = ids.into_iter().zip(vec![None; len_ids].into_iter()).collect();
        }
        else {
            tmp = ids.into_iter().zip(refs.into_iter()).collect();
        }

        if index > 0 {

            // Remove dead ends from previous route
            let current_ids: Vec<Token> = tmp.iter().map(|i| i.0.clone()).collect();
            routes[index-1] = routes[index-1].clone().into_iter().filter(|x| current_ids.contains(&x.1.clone().unwrap())).collect();
        }

        routes[index] = tmp;
    }

    routes
}

fn traverse_up_routes_removing_paths(original_routes: &Vec<Vec<(Token, Option<Token>)>>) -> Vec<Vec<(Token, Option<Token>)>> {

    let mut routes = original_routes.clone();
  
    for index in (1..routes.len()).rev() {
        let ids_in_route: Vec<Token> = original_routes[index].iter().map(|i| i.0.clone()).collect();
        routes[index-1] = original_routes[index-1].clone().into_iter().filter(|x| ids_in_route.contains(&x.1.clone().unwrap())).collect();
    }

    routes
}

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

    'rpn_loop: for token in &rpn {

        match token {

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

                let routes: Vec<Vec<(Token, Option<Token>)>> = get_routes_for_path(&values, &tags);

                // println!("routes: {:?}", routes);

                // ok we need to turn this into head/tail Path for the stack now
                // stack.push(StackValue::Path((routes[0].iter().map(|x|x.0.clone()).collect(),
                //                              routes[tags.len()-1].iter().map(|x|x.0.clone()).collect())));

                stack.push(StackValue::Refs(routes[0].iter().map(|x|x.0.clone()).collect()));
            },
            FilterToken::Compare(path, op, val) => {

                match **path {

                    FilterToken::Path(ref tags) => { 

                        println!("tags: {:?}", tags);

                        let routes: Vec<Vec<(Token, Option<Token>)>> = get_routes_for_path(&values, &tags);

                        match op {
                            Operation::Equals => {
                                // Check the leaf tokens for the comparison first.
                                // Then go up each level of routes removing possible routes to now missing leafs

                                println!("leafs: {:?}", routes[routes.len() - 1]);
                            },
                            _ => {
                                return Err(FilterError::EvalError("Unexpected comparison operation".to_string()));
                            }
                        }
                    }
                    _ => {
                        return Err(FilterError::EvalError("Unexpected type".to_string()));
                    }
                }

                println!("Compare {:?} {:?} {:?}", path, op, val);
            },
            FilterToken::Binary(op) => {

                let right_stack_value: StackValue = stack.pop().unwrap();
                let left_stack_value: StackValue = stack.pop().unwrap();
                
                match (left_stack_value.clone(), right_stack_value.clone()) {

                    // (StackValue::Path((lhs_head, lhs_tail)), StackValue::Path((rhs_head, rhs_tail))) => {
                    (StackValue::Refs(lhs), StackValue::Refs(rhs)) => {
                        let r: StackValue = match op {
                            Operation::And => {

                               let mut v = lhs.intersect_if(rhs, |l, r| l == r);
                               v.sort();
                               StackValue::Refs(v)
                            },
                            Operation::Or => {

                                let mut merged = lhs.clone();
                                merged.extend(rhs);

                                let hs = merged.iter().cloned().collect::<HashSet<Token>>();

                                let mut v: Vec<Token> = hs.into_iter().collect();
                                v.sort();
                                StackValue::Refs(v)
                            },

                            _ => {
                                return Err(FilterError::EvalError(format!(
                                    "Unimplemented binary operation: {:?}",
                                    op
                                )));
                            }
                        };

                        stack.push(r);
                    },

                    _ => {
                        return Err(FilterError::EvalError("Unimplemented binary types".to_string()));
                    }
       
                 };
            }
            FilterToken::Unary(op) => {

                let x_stack_value: StackValue = stack.pop().unwrap();
                match x_stack_value {

                    StackValue::Refs(refs) => {

                        let r = match op {
                       
                            Operation::Not => {

                                let v = values.iter().filter(|x| !refs.contains(&x.0)).map(|x| x.0.clone()).collect();
        
                                StackValue::Refs(v)
                            },

                            _ => {
                                return Err(FilterError::EvalError(format!(
                                    "Unimplemented binary operation: {:?}",
                                    op
                                )));
                            }
                        };

                        stack.push(r);
                    },

                    _ => {
                        return Err(FilterError::EvalError("Unimplemented binary types".to_string()));
                    }
       
                 };
            },
           
            _ => return Err(FilterError::EvalError(format!("Unrecognized token: {:?}", token))),
        }
    }

    let r: StackValue = stack.pop().expect("Stack is empty, this is impossible.");

    if !stack.is_empty() {

        println!("Stack: {:?}", stack);

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

    macro_rules! token_ref {
        ( $x:expr ) => {
            {
                Token::Ref($x.to_string(), None)
            }
        };
    }

    #[test]
    fn test_eval() {

        // In the real world the idea is to get all the refs with tags from a db or whatever.
        // Maybe inefficient but will do for now.
        fn get_tags() -> RefTags {

            vec![

                (Token::Ref("@1".to_string(), None), vec![Tag::new_string("dis", "One"), Tag::new_string("elec", "elec"), Tag::new_string("heat", "heat"),
                                                          Tag::new_string("water", "water"), Tag::new_string("geoCity", "geoCity"), Tag::new_ref("equipRef", "@2")]),

                (Token::Ref("@2".to_string(), None), vec![Tag::new_string("dis", "Two"), Tag::new_ref("pointRef", "@9")]),

                (Token::Ref("@3".to_string(), None), vec![Tag::new_string("dis", "Three"), Tag::new_string("elec", "elec"), Tag::new_string("heat", "heat"),
                                                          Tag::new_ref("siteRef", "@1")]),

                (Token::Ref("@4".to_string(), None), vec![Tag::new_string("dis", "Four"), Tag::new_string("heat", "heat"), Tag::new_string("geoCity", "London"),
                                                          Tag::new_ref("equipRef", "@7")]),

                (Token::Ref("@5".to_string(), None), vec![Tag::new_string("dis", "Five"), Tag::new_string("elec", "elec"), Tag::new_string("heat", "heat"),
                                                          Tag::new_string("water", "water"), Tag::new_ref("siteRef", "@2")]),

                (Token::Ref("@6".to_string(), None), vec![Tag::new_string("dis", "Six"), Tag::new_ref("siteRef", "@4")]),
            
                (Token::Ref("@7".to_string(), None), vec![Tag::new_string("dis", "Seven"),  Tag::new_ref("pointRef", "@8")]),
                
                (Token::Ref("@8".to_string(), None), vec![Tag::new_string("dis", "Eight")]),

                (Token::Ref("@9".to_string(), None), vec![Tag::new_string("dis", "Nine")]),

                (Token::Ref("@10".to_string(), None), vec![Tag::new_string("dis", "Ten"), Tag::new_ref("siteRef", "@11")]),

                (Token::Ref("@11".to_string(), None), vec![Tag::new_string("dis", "Eleven"), Tag::new_string("geoCounty", "Cornwall"),
                                                          Tag::new_ref("equipRef", "@7")]),
             
            ]
        }

        assert_eq!(filter_eval_str("siteRef", &get_tags), Ok(refs!("@3", "@5", "@6", "@10")));

        assert_eq!(filter_eval_str("siteRef->dis", &get_tags), Ok(refs!("@3", "@5", "@6", "@10")));

        assert_eq!(filter_eval_str("siteRef->heat", &get_tags), Ok(refs!("@3", "@6")));

        // Needs to fail
        // assert_eq!(filter_eval_str("siteRef->elec->dis", &get_tags),
        //     Ok(path!(refs!("@3", "@5", "@6", "@10"), refs!("@1", "@2", "@3", "@4", "@5", "@6", "@7", "@8", "@9", "@10", "@11"))));

        assert_eq!(filter_eval_str("siteRef->equipRef->dis", &get_tags), Ok(refs!("@3", "@6", "@10")));

        // Entity has siteRef Tag that points to entity With equipRef which points to entity with dis tag
        assert_eq!(filter_eval_str("siteRef->equipRef->pointRef->dis", &get_tags), Ok(refs!("@3", "@6", "@10")));
        
        assert_eq!(filter_eval_str("elec", &get_tags), Ok(refs!("@1", "@3", "@5")));
        assert_eq!(filter_eval_str("heat", &get_tags), Ok(refs!("@1", "@3", "@4", "@5")));
        assert_eq!(filter_eval_str("elec and heat", &get_tags), Ok(refs!("@1", "@3", "@5")));
        assert_eq!(filter_eval_str("elec or heat", &get_tags), Ok(refs!("@1", "@3", "@4", "@5")));
        assert_eq!(filter_eval_str("not elec", &get_tags), Ok(refs!("@2", "@4", "@6", "@7", "@8", "@9", "@10", "@11")));
        assert_eq!(filter_eval_str("not elec and water", &get_tags), Ok(refs!()));
        assert_eq!(filter_eval_str("not elec and heat", &get_tags), Ok(refs!("@4")));
        assert_eq!(filter_eval_str("siteRef->geoCity", &get_tags), Ok(refs!("@3", "@6")));

        let routes = vec![
                        vec![(token_ref!("@3"), Some(token_ref!("@1"))),
                             (token_ref!("@6"), Some(token_ref!("@4"))),
                             (token_ref!("@10"), Some(token_ref!("@11")))],
                        vec![(token_ref!("@1"), Some(token_ref!("@2"))),
                             (token_ref!("@4"), Some(token_ref!("@7"))),
                             (token_ref!("@11"), Some(token_ref!("@7")))],
                        vec![(token_ref!("@2"), Some(token_ref!("@9"))),
                             (token_ref!("@7"), Some(token_ref!("@8")))],
                        vec![(token_ref!("@1"), None),
                             (token_ref!("@2"), None),
                             (token_ref!("@3"), None),
                             (token_ref!("@4"), None),
                             (token_ref!("@5"), None),
                             (token_ref!("@6"), None),
                             (token_ref!("@7"), None),
                             (token_ref!("@8"), None),
                             (token_ref!("@9"), None),
                             (token_ref!("@10"), None),
                             (token_ref!("@11"), None)]];

        
        // let route1 = vec![(Token::Ref("@1".to_string(), None), Token::Ref("@11".to_string(), None))];

        println!("{:?}",  traverse_up_routes_removing_paths(&routes));


     //   println!("\n\n{:?}", filter_eval_str("elec and siteRef->geoCity == \"Chicago\"", &get_tags));
    }
}
