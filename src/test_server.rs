extern crate libproject_haystack_rs;

use libproject_haystack_rs::server;

use parking_lot::RwLock;
use std::sync::Arc;
use std::collections::{HashMap};

use libproject_haystack_rs::error::*;

#[tokio::main]
async fn main() {

    pretty_env_logger::init();
    
    println!("Starting Test Server");

    server::serve().await;
}

