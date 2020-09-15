
use std::io;
use std::fmt;
use std::num;
// use std::error::Error;

// macro_rules! fatal {
//     ($($tt:tt)*) => {{
//         use std::io::Write;
//         writeln!(&mut ::std::io::stderr(), $($tt)*).unwrap();
//         ::std::process::exit(1)
//     }}
// }



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


/// An error produced during parsing or evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterError {
    UnknownVariable(String),
    UnknownAlias(String),

    /// An error returned by the parser.
    ParseError(FilterTokenParseError),
    /// The shunting-yard algorithm returned an error.
    RPNError(RPNError),
    // A catch all for all other errors during evaluation
    EvalError(String),
}

impl fmt::Display for FilterError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FilterError::UnknownVariable(ref name) => {
                write!(f, "Evaluation error: unknown variable `{}`.", name)
            }
            FilterError::UnknownAlias(ref name) => {
                write!(f, "Evaluation error: unknown alias `{}`.", name)
            }
            FilterError::ParseError(ref e) => {
                write!(f, "Parse error: ")?;
                e.fmt(f)
            }
            FilterError::RPNError(ref e) => {
                write!(f, "RPN error: ")?;
                e.fmt(f)
            }
            FilterError::EvalError(ref e) => {
                write!(f, "Eval error: ")?;
                e.fmt(f)
            }
        }
    }
}

impl From<FilterTokenParseError> for FilterError {
    fn from(err: FilterTokenParseError) -> FilterError {
        FilterError::ParseError(err)
    }
}

impl From<RPNError> for FilterError {
    fn from(err: RPNError) -> FilterError {
        FilterError::RPNError(err)
    }
}

impl std::error::Error for FilterError {
    fn description(&self) -> &str {
        match *self {
            FilterError::UnknownVariable(_) => "unknown variable",
            FilterError::UnknownAlias(_) => "unknown alias",
            FilterError::EvalError(_) => "eval error",
            FilterError::ParseError(ref e) => e.description(),
            FilterError::RPNError(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        match *self {
            FilterError::ParseError(ref e) => Some(e),
            FilterError::RPNError(ref e) => Some(e),
            _ => None,
        }
    }
}


/// An error reported by the parser.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterTokenParseError {
    /// A token that is not allowed at the given location (contains the location of the offending
    /// character in the source string).
    UnexpectedToken(usize),

    UnexpectedStrToken(String),
    /// Missing right parentheses at the end of the source string (contains the number of missing
    /// parens).
    MissingRParen(i32),
    /// Missing operator or function argument at the end of the expression.
    MissingArgument,

    UnknownFilterTokenParseError
}

impl fmt::Display for FilterTokenParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &*self {
            FilterTokenParseError::UnexpectedToken(i) => write!(f, "Unexpected token at byte {}.", i),
            FilterTokenParseError::UnexpectedStrToken(s) => write!(f, "Unexpected token {}.", s),
            FilterTokenParseError::MissingRParen(i) => write!(
                f,
                "Missing {} right parenthes{}.",
                i,
                if *i == 1 { "is" } else { "es" }
            ),
            FilterTokenParseError::MissingArgument => write!(f, "Missing argument at the end of expression."),
            FilterTokenParseError::UnknownFilterTokenParseError => write!(f, "Unknown filter pass error."),
        }
    }
}

impl std::error::Error for FilterTokenParseError {
    fn description(&self) -> &str {
        match *self {
            FilterTokenParseError::UnexpectedToken(_) => "unexpected token",
            FilterTokenParseError::UnexpectedStrToken(_) => "Unexpected token",
            FilterTokenParseError::MissingRParen(_) => "missing right parenthesis",
            FilterTokenParseError::MissingArgument => "missing argument",
            FilterTokenParseError::UnknownFilterTokenParseError => "unknown filter error",
        }
    }
}


// We derive `Debug` because all types should probably derive `Debug`.
// This gives us a reasonable human readable description of `CliError` values.
#[derive(Debug)]
pub enum HaystackError {
    GeneralError(String),
    //ParseError(String),
    Io(std::io::Error),
    // ParseInt(std::num::ParseIntError),
    ParseBool(std::str::ParseBoolError),
    ParseFloat(std::num::ParseFloatError),
    //PostgresError(error: String),
    // NotFound,
    //ModBusError(modbus::Error),
    //SerdeError(serde_json::Error),
}

pub type HaystackResult<T> = std::result::Result<T, HaystackError>;

impl fmt::Display for HaystackError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            HaystackError::GeneralError(ref err) => err.fmt(f),
            //HaystackError::ParseError(ref err) => err.fmt(f),
            HaystackError::Io(ref err) => err.fmt(f),
            // HaystackError::ParseInt(ref err) => err.fmt(f),
            HaystackError::ParseFloat(ref err) => err.fmt(f),
            HaystackError::ParseBool(ref err) => err.fmt(f),
            // HaystackError::NotFound => write!(f, "No matching cities with a \
            //                                 population were found."),
            //HaystackError::ModBusError(ref err) => err.fmt(f),
            //HaystackError::SerdeError(ref err) => err.fmt(f),
        }
    }
}

// impl Error for HaystackError {
//     fn description(&self) -> &str {
//         match *self {
//             HaystackError::GeneralError(ref err) => err,
//             HaystackError::Io(ref err) => err.description(),
//             // HaystackError::ParseInt(ref err) => err.description(),
//             HaystackError::ParseFloat(ref err) => err.description(),
//             // HaystackError::NotFound => "not found",
//             HaystackError::ModBusError(ref err) => err.description(),
//         }
//     }
// }

impl From<&str> for HaystackError {
    fn from(err:&str) -> HaystackError {
        HaystackError::GeneralError(err.to_string())
    }
}

impl From<io::Error> for HaystackError {
    fn from(err: io::Error) -> HaystackError {
        HaystackError::Io(err)
    }
}

impl From<num::ParseFloatError> for HaystackError {
    fn from(err: num::ParseFloatError) -> HaystackError {
        HaystackError::ParseFloat(err)
    }
}

impl From<std::str::ParseBoolError> for HaystackError {
    fn from(err: std::str::ParseBoolError) -> HaystackError {
        HaystackError::ParseBool(err)
    }
}

// impl From<modbus::Error> for HaystackError {
//     fn from(err: modbus::Error) -> HaystackError {
//         HaystackError::ModBusError(err)
//     }
// }

// impl From<serde_json::Error> for HaystackError {
//     fn from(err: serde_json::Error) -> HaystackError {
//         HaystackError::SerdeError(err)
//     }
// }