
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