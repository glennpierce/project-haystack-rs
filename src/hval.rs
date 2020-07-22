
use std::fmt;

pub trait HVal: fmt::Debug + fmt::Display
{
    fn clone_dyn(&self) -> Box<dyn HVal>;

    // Encode value to zinc format
    fn to_zinc(&self) -> String ;

    // Encode to JSON string value
    fn to_json(&self) -> String ;
}