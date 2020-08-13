use chrono::{DateTime, NaiveDate, FixedOffset, Utc};

use std::fmt;
use std::f64;

use std::collections::BTreeMap;

use crate::hval::{HVal};

/// An error reported by the parser.
#[derive(Debug, Clone)]
pub enum TokenParseError {
    /// A token that is not allowed at the given location (contains the location of the offending
    /// character in the source string).
    UnexpectedToken(usize),

    UnexpectedStrToken(String),
    /// Missing right parentheses at the end of the source string (contains the number of missing
    /// parens).
    MissingRParen(i32),
    /// Missing operator or function argument at the end of the expression.
    MissingArgument,

    UnknownError
}

impl fmt::Display for TokenParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &*self {
            TokenParseError::UnexpectedToken(i) => write!(f, "Unexpected token at byte {}.", i),
            TokenParseError::UnexpectedStrToken(s) => write!(f, "Unexpected token {}.", s),
            TokenParseError::MissingRParen(i) => write!(
                f,
                "Missing {} right parenthes{}.",
                i,
                if *i == 1 { "is" } else { "es" }
            ),
            TokenParseError::MissingArgument => write!(f, "Missing argument at the end of expression."),
            TokenParseError::UnknownError => write!(f, "Unknown pass error."),
        }
    }
}

impl std::error::Error for TokenParseError {
    fn description(&self) -> &str {
        match *self {
            TokenParseError::UnexpectedToken(_) => "unexpected token",
            TokenParseError::UnexpectedStrToken(_) => "Unexpected token",
            TokenParseError::MissingRParen(_) => "missing right parenthesis",
            TokenParseError::MissingArgument => "missing argument",
            TokenParseError::UnknownError => "unknown error",
        }
    }
}

/// Expression tokens.
#[derive(Debug, PartialEq, Clone)]
pub enum Token {

    Empty, 

    Null, 

    Marker,

    Remove,

    NL,

    NA,

    Bool(bool),

    Inf,

    InfNeg,

    NaN,

    Comma,

    /// A number and units
    Number(f64, String),
    /// A variable.
    Id(String),

    Ref(String, Option<String>),

    EscapedString(String),

    Date(NaiveDate),

    //Time(String),

    DateTime(DateTime<FixedOffset>),

    Uri(String),

    Ver(String),
}


////////////////////////////////////////////////////
/// 

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &*self {
            Token::Empty => write!(f, ""),
            Token::Null => write!(f, "N"),
            Token::Marker => write!(f, "M"),
            Token::Remove => write!(f, "R"),
            Token::NL => write!(f, "\n"),
            Token::NA => write!(f, "NA"),
            Token::Bool(b) => write!(f, "{}", if *b { "true" } else { "false" }),
            Token::Inf => write!(f, "Inf"),
            Token::InfNeg => write!(f, "-Inf"),
            Token::NaN => write!(f, "NaN"),
            Token::Comma => write!(f, ","),
            Token::Number(num, units) => write!(f, "{}{}", num, units),
            Token::Id(val) => write!(f, "{}", val),
            
            Token::Ref(val, display) => {
                if display.is_some() {
                    return write!(f, "{}{}", val, display.clone().unwrap());
                }
                else {
                    return write!(f, "{}", val);
                }
            },

            Token::EscapedString(val) => write!(f, "{}", val),
        
            Token::Date(val) => write!(f, "{}", val.format("%Y-%m-%d")),

            Token::DateTime(val) => {
                let utc: DateTime<Utc> = val.with_timezone(&Utc);
                write!(f, "{}", utc.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
            }
            
            Token::Uri(val) => write!(f, "{}", val),
            Token::Ver(val) => write!(f, "{}", val),
        }
    }
}

impl HVal for Token {

    fn clone_dyn(&self) -> Box<dyn HVal> {
        Box::new(self.clone()) as Box<dyn HVal>
    }

   fn to_zinc(&self) -> String  {
        let result = match &*self {
            Token::Empty => "".to_string(),
            Token::Null => "N".to_string(),
            Token::Marker => "M".to_string(),
            Token::Remove => "R".to_string(),
            Token::NA => "NA".to_string(),
            Token::NL => "\n".to_string(),
            Token::Bool(b) => if *b { "T".to_string() } else { "F".to_string() },
            Token::Inf => "Inf".to_string(),
            Token::InfNeg => "-Inf".to_string(),
            Token::NaN => "NaN".to_string(),
            Token::Comma => ",".to_string(),
            Token::Number(num, units) => format!("{}{}", num, units),
            Token::Id(val) => format!("{}", val),
            
            Token::Ref(val, display) => {
                if display.is_some() {
                    return format!("@{} {}", val, display.clone().unwrap());
                }
        
                return format!("@{}", val);
            },

            Token::EscapedString(val) => format!("\"{}\"", val.escape_debug()),
        
            Token::Date(val) => format!("{}", val.format("%Y-%m-%d")),

            Token::DateTime(val) => {

                // DateTime: 2010-03-11T23:55:00-05:00 New_York or 2009-11-09T15:39:00Z
                // Haystack-rs always returns in Utc
                let utc: DateTime<Utc> = val.with_timezone(&Utc);
                return format!("{}", utc.format("%Y-%m-%dT%H:%M:%S%.3fZ"))
            }
            
            Token::Uri(val) => format!("`{}`", val),
            Token::Ver(val) => format!("ver:\"{}\"", val),
        };

        result
    }

    fn to_json(&self) -> String  {
        return  "".to_string();
    }
}

pub struct Val {
    pub hval: Box<dyn HVal>,
}

impl Val {
    pub fn new(t: Box<dyn HVal>) -> Self {

        Val {
            hval: t,
        }
    }
}

impl Clone for Val {
    fn clone(&self) -> Self {
        Val::new(self.hval.clone_dyn())
    }
}

impl fmt::Debug for Val {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.hval)
    }
}

impl fmt::Display for Val {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.hval)
    }
}

impl HVal for Val {

    fn clone_dyn(&self) -> Box<dyn HVal> {
        Box::new(Val::new(self.hval.clone_dyn())) as Box<dyn HVal>
    }

    fn to_zinc(&self) -> String  {
        format!("{}", self.hval.to_zinc())
    }
 
    fn to_json(&self) -> String  {
        return  "".to_string();
    }
}


// ////////////////////////////////////////////////
// ///
pub struct Scaler {
    pub token: Token,
}

impl Scaler {
    pub fn new(t: Token) -> Self {

        Scaler {
            token: t,
        }
    }
}

impl fmt::Debug for Scaler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.token)
    }
}

impl fmt::Display for Scaler {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.token)
    }
}

impl HVal for Scaler {

    fn clone_dyn(&self) -> Box<dyn HVal> {

        Box::new(Scaler::new(self.token.clone())) as Box<dyn HVal>
    }

    fn to_zinc(&self) -> String  {
        format!("{:?}", self)
    }
 
    fn to_json(&self) -> String  {
        return  "".to_string();
    }
}


////////////////////////////////
// ///
pub struct Comma {
    //pub token: Token,
}

impl Comma {
    pub fn new() -> Self {

        Comma {}
    }
}

impl fmt::Debug for Comma {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ",")
    }
}

impl fmt::Display for Comma {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, ",")
    }
}

impl HVal for Comma {

    fn clone_dyn(&self) -> Box<dyn HVal> {
        Box::new(Comma::new()) as Box<dyn HVal>
    }

    fn to_zinc(&self) -> String  {
        ",".into()
    }
 
    fn to_json(&self) -> String  {
        ",".into()
    }
}

// ///////////////////////////////////
/// 
#[derive(PartialEq, Clone)]
pub struct Tag {
    pub ident: Token,
    pub value: Option<Token>
}

impl Tag {
    pub fn new(ident: Token, value: Option<Token>) -> Self {

        match &ident {
            Token::Id(id) => (),
            _ => assert!(true),
        };

        Tag {
            ident: ident.clone(),
            value: value.clone(),
        }
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tag({:?}, {:?})", self.ident, self.value)
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.value.is_some() {
            write!(f, "{} {}", self.ident, self.value.as_ref().unwrap())
        }
        else {
            write!(f, "{}", self.ident)
        }
    }
}

impl HVal for Tag {

    fn clone_dyn(&self) -> Box<dyn HVal> {
        Box::new(Tag::new(self.ident.clone(), self.value.clone())) as Box<dyn HVal>
    }

    fn to_zinc(&self) -> String  {
        if self.value.is_some() {
            format!("{}:{}", self.ident.to_zinc(), self.value.clone().unwrap().to_zinc())
        }
        else {
            format!("{}", self.ident.to_zinc())
        }
    }
 
    fn to_json(&self) -> String  {
        return  "".to_string();
    }
}

////////////////////////////////////
#[derive(PartialEq, Clone)]
pub struct Tags {
    tags: Vec<Tag>,
}

impl Tags {
    pub fn new(tags: &Vec<Tag>) -> Self {

        Tags {
            tags: tags.clone(),
        }
    }

    // pub fn new_from_tokens(tokens: &Vec<(Token, Option<Token>)>) -> Self {
    //     Tags {
    //         tags: tokens.into_iter().map(|x| Tag::new(&x.0, x.1.clone())).collect(),
    //     }
    // }
}

impl fmt::Debug for Tags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

    //     self.tags.sort_by(|t1, t2| { 

    //         let s1 = match &t1.ident {
    //             Token::Id(id) => id.to_string(),
    //             _ => "".into(),
    //         };

    //         let s2 = match &t2.ident {
    //             Token::Id(id) => id.to_string(),
    //             _ => "".into(),
    //         };

    //         s1.cmp(&s2)
    //     }
    // );

        write!(f, "{:?}", self.tags)
    }
}

impl fmt::Display for Tags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.tags)
    }
}

impl HVal for Tags {

    fn clone_dyn(&self) -> Box<dyn HVal> {
        let tmp: Vec<Tag> = self.tags.clone().into_iter().map(|t| t.clone()).collect();
        Box::new(Tags::new(&tmp)) as Box<dyn HVal>
    }

    fn to_zinc(&self) -> String  {
        let s = self.tags.iter().map(|t: &Tag| {

            format!("{}", t.clone().to_zinc())

        }).collect::<Vec<String>>().join(" ");

        format!("{}", s)
    }
 
    fn to_json(&self) -> String  {
        return  "".to_string();
    }
}

/////////////////////////////////////

#[derive(PartialEq, Clone)]
pub struct Dict {
    map: BTreeMap<String, Option<Tag>>,
}

impl Dict {
    pub fn new(tags: &Vec<Tag>) -> Self {

        let tmp = tags.clone();

        tmp.clone().sort_by(|t1, t2| { 

                let s1 = match &t1.ident {
                    Token::Id(id) => id.to_string(),
                    _ => "".into(),
                };

                let s2 = match &t2.ident {
                    Token::Id(id) => id.to_string(),
                    _ => "".into(),
                };

                s1.cmp(&s2)
            }
        );

        let mut m: BTreeMap<String, Option<Tag>> = BTreeMap::new();
        for t in tmp { 
            if t.value.is_none() {
                m.insert(t.ident.to_string(), None);
            }
            else {
                m.insert(t.ident.to_string(), Some(t.clone()));
            }
        }

        Dict{map: m}
    }

    pub fn new_from_tags(tags: &Tags) -> Self {
        Dict::new(&tags.tags)
    }
}

impl fmt::Debug for Dict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Dict({:?})", self.map)
    }
}

impl fmt::Display for Dict {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Dict({:?})", self.map)
    }
}

impl HVal for Dict {

    fn clone_dyn(&self) -> Box<dyn HVal> {
        Box::new(self.clone()) as Box<dyn HVal>
    }

    fn to_zinc(&self) -> String  {

        let s = self.map.iter().map(|t: (&String, &Option<Tag>)| {

            if t.1.is_some() {
                format!("{}", t.1.clone().unwrap().to_zinc())
            }
            else {
                format!("{}", t.0.clone())
            }

        }).collect::<Vec<String>>().join(" ");

        format!("{}", s)
    }
 
    fn to_json(&self) -> String  {
        return  "".to_string();
    }
}

////////////////////////////////

pub struct List {
    vals: Vec<Val>,
}

impl List {
    pub fn new(vals: Vec<Val>) -> Self {
        List{vals: vals}
    }
}

impl fmt::Debug for List {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "List({:?})", self.vals)
    }
}

impl fmt::Display for List {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "List({:?})", self.vals)
    }
}

impl HVal for List {

    fn clone_dyn(&self) -> Box<dyn HVal> {
        let tmp: Vec<Val> = self.vals.clone().into_iter().map(|v| v.clone()).collect();
        Box::new(List::new(tmp)) as Box<dyn HVal>
    }

    fn to_zinc(&self) -> String  {
        let s = self.vals.iter().map(|v: &Val| {

            format!("{}", v.clone().to_zinc())

        }).collect::<Vec<String>>().join(",");

        format!("[{}]", s)
    }
 
    fn to_json(&self) -> String  {
        return  "".to_string();
    }
}

// Col(Box<Token>, Box<Vec<Token>>),

#[derive(PartialEq, Clone)]
pub struct Col {
    pub id: Token,
    pub tags: Option<Tags>
}

impl Col {
    pub fn new(id: Token, tags: Option<Tags>) -> Self {

        match &id {
            Token::Id(s) => (),
            _ => assert!(true),
        };

        Col {
            id: id.clone(),
            tags: tags.clone(),
        }
    }
}

impl fmt::Debug for Col {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Col({:?}, {:?})", self.id, self.tags)
    }
}

impl fmt::Display for Col {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Col({}, {:?})", self.id, self.tags)
    }
}

impl HVal for Col {

    fn clone_dyn(&self) -> Box<dyn HVal> {
        Box::new(Col::new(self.id.clone(), self.tags.clone())) as Box<dyn HVal>
    }

    fn to_zinc(&self) -> String  {
        if self.tags.is_some() {
            format!("{}:{}", self.id.to_zinc(), self.tags.clone().unwrap().to_zinc())
        }
        else {
            format!("{}", self.id.to_zinc())
        }
    }
 
    fn to_json(&self) -> String  {
        return  "".to_string();
    }
}

////////////////////////////////////

#[derive(PartialEq, Clone)]
pub struct Cols {
    cols: Vec<Col>,
}

impl Cols {
    pub fn new(cols: Vec<Col>) -> Self {
        Cols {
            cols: cols.clone(),
        }
    }
}

impl fmt::Debug for Cols {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cols({:?})", self.cols)
    }
}

impl fmt::Display for Cols {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Cols({:?})", self.cols)
    }
}


impl HVal for Cols {

    fn clone_dyn(&self) -> Box<dyn HVal> {
        let tmp: Vec<Col> = self.clone().cols.into_iter().map(|v| v.clone()).collect();
        Box::new(Cols::new(tmp)) as Box<dyn HVal>
    }

    fn to_zinc(&self) -> String  {
     
        let s = self.cols.iter().map(|c: &Col| {

            format!("{}", c.clone().to_zinc())

        }).collect::<Vec<String>>().join(",");

        format!("{}", s)
    }
 
    fn to_json(&self) -> String  {
        return  "".to_string();
    }
}


//////////////////////////////////////////////////////////////
/// <cell> ["," <cell>]* <nl>
//#[derive(PartialEq)]
pub struct Row {
    pub cells: Vec<Val>,
}

impl Row {
    pub fn new(cells: Vec<Val>) -> Self {

        Row {
            cells: cells,
        }
    }
}

impl Clone for Row {
    fn clone(&self) -> Self {
        Row::new(self.cells.clone())
    }
}

impl fmt::Debug for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Row({:?})", self.cells)
    }
}

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Row({:?})", self.cells)
    }
}

impl HVal for Row {

    fn clone_dyn(&self) -> Box<dyn HVal> {
        // let tmp: Vec<Val> = self.cells.into_iter().map(|v| v.clone()).collect();
        // Box::new(Row::new(tmp)) as Box<dyn HVal>

        Box::new(self.clone()) as Box<dyn HVal>
    }

    fn to_zinc(&self) -> String  {
        let s = self.cells.iter().map(|v: &Val| {

            format!("{}", v.clone().to_zinc())

        }).collect::<Vec<String>>().join(",");

        format!("{}", s)
    }
 
    fn to_json(&self) -> String  {
        return  "".to_string();
    }
}


////////////////////////////////////

#[derive(Clone)]
pub struct Rows {
    rows: Vec<Row>,
}

impl Rows {
    pub fn new(rows: Vec<Row>) -> Self {
        Rows {
            rows: rows,
        }
    }
}

impl fmt::Debug for Rows {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rows({:?})", self.rows)
    }
}

impl fmt::Display for Rows {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Rows({:?})", self.rows)
    }
}


impl HVal for Rows {

    fn clone_dyn(&self) -> Box<dyn HVal> {
        let tmp: Vec<Row> = self.rows.clone().into_iter().map(|r| r.clone()).collect();
        Box::new(Rows::new(tmp)) as Box<dyn HVal>
    }

    fn to_zinc(&self) -> String  {
        let s = self.rows.iter().map(|r: &Row| {

            format!("{}", r.clone().to_zinc())

        }).collect::<Vec<String>>().join("\n");

        format!("{}", s)
    }
 
    fn to_json(&self) -> String  {
        return  "".to_string();
    }
}

//////////////////////////////////
/// 
/// // GridMeta(Box<Token>, Option<Tags>), 

#[derive(Clone)]
pub struct GridMeta {
    pub version: Token,
    pub metadata: Option<Tags>
}

impl GridMeta {
    pub fn new(version: Token, metadata: Option<Tags>) -> Self {

        match &version {
            Token::Ver(id) => (),
            _ => assert!(true),
        };

        GridMeta {
            version: version.clone(),
            metadata: metadata.clone(),
        }
    }
}

impl fmt::Debug for GridMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GridMeta({:?}, {:?})", self.version, self.metadata)
    }
}

impl fmt::Display for GridMeta {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "GridMeta({:?}, {:?})", self.version, self.metadata)
    }
}

impl HVal for GridMeta {

    fn clone_dyn(&self) -> Box<dyn HVal> {
        Box::new(self.clone()) as Box<dyn HVal>
    }

    fn to_zinc(&self) -> String  {
        if self.metadata.is_some() {
            format!("{} {}", self.version.to_zinc(), self.metadata.clone().unwrap().to_zinc())
        }
        else {
            format!("{}", self.version.to_zinc())
        }
    }
 
    fn to_json(&self) -> String  {
        return  "".to_string();
    }
}


#[derive(Clone)]
pub struct Grid {
    pub grid_meta: GridMeta,
    pub cols: Cols,
    pub rows: Rows,
}

impl Grid {
    pub fn new(grid_meta: GridMeta, cols: Cols, rows: Rows) -> Self {

        Grid {
            grid_meta: grid_meta,
            cols: cols,
            rows: rows,
        }
    }

    // Empty grid with one column called "empty" and zero rows
    pub fn empty() -> Self {
        Grid {
            grid_meta: GridMeta::new(Token::Ver("3.0".into()), None),
            cols: Cols::new(vec![Col::new(Token::Id("empty".into()), None)]),
            rows: Rows::new(vec![]),
        }
    }
}

impl fmt::Debug for Grid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Grid({:?}, {:?}, {:?})", self.grid_meta, self.cols, self.rows)
    }
}

impl fmt::Display for Grid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Grid({:?}, {:?}, {:?})", self.grid_meta, self.cols, self.rows)
    }
}

impl HVal for Grid {

    fn clone_dyn(&self) -> Box<dyn HVal> {
        Box::new(self.clone()) as Box<dyn HVal>
    }

    fn to_zinc(&self) -> String  {
        format!("{}\n{}\n{}", self.grid_meta.to_zinc(), self.cols.to_zinc(), self.rows.to_zinc())
    }
 
    fn to_json(&self) -> String  {
        return  "".to_string();
    }
}

//type EmpytTags = Box::new(vec![]);