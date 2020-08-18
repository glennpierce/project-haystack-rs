
use std::fmt;

use downcast_rs;

pub trait HVal: fmt::Debug + fmt::Display + Send + downcast_rs::Downcast
{
    fn clone_dyn(&self) -> Box<dyn HVal>;

    fn type_name(&self) -> String ;

    // fn cast_to_type_name_ref<T>(&self) -> Option<&T> {
    //     self.downcast_ref::<T>()
    // }  
    
    // Encode value to zinc format
    fn to_zinc(&self) -> String ;

    // Encode to JSON string value
    fn to_json(&self) -> String ;
}

impl_downcast!(HVal);