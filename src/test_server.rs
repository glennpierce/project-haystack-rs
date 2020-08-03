extern crate libproject_haystack_rs;

use libproject_haystack_rs::server;

use parking_lot::RwLock;
use std::sync::Arc;
use std::collections::{HashMap};

use libproject_haystack_rs::error::*;
use libproject_haystack_rs::server::UserAuthStore;

#[derive(Clone, Debug)]
struct DemoAuthDetails {
    pub handshake_token: String,
    pub username: String,
    pub server_salt: Option<String>,
    temp_store: HashMap<String, String>,
}

impl DemoAuthDetails {
    fn new() -> Self {
        DemoAuthDetails {
            handshake_token: libproject_haystack_rs::server::get_hanshake_token(),
            username: "user".to_string(),
            server_salt: None,
            temp_store: HashMap::new(),
        }
    }
}

impl UserAuthStore for DemoAuthDetails {
    fn get_handshake_token(&self, username: &str) -> HaystackResult<String> {
        Ok(self.handshake_token.clone())
    }

    fn get_username(&self, handshake_token: &str) -> HaystackResult<String> {
        // fn get_handshake_token_for_haystack_username(&self, username: &str) -> Result<String, BmosError> {
        Ok("user".into())
    }

    fn get_password_salt(&self) -> HaystackResult<String> {
        Ok("G2GXvHuTWUC3OZOmtNa2R3f4g1/GWA==".to_string())
    }

    fn get_salted_password(&self) -> HaystackResult<String> {
        Ok("vN9cNN6WxRTOGsaylAvv9upaVPw7j/ODkZUvQnpbCp4=".to_string())
    }

    fn set_temporary_value(&mut self, k: &str, v: &str) -> HaystackResult<()> {
        self.temp_store.insert(k.to_string(), v.to_string());
        Ok(())
    }

    fn get_temporary_value(&self,  k: &str) -> HaystackResult<Option<&String>> {
        Ok(self.temp_store.get(k))
    }
}


#[tokio::main]
async fn main() {

    pretty_env_logger::init();
    
    println!("Starting Test Server");

    // password: pencil, salt: G2GXvHuTWUC3OZOmtNa2R3f4g1/GWA==, iterations: 10000, salted_password: vN9cNN6WxRTOGsaylAvv9upaVPw7j/ODkZUvQnpbCp4=

    let store = Arc::new(RwLock::new(Box::new(DemoAuthDetails::new()) as Box<dyn UserAuthStore>));

    server::serve(store.clone()).await;
}

