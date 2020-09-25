extern crate libproject_haystack_rs;

use data_encoding::{BASE64};

fn main() {

    pretty_env_logger::init();

    let salt: Vec<u8> = libproject_haystack_rs::server::get_salt();

    println!("salt: {:?}", &salt);

    let salt_base64 = BASE64.encode(&salt);

    let salted_password: Vec<u8> = libproject_haystack_rs::server::haystack_generate_salted_password("pencil", salt_base64.clone().as_bytes(), 10000);

    println!("salt: {:?}", &salted_password);

    println!("password: pencil, salt: {}, iterations: 10000, salted_password: {}", salt_base64, BASE64.encode(&salted_password));
}

