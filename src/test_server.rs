extern crate libproject_haystack_rs;

use libproject_haystack_rs::server;

#[tokio::main]
async fn main() {

    pretty_env_logger::init();
    
    println!("Starting Test Server");

    server::serve().await;
}

