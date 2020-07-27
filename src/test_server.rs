extern crate libproject_haystack_rs;

use libproject_haystack_rs::server;

use parking_lot::RwLock;
use std::sync::Arc;

use libproject_haystack_rs::error::*;
use libproject_haystack_rs::server::UserAuthStore;

#[derive(Clone, Debug)]
struct DemoAuthDetails {
    pub handshake_token: String,
    pub username: String,
    pub auth_token: Option<String>,
    pub client_nonce: Option<String>,
    pub server_salt: Option<String>,
    pub client_first_message: Option<String>,
    pub server_first_message: Option<String>,
    pub client_final_no_pf: Option<String>,
}

impl DemoAuthDetails {
    fn new() -> Self {
        DemoAuthDetails {
            handshake_token: libproject_haystack_rs::server::get_hanshake_token(),
            username: "user".to_string(),
            auth_token: None,
            client_nonce: None,
            server_salt: None,
            client_first_message: None,
            server_first_message: None,
            client_final_no_pf: None,
        }
    }
}

impl UserAuthStore for DemoAuthDetails {
    fn get_handshake_token(&self, username: &str) -> HaystackResult<String> {
        Ok(self.handshake_token.clone())
    }

    fn get_username(&self, handshake_token: &str) -> HaystackResult<String> {
        Ok("user".into())
    }

    // fn get_password(handshake_token: &str) -> String;
 
    fn set_client_final_no_pf(&mut self, s: Option<String>) -> HaystackResult<()> {
        self.client_final_no_pf = s;
        Ok(())
    }

    fn set_client_first_message(&mut self, s: Option<String>) -> HaystackResult<()> {
        self.client_first_message = s;
        Ok(())
    }

    fn set_server_salt(&mut self, s: Option<String>) -> HaystackResult<()> {
        self.server_salt = s;
        Ok(())
    }

    fn set_client_nonce(&mut self, s: Option<String>) -> HaystackResult<()> {
        self.client_nonce = s;
        Ok(())
    }

    fn set_server_first_message(&mut self, s: Option<String>) -> HaystackResult<()> {
        self.server_first_message = s;
        Ok(())
    }

    fn set_authtoken(&mut self, s: Option<String>) -> HaystackResult<()> {
        self.auth_token = s;
        Ok(())
    }

    fn get_server_salt(&self) -> HaystackResult<Option<String>> {
        Ok(self.server_salt.clone())
    }

    fn get_client_nonce(&self) -> HaystackResult<Option<String>> {
        Ok(self.client_nonce.clone())
    }

    fn get_client_first_message(&self) -> HaystackResult<Option<String>> {
        Ok(self.client_first_message.clone())
    }

    fn get_server_first_message(&self) -> HaystackResult<Option<String>> {
        Ok(self.server_first_message.clone())
    }

    fn get_client_final_no_pf(&self) -> HaystackResult<Option<String>> {
        Ok(self.client_final_no_pf.clone())
    }

    fn get_authtoken(&self) -> HaystackResult<Option<String>> {
        Ok(self.auth_token.clone())
    }
}


#[tokio::main]
async fn main() {

    pretty_env_logger::init();
    
    println!("Starting Test Server");

    let store = Arc::new(RwLock::new(Box::new(DemoAuthDetails::new()) as Box<dyn UserAuthStore>));

    server::serve(store).await;
}

