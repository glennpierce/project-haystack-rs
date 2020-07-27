#[macro_use]
extern crate lazy_static;

extern crate bytes;

extern crate nom_unicode;

#[macro_use]
extern crate log;

extern crate data_encoding;

extern crate rand;

// extern crate openssl;

extern crate stringprep;

extern crate ring;

extern crate escape8259;

pub mod error;
pub mod token;
pub mod hval;
pub mod zinc_tokenizer;
pub mod server;


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {

        assert_eq!(2 + 2, 4);
    }
}
