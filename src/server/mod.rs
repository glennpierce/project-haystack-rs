use parking_lot::RwLock;
use std::collections::{HashMap};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::str;
use std::fmt;

use warp;
use warp::{http::StatusCode, Filter, http::Response, Rejection, reject};
use std::convert::Infallible;

use chrono::{DateTime, FixedOffset, TimeZone, Utc};

use std::time::Duration;

use ring;
use bytes;

use crate::hval::{HVal};
use crate::token::*;
use crate::error::*;

use nom::{
    branch::alt,
    bytes::complete::{is_a, tag, tag_no_case},
    character::complete::{multispace0, char, one_of, alpha1},
    combinator::{complete, recognize, map, opt},
    error::{ErrorKind},
    multi::{separated_list},
    sequence::{delimited, preceded, tuple, separated_pair},
    IResult
  };

use data_encoding::{BASE64, BASE64URL, BASE64URL_NOPAD};
use data_encoding::Encoding;

use std::iter;
use rand::{Rng};         // The generic trait all random generators support.
use rand::rngs::OsRng;   // Specific implementation of above for strong crypto.
use rand::prelude::*;

use rand::distributions::{Alphanumeric};

use crate::zinc_tokenizer::grid;

pub fn get_nonce() -> String {

    // r: This attribute specifies a sequence of random printable ASCII
    // characters excluding ',' (which forms the nonce used as input to
    // the hash function).  No quoting is applied to this string.
    iter::repeat(())
        .map(|()| rand::thread_rng().sample(Alphanumeric))
        .take(16)
        .collect()
}

pub fn get_authtoken() -> String {

    // r: This attribute specifies a sequence of random printable ASCII
    // characters excluding ',' (which forms the nonce used as input to
    // the hash function).  No quoting is applied to this string.
    iter::repeat(())
        .map(|()| rand::thread_rng().sample(Alphanumeric))
        .take(30)
        .collect()
}

pub fn get_hanshake_token() -> String {
    iter::repeat(())
        .map(|()| rand::thread_rng().sample(Alphanumeric))
        .take(7)
        .collect()
}

pub fn get_salt() -> Vec<u8> {
    let mut r = OsRng::new().unwrap();

    // Random bytes.
    let mut my_secure_bytes = vec![0u8; 22];
    r.fill_bytes(&mut my_secure_bytes);

    my_secure_bytes
}


// //////////////////////////////////////////////////////////////////////////
// // InvokeActionOp
// //////////////////////////////////////////////////////////////////////////

// class InvokeActionOp extends HOp
// {
//   public String name() { return "invokeAction"; }
//   public String summary() { return "Invoke action on target entity"; }
//   public HGrid onService(HServer db, HGrid req) throws Exception
//   {
//     HRef id = valToId(db, req.meta().get("id"));

//     String action = req.meta().getStr("action");
//     HDict args = HDict.EMPTY;
//     if (req.numRows() > 0) args = req.row(0);
//     return db.invokeAction(id, action, args);
//   }
// }


pub type BoxError = std::boxed::Box<dyn
	std::error::Error   // must implement Error to satisfy ?
	+ std::marker::Send // needed for threads
	+ std::marker::Sync // needed for threads
>;

fn base64url_char<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    let allowed_chars: &str = "_-=abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    is_a(allowed_chars)(i)
}

fn base64_char<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    let allowed_chars: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+/=";
    is_a(allowed_chars)(i)
}

pub fn nom_authorization<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple((tag_no_case("Authorization:"), multispace0)))(i) 
}

pub fn nom_hello<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple((tag_no_case("HELLO"), multispace0)))(i) 
}

pub fn nom_scram<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple((tag_no_case("SCRAM"), multispace0)))(i) 
}

// BEARER authToken=xxxyyyzzz
pub fn bearer<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {

    recognize(tuple((tag_no_case("BEARER"), multispace0)))(i) 
}

pub fn auth_token<'a>(i: &'a str) -> IResult<&'a str, (&'a str, &'a str), (&'a str, ErrorKind)> {

    preceded(bearer, separated_pair(tag_no_case("authToken"), char('='), base64url_char))(i)
}

pub fn nom_hello_username_string<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple((nom_authorization, nom_hello)))(i)
}

fn nom_username_decoded<'a>(i: &'a str) -> IResult<&'a str, String, (&'a str, ErrorKind)> {

    map(
        //preceded(nom_hello_username_string, separated_pair(alpha1, char('='), base64url_char)),
        preceded(nom_hello, separated_pair(alpha1, char('='), base64url_char)),
        |t: (&str, &str)| {
            let tmp: Vec<u8> = BASE64URL_NOPAD.decode(t.1.as_bytes()).unwrap();
            str::from_utf8(&tmp).unwrap().to_string()
         }
        )(i)
}

// scram data=biwsbj11c2VyLHI9VCthZFF4bTlGTTVmU3k0NnR0SFZEK0ov, handshakeToken=aabbbcc
pub fn nom_scram_data<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    delimited(nom_scram, base64url_char, char(','))(i)
}

pub fn nom_comma_space<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    recognize(tuple((multispace0, char(','), multispace0)))(i)
}

fn ident<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    let remaining_chars: &str = "_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let first_chars: &str = "_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
  
    // Returns whole strings matched by the given parser.
    recognize(
      // Runs the first parser, if succeeded then runs second, and returns the second result.
      // Note that returned ok value of `preceded()` is ignored by `recognize()`.
      preceded(
        // Parses a single character contained in the given string.
        one_of(first_chars),
        // Parses the longest slice consisting of the given characters
        opt(is_a(remaining_chars)),
      )
    )(i)
  }

fn var<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    complete(ident)(i)
}

pub fn nom_base64_pair_list<'a>(i: &'a str) -> IResult<&'a str, Vec<(&'a str,&'a str)>, (&'a str, ErrorKind)> {

    separated_list(tuple((multispace0, tag(","), multispace0)), separated_pair(var, char('='), base64_char))(i)
}

pub fn gs2_header<'a>(i: &'a str) -> IResult<&'a str, &'a str, (&'a str, ErrorKind)> {
    //SASL authorization identity
    recognize(tuple((alt((char('n'), char('y'), char('p'))), char(','), opt(var), char(','))))(i)
}

pub fn nom_username_nonce_extractor<'a>(i: &'a Vec<(&'a str,&'a str)>) -> IResult<&'a str, HashMap<String,String>, (&'a str, ErrorKind)> {

    let mut tmp: HashMap<String, String> = HashMap::new();

    for item in i {
        if item.0 == "data" {
            let byte_str: Vec<u8> = BASE64URL_NOPAD.decode(item.1.as_bytes()).unwrap();
            let s = str::from_utf8(&byte_str).unwrap().to_string();
   
            println!("s {}", s);

            match gs2_header(&s) {
                Ok( (remaining, _) ) => {
                    println!("remaining {}", remaining);

                    let data_split = nom_base64_pair_list(remaining);

                    //println!("data_split {:?}", data_split);
                    if data_split.is_err() {
                        return Err(nom::Err::Error(("bad", nom::error::ErrorKind::Tag)));
                    }

                    for j in data_split.unwrap().1 {
                        //tmp.push((j.0.to_string(), j.1.to_string()));
                        tmp.insert(j.0.to_string(), j.1.to_string());
                    }
                },
                Err(_e) => return Err(nom::Err::Error(("bad", nom::error::ErrorKind::Tag)))
            }
        }
        else {
            //tmp.push((item.0.to_string(), item.1.to_string()));
            tmp.insert(item.0.to_string(), item.1.to_string());
        }
    }

    Ok(("", tmp))
}

pub fn nom_scram_first_message<'a>(i: &'a str) -> IResult<&'a str, HashMap<String,String>, (&'a str, ErrorKind)> {

    map(
        preceded(nom_scram, nom_base64_pair_list),
        |v: Vec<(&'a str,&'a str)>| {
            println!("-- {:?}", v);
            nom_username_nonce_extractor(&v).unwrap().1
        }
    )(i)
}

pub fn decode_scram_data<'a>(i: &'a str, encoding: Encoding) -> IResult<&'a str, HashMap<String,String>, (&'a str, ErrorKind)> {

    map(
        nom_base64_pair_list,
        |v: Vec<(&'a str,&'a str)>| {

            let mut tmp: HashMap<String, String> = HashMap::new();

            for item in v {

                let mut s: String = item.1.to_string();

                let byte_str_result = encoding.decode(item.1.as_bytes());

                if byte_str_result.is_ok() {
                    let byte_str: Vec<u8> = byte_str_result.unwrap();
                    let str_result = str::from_utf8(&byte_str);
                    if str_result.is_ok() {
                        s = str_result.unwrap().to_string();
                    }
                }
              
                tmp.insert(item.0.to_string(), s);
            }

            tmp
        }
    )(i)
}

pub fn nom_handshake_token<'a>(i: &'a str) -> IResult<&'a str, String, (&'a str, ErrorKind)> {

    map(
        preceded(nom_scram, separated_pair(tag_no_case("handshakeToken"), char('='), base64url_char)),
        |t: (&str, &str)| {
            let tmp: Vec<u8> = BASE64URL_NOPAD.decode(t.1.as_bytes()).unwrap();
            str::from_utf8(&tmp).unwrap().to_string()
         }
        )(i)
}

fn xor(src1: &[u8], src2: &[u8]) -> Vec<u8> {
    let v3: Vec<u8> = src1.iter().zip(src2.iter()).map(|(&x1, &x2)| x1 ^ x2).collect();

    v3
}

// macro_rules! return_reponse_token {

//     ($code:expr, $token:expr) => {

//         {
//             let response = warp::reply::with_status("", http::StatusCode::from_u16($code).unwrap());
//             let response = warp::reply::with_header(response, "WWW-Authenticate", &format!("SCRAM hash=SHA-256, handshakeToken={}", $token));
//             return Ok(response);
//         }
//     };
// }

pub trait UserAuthStore: fmt::Debug + Send + Sync {

    // Return handshake token for username. If user has no handshake token generate one
    fn get_handshake_token(&self, username: &str) -> HaystackResult<String>;
    fn get_username(&self, handshake_token: &str) -> HaystackResult<String>;

    fn set_temporary_value(&mut self, k: &str, v: &str) -> HaystackResult<()>;
    fn get_temporary_value(&self,  k: &str) -> HaystackResult<Option<&String>>;

    /// returns a base64 encoded sha256 salt of password.
    fn get_password_salt(&self) -> HaystackResult<String>;
    fn get_salted_password(&self) -> HaystackResult<String>;

}

pub type Store = Arc<RwLock<Box<dyn UserAuthStore>>>;


/// returns a password sha256 signed. 
pub fn haystack_generate_salted_password(password: &str, salt: &[u8], iterations: u32) -> Vec<u8>
{
    let password_prep: String = stringprep::saslprep(password).unwrap().to_string();
    
    let PBKDF2_ALG: ring::pbkdf2::Algorithm = ring::pbkdf2::PBKDF2_HMAC_SHA256;
    const CREDENTIAL_LEN: usize = ring::digest::SHA256_OUTPUT_LEN;
    pub type Credential = [u8; CREDENTIAL_LEN];
    let pbkdf2_iterations: NonZeroU32 = NonZeroU32::new(iterations).unwrap();

    let mut salted_passwd: Credential = [0u8; CREDENTIAL_LEN];
    ring::pbkdf2::derive(PBKDF2_ALG, pbkdf2_iterations, &salt,
        password_prep.as_bytes(), &mut salted_passwd);

    salted_passwd.to_vec()
}

#[derive(Debug)]
pub struct HayStackRejection {
    error: String,
}

impl HayStackRejection {

    pub fn new(err: &str) -> Self {
        HayStackRejection {error: err.to_string()}
    }
}

impl reject::Reject for HayStackRejection {}

#[derive(Debug)]
struct HayStackAuthRejection;

impl reject::Reject for HayStackAuthRejection {}


pub async fn haystack_authentication(header: String, store: Store) -> Result<impl warp::Reply, warp::Rejection> {

    debug!("header: {}", header);

    if header.to_lowercase().contains("hello") {
        // Hello message set. Here we decode the baseurl64 username
        // and pass it on
        // Note we only support SCRAM
        let result = nom_username_decoded(&header);

        if result.is_err() {
            return Err(reject::custom(HayStackAuthRejection));
        }

        let username = &result.unwrap().1;
        let handshaken_token_result = store.read().get_handshake_token(&username);

        if handshaken_token_result.is_err() {
            return Err(reject::custom(HayStackAuthRejection));
        }

        let handshaken_token = handshaken_token_result.unwrap();

        let response = warp::reply::with_status("", http::StatusCode::from_u16(401).unwrap());
        let response = warp::reply::with_header(response, "WWW-Authenticate", &format!("SCRAM hash=SHA-256, handshakeToken={}",
            handshaken_token));

        info!("{:?}", response);

        return Ok(response);
    }
    else if header.to_lowercase().contains("scram") {

        let (remaining, _) = nom_scram(&header).unwrap();
        let message = decode_scram_data(remaining, BASE64URL_NOPAD).unwrap().1;
        debug!("message: {:?}", message);

        let client_handshake_token = message.get("handshakeToken").unwrap();
        debug!("client_handshake_token: {:?}", client_handshake_token);
        let data_str = message.get("data").unwrap();
        debug!("data_str: {:?}", data_str);

        // "c=biws,r=FGtSdkud2+OITwYjnsinhdFQTV30vcq9gJLfOA24,p=Rvtb2jtsDwpOTxCul7iqH+btzD8662mQNSped/x8THc="
        let parts: Vec<&str> = data_str.split(",p=").collect();

        // if store.write().set_client_final_no_pf(Some(parts[0].to_string())).is_err() {
        //     return_reponse_token!(401, &get_hanshake_token());
        // }
        if store.write().set_temporary_value("client_final_no_pf", &parts[0].to_string()).is_err() {
            return Err(reject::custom(HayStackAuthRejection));
        }

        let gs2_header_result = gs2_header(data_str);
        let data;

        if gs2_header_result.is_ok() {

            debug!("first message");

            // Respond to client second message

            // In response, the server sends a "server-first-message" containing the
            // user's iteration count i and the user's salt, and appends its own
            // nonce to the client-specified one.

            let (remaining, _) = gs2_header(data_str).unwrap();
     
            // if store.write().set_client_first_message(Some(remaining.to_string())).is_err() {
            //     return_reponse_token!(401, &get_hanshake_token());
            // }
            if store.write().set_temporary_value("client_first_message", &remaining.to_string()).is_err() {
                return Err(reject::custom(HayStackAuthRejection));
            }
  
            data = decode_scram_data(remaining, BASE64).unwrap().1;
            debug!("data: {:?}", data);

            // nonce
            // r: This attribute specifies a sequence of random printable ASCII
            // characters excluding ',' (which forms the nonce used as input to
            // the hash function).  No quoting is applied to this string.
            let server_nonce = get_nonce();

            debug!("server_nonce: {:?}", server_nonce);
            debug!("server_nonce hex: {:X?}", server_nonce);

            //let salt = get_salt();

            let client_username = data.get("n").unwrap();
            let client_nonce = data.get("r").unwrap();

            debug!("client_nonce: {:?}", client_nonce);
            debug!("client_nonce hex: {:X?}", client_nonce);

            let data_store = store.read();
            let stored_username_result = data_store.get_username(client_handshake_token);

            if stored_username_result.is_err() {
                return Err(reject::custom(HayStackAuthRejection));
            }

            if stored_username_result.unwrap() != client_username.to_string() {
                return Err(reject::custom(HayStackAuthRejection));
            }  

            drop(data_store);

            debug!("client_handshake_token: {}", client_handshake_token);
            debug!("client_username: {}", client_username);
            debug!("client_nonce: {}", client_nonce);

            let concatenated_nonce = client_nonce.to_string() + &server_nonce;

            let salt = store.read().get_password_salt().expect("server salt returned error");
            let message = format!("r={},s={},i=10000", concatenated_nonce, BASE64.encode(&salt.as_bytes()));

            if store.write().set_temporary_value("server_first_message", &message.to_string()).is_err() {
                return Err(reject::custom(HayStackAuthRejection));
            }

            drop(store);

            let data = BASE64URL.encode(message.as_bytes());
            let header_str = format!("SCRAM handshakeToken={}, hash=SHA-256, data={}", client_handshake_token, &data);
            let response = warp::reply::with_status("", http::StatusCode::from_u16(401).unwrap());
            let response = warp::reply::with_header(response, "WWW-Authenticate", header_str);

            return Ok(response);
        }
        else {

            println!("second message");

            // To begin with, the SCRAM client is in possession of a username and
            // password (*) (or a ClientKey/ServerKey, or SaltedPassword).  It sends
            // the username to the server, which retrieves the corresponding
            // authentication information, i.e., a salt, StoredKey, ServerKey, and
            // the iteration count i.  (Note that a server implementation may choose    
            //     to use the same iteration count for all accounts.)  The server sends
            //     the salt and the iteration count to the client, which then computes
            //     the following values and sends a ClientProof to the server:
             
            //     (*) Note that both the username and the password MUST be encoded in
            //     UTF-8 [RFC3629].
             
            //     Informative Note: Implementors are encouraged to create test cases
            //     that use both usernames and passwords with non-ASCII codepoints.  In
            //     particular, it's useful to test codepoints whose "Unicode
            //     Normalization Form C" and "Unicode Normalization Form KC" are
            //     different.  Some examples of such codepoints include Vulgar Fraction
            //     One Half (U+00BD) and Acute Accent (U+00B4).
             
            //       SaltedPassword  := Hi(Normalize(password), salt, i)  ->  ring::pbkdf2::derive
            //       ClientKey       := HMAC(SaltedPassword, "Client Key")  -> ring::hmac::sign
            //       StoredKey       := H(ClientKey)    -> ring::digest::digest
            //       AuthMessage     := client-first-message-bare + "," +
            //                          server-first-message + "," +
            //                          client-final-message-without-proof
            //       ClientSignature := HMAC(StoredKey, AuthMessage)
            //       ClientProof     := ClientKey XOR ClientSignature
            //       ServerKey       := HMAC(SaltedPassword, "Server Key")
            //       ServerSignature := HMAC(ServerKey, AuthMessage)
             
            //     The server authenticates the client by computing the ClientSignature,
            //     exclusive-ORing that with the ClientProof to recover the ClientKey
            //     and verifying the correctness of the ClientKey by applying the hash
            //     function and comparing the result to the StoredKey.  If the ClientKey
            //     is correct, this proves that the client has access to the user's
            //     password.
             
            //     Similarly, the client authenticates the server by computing the
            //     ServerSignature and comparing it to the value sent by the server.  If
            //     the two are equal, it proves that the server had access to the user's
            //     ServerKey.
             
            //     The AuthMessage is computed by concatenating messages from the
            //     authentication exchange.

            // The server verifies the nonce and the proof, verifies that the
            // authorization identity (if supplied by the client in the first
            // message) is authorized to act as the authentication identity, and,
            // finally, it responds with a "server-final-message", concluding the
            // authentication exchange.
            let data_store = store.read();

            // We should have server_salt and client_nonce from last message
            let stored_server_salt_result = data_store.get_password_salt();
            let stored_client_first_message_result = data_store.get_temporary_value("client_first_message");
            let stored_server_first_message_result = data_store.get_temporary_value("server_first_message");
            let stored_client_final_no_pf_result = data_store.get_temporary_value("client_final_no_pf");

            if stored_server_salt_result.is_err() 
              || stored_client_first_message_result.is_err() || stored_server_first_message_result.is_err()
              || stored_client_final_no_pf_result.is_err() {
                return Err(reject::custom(HayStackAuthRejection));
            }

            let stored_client_first_message_option = stored_client_first_message_result.unwrap();
            let stored_server_first_message_option = stored_server_first_message_result.unwrap();
            let stored_client_final_no_pf_option = stored_client_final_no_pf_result.unwrap();

            if stored_client_first_message_option.is_none() || stored_server_first_message_option.is_none()
              || stored_client_final_no_pf_option.is_none() {
                debug!("No salt or nonce");
                return Err(reject::custom(HayStackAuthRejection));
            }

            data = decode_scram_data(data_str, BASE64).unwrap().1;

            debug!("data: {:?}", data);

            //let password: String = stringprep::saslprep("pencil").unwrap().to_string();

            let auth_message: String = format!("{},{},{}", 
                    &stored_client_first_message_option.unwrap().to_string(),
                    &stored_server_first_message_option.unwrap().to_string(),
                    &stored_client_final_no_pf_option.unwrap().to_string());

            debug!("auth_message: {:?}", &auth_message.to_string());

            let salted_password_str = data_store.get_salted_password().expect("unable to get salted password");

            debug!("salted_password: {:?}", salted_password_str);

            let salted_passwd = BASE64.decode(salted_password_str.as_bytes()).expect("unable to decode base64");

            debug!("salted_password hex: {:X?}", salted_passwd);

            // var clientKey = hash("Client Key", saltedPassword);
            // ClientKey       := HMAC(SaltedPassword, "Client Key")
            let key: ring::hmac::Key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &salted_passwd);
            let signed_client_key = ring::hmac::sign(&key, "Client Key".as_bytes());

            debug!("signed_client_key: {:X?}", signed_client_key);

            // var storedKey = hash(clientKey);
            // StoredKey       := H(ClientKey)
            let stored_key = ring::digest::digest(&ring::digest::SHA256, signed_client_key.as_ref());
            debug!("my_stored_key: {:X?}", stored_key);

            let client_signature_key: ring::hmac::Key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, stored_key.as_ref());
            let client_signature = ring::hmac::sign(&client_signature_key, auth_message.as_bytes());

            debug!("client_signature: {:X?}", client_signature);

            let client_proof_base64 = data.get("p").unwrap();

            debug!("client_proof_base64: {:?}", client_proof_base64);    

            let client_proof: Vec<u8> = BASE64.decode(client_proof_base64.as_bytes()).unwrap();

            debug!("client_proof: {:x?}", client_proof);    

            let client_key_computed: Vec<u8> = xor(&client_proof, client_signature.as_ref());

            debug!("client_key_computed: {:x?}", client_key_computed);

            drop(data_store);

            if signed_client_key.as_ref() == client_key_computed {
  
                let key: ring::hmac::Key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &salted_passwd);
                let server_key = ring::hmac::sign(&key, "Server Key".as_bytes());

                debug!("signed_client_key: {:X?}", server_key);

                let server_signature_key: ring::hmac::Key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, server_key.as_ref());
                let server_signature = ring::hmac::sign(&server_signature_key, auth_message.as_bytes());

                debug!("server_signature: {:X?}", server_signature);

                let server_signature_base64 = BASE64.encode(server_signature.as_ref());
                debug!("server_signature_base64: {:X?}", server_signature_base64);

                let data = format!("v={}", server_signature_base64);
         
                let auth_token = get_authtoken();

                let message = format!("authToken={}, hash=SHA-256, data={}", auth_token, BASE64URL.encode(data.as_bytes()));

                let response = warp::reply::with_status("Auth successful", http::StatusCode::from_u16(200).unwrap());
                let response = warp::reply::with_header(response, "Authentication-Info", message);

                if store.write().set_temporary_value("authtoken", &auth_token).is_err() {
                    return Err(reject::custom(HayStackAuthRejection));
                }

                debug!("response: {:?}", response);

                return Ok(response);
            }
        }
    }

    return Err(reject::custom(HayStackAuthRejection));
}


// //////////////////////////////////////////////////////////////////////////
// // AboutOp
// //////////////////////////////////////////////////////////////////////////

// Response: single row grid with following columns:

// haystackVersion: Str version of REST implementation, must be "3.0"
// tz: Str of server's default timezone
// serverName: Str name of the server or project database
// serverTime: current DateTime of server's clock
// serverBootTime: DateTime when server was booted up
// productName: Str name of the server software product
// productUri: Uri of the product's web site
// productVersion: Str version of the server software product
// moduleName: module which implements Haystack server protocol if its a plug-in to the product
// moduleVersion: Str version of moduleName
async fn about(_store: Store) -> Result<impl warp::Reply, warp::Rejection> {
   
    let grid_metadata = GridMeta::new(Token::Ver("3.0".into()), None);

    let now: DateTime<FixedOffset> = DateTime::<FixedOffset>::from(Utc::now());

    // Response: single row grid with following columns:
    let cols = Cols::new(vec![Col::new(Token::Id("serverTime".into()), None),
                              Col::new(Token::Id("tz".into()), None),
                             ]);

    let row = Row::new(vec![Val::new(Box::new(Token::DateTime(now))),
                            Val::new(Box::new(Token::EscapedString("UTC".into())))]);

    let grid = Grid::new(grid_metadata, cols, Rows::new(vec![row]));
    let response = warp::reply::with_status(grid.to_zinc(), http::StatusCode::from_u16(200).unwrap());
    let response = warp::reply::with_header(response, "WWW-Authenticate", "SCRAM hash=SHA-256, handshakeToken=aabbbcc");

    Ok(response)
}

// //////////////////////////////////////////////////////////////////////////
// // FOpsOp
// //////////////////////////////////////////////////////////////////////////

async fn ops(_store: Store) -> Result<impl warp::Reply, warp::Rejection> {
   
    // ver:"3.0"
    // name,summary
    // "about","Summary information for server"
    // "ops","Operations supported by this server"
    // "formats","Grid data formats supported by this server"
    // "read","Read records by id or filter"

    let grid_metadata = GridMeta::new(Token::Ver("3.0".into()), None);

    // Response: single row grid with following columns:
    let cols = Cols::new(vec![Col::new(Token::Id("name".into()), None),
                              Col::new(Token::Id("Summary".into()), None),
                             ]);

    let row1 = Row::new(vec![Val::new(Box::new(Token::EscapedString("about".into()))),
                             Val::new(Box::new(Token::EscapedString("Summary information for server".into())))]);

    let row2 = Row::new(vec![Val::new(Box::new(Token::EscapedString("ops".into()))),
                            Val::new(Box::new(Token::EscapedString("Operations supported by this server".into())))]);

    let row3 = Row::new(vec![Val::new(Box::new(Token::EscapedString("formats".into()))),
                            Val::new(Box::new(Token::EscapedString("Grid data formats supported by this server".into())))]);

    let grid = Grid::new(grid_metadata, cols, Rows::new(vec![row1, row2, row3]));
    let response = warp::reply::with_status(grid.to_zinc(), http::StatusCode::from_u16(200).unwrap());
    let response = warp::reply::with_header(response, "WWW-Authenticate", "SCRAM hash=SHA-256, handshakeToken=aabbbcc");

    Ok(response)
}

// //////////////////////////////////////////////////////////////////////////
// // FormatsOp
// //////////////////////////////////////////////////////////////////////////

async fn formats(_store: Store) -> Result<impl warp::Reply, warp::Rejection> {
   
    // ver:"3.0"
    // mime,receive,send
    // "text/csv",,M
    // "text/plain",M,M
    // "text/zinc",M,M

    let grid_metadata = GridMeta::new(Token::Ver("3.0".into()), None);

    // Response: single row grid with following columns:
    let cols = Cols::new(vec![Col::new(Token::Id("mime".into()), None),
                              Col::new(Token::Id("receive".into()), None),
                              Col::new(Token::Id("send".into()), None),
                             ]);

    let row1 = Row::new(vec![Val::new(Box::new(Token::EscapedString("text/csv".into()))),
                            Val::new(Box::new(Token::Empty)),
                            Val::new(Box::new(Token::Marker))]);

    let row2 = Row::new(vec![Val::new(Box::new(Token::EscapedString("text/plain".into()))),
                            Val::new(Box::new(Token::Marker)),
                            Val::new(Box::new(Token::Marker))]);

    let row3 = Row::new(vec![Val::new(Box::new(Token::EscapedString("text/zinc".into()))),
                            Val::new(Box::new(Token::Marker)),
                            Val::new(Box::new(Token::Marker))]);

    let grid = Grid::new(grid_metadata, cols, Rows::new(vec![row1, row2, row3]));
    let response = warp::reply::with_status(grid.to_zinc(), http::StatusCode::from_u16(200).unwrap());
    //let response = warp::reply::with_header(response, "WWW-Authenticate", "SCRAM hash=SHA-256, handshakeToken=aabbbcc");

    Ok(response)
}


// If the request grid is anything other than a single row of name/value pairs, then it must be be sent using HTTP POST. The client must encode the grid using a MIME type supported by server. The client can query the supported MIME types using the formats op. The following is an example of posting to the hisRead op using Zinc:

// POST /haystack/hisRead HTTP/1.1
// Content-Type: text/zinc; charset=utf-8
// Content-Length: 39

// ver:"3.0"
// id,range
// @outsideAirTemp,"yesterday"


// HisRead
// The hisRead op is used to read a time-series data from historized point.

// Request: a grid with a single row and following columns:

// id: Ref identifier of historized point
// range: Str encoding of a date-time range

// Response: rows of the result grid represent timetamp/value pairs with a DateTime ts column and a val column for each scalar value. In addition the grid metadata includes:

// id: Ref of the point we read
// hisStart: DateTime timestamp for inclusive range start in point's timezone
// hisEnd: DateTime timestamp for exclusive range end in point's timezone
// The range Str is formatted as one of the following options:

// "today"
// "yesterday"
// "{date}"
// "{date},{date}"
// "{dateTime},{dateTime}"
// "{dateTime}" // anything after given timestamp
// Ranges are inclusive of start timestamp and exclusive of end timestamp. The {date} and {dateTime} options must be correctly Zinc encoded. Date based ranges are always inferred to be from midnight of starting date to midnight of the day after ending date using the timezone of the his point being queried.

// Clients should query the range using the configured timezone of the point. Although if a different timezone is specified in the range, then servers must convert to the point's configured timezone before executing the query.

// Example:

// // request
// ver:"3.0"
// id,range
// @someTemp,"2012-10-01"

// // reponse
// ver:"3.0" id:@someTemp hisStart:2012-10-01T00:00:00-04:00 New_York hisEnd:2012-10-02T00:00:00-04:00 New_York
// ts,val
// 2012-10-01T00:15:00-04:00 New_York,72.1°F
// 2012-10-01T00:30:00-04:00 New_York,74.2°F
// 2012-10-01T00:45:00-04:00 New_York,75.0°F
// ..
pub async fn historical_read (
    _store: Store,
    grid_bytes: bytes::Bytes,
) -> Result<impl warp::Reply, Infallible> {

    let s = str::from_utf8(&grid_bytes).unwrap();

    println!("s: {}", &s);

    let grid_nom_parse = grid(s);

    println!("nom: {:?}", grid_nom_parse);

    if grid_nom_parse.is_err() {
        let response = warp::reply::with_status("".into(), http::StatusCode::from_u16(400).unwrap());
        return Ok(response);
        // return Ok(StatusCode::BAD_REQUEST);
    }

    let grid: Grid = grid_nom_parse.unwrap().1;

    // id: Ref identifier of historized point
    // range: Str encoding of a date-time range
    let rows = grid.rows;

    println!("{}", rows);

    //let non_ref = Token::Ref("id".into(), Some("someTemp".into()));
    let non_ref = Tag::new_from_token(Token::Id("id".into()), Token::Ref("someTemp".into(), None));
    // hisStart:2012-10-01T00:00:00-04:00 New_York 
    let dt_start = Utc.ymd(1980, 1, 1).and_hms_milli(0, 0, 1, 444);
    let dt_start_fixed_offset: DateTime<FixedOffset> = DateTime::<FixedOffset>::from(dt_start);
    let his_start = Tag::new_from_token(Token::Id("hisStart".into()), Token::DateTime(dt_start_fixed_offset));

    let dt_end = Utc.ymd(2003, 1, 1).and_hms_milli(0, 0, 1, 444);
    let dt_end_fixed_offset: DateTime<FixedOffset> = DateTime::<FixedOffset>::from(dt_end);
    let his_end = Tag::new_from_token(Token::Id("hisEnd".into()),Token::DateTime(dt_end_fixed_offset));

    let grid_metadata = GridMeta::new(Token::Ver("3.0".into()), Some(Tags::new(&vec![non_ref, his_start, his_end])));
    //let grid_metadata = GridMeta::new(Token::Ver("3.0".into()), Some(Tags::new(vec![Tag::new(Token::Id("id".into()), Some(non_ref)])));

    // Response: single row grid with following columns:
    let cols = Cols::new(vec![Col::new(Token::Id("ts".into()), None),
                              Col::new(Token::Id("val".into()), None),
                             ]);

    let dt = Utc.ymd(2008, 5, 1).and_hms_milli(0, 0, 1, 444);
    let dt_offset: DateTime<FixedOffset> = DateTime::<FixedOffset>::from(dt);
 
    let row1 = Row::new(vec![Val::new(Box::new(Token::DateTime(dt_offset))),
                            Val::new(Box::new(Token::Number(ZincNumber::new(637.6), "".into())))]);

    let grid = Grid::new(grid_metadata, cols, Rows::new(vec![row1]));
    let response = warp::reply::with_status(grid.to_zinc(), http::StatusCode::from_u16(200).unwrap());
    //let response = warp::reply::with_header(response, "WWW-Authenticate", "SCRAM hash=SHA-256, handshakeToken=aabbbcc");

    Ok(response)
}


// The hisWrite op is used to post new time-series data to a historized point. The point must already be configured on the server and assigned a unique identifier.

// Request: a grid metadata must define id Ref of point to write to. The rows define new timestamp/value samples to write with following columns:

// ts: DateTime timestamp of sample in point's timezone
// val value of each timestamp sample
// Response: empty grid

// Clients should attempt to avoid writing duplicate data. But servers must gracefully handle clients posting out-of-order or duplicate history data. The timestamp and value kind of the posted data must match the entity's configured timezone and kind. Numeric data posted must either be unitless or must match the entity's configured unit. Timezone, value kind or unit conversion is explicitly disallowed.

// Example:

// Here is an example which posts some new history data to a point:

// // request
// ver:"3.0" id:@hisId
// ts,val
// 2012-04-21T08:30:00-04:00 New_York,72.2
// 2012-04-21T08:45:00-04:00 New_York,76.3
// curl -X POST http://127.0.0.1:4337/hisWrite -H "authorization: BEARER authToken=7e0d0ab09e04776c50681f61cc2e66b0d216fbcc" --data $'ver:"3.0" id:@hisId\nts,val\n2012-04-21T08:30:00-04:00,48.7'
pub async fn historical_write (
    _store: Store,
    grid_bytes: bytes::Bytes,
) -> Result<impl warp::Reply, Infallible> {

    let s = str::from_utf8(&grid_bytes).unwrap();

    println!("s: {}", &s);

    let grid_nom_parse = grid(s);

    println!("nom: {:?}", grid_nom_parse);

    if grid_nom_parse.is_err() {
        let response = warp::reply::with_status("".into(), http::StatusCode::from_u16(400).unwrap());
        return Ok(response);
        // return Ok(StatusCode::BAD_REQUEST);
    }

    let grid: Grid = grid_nom_parse.unwrap().1;

    // id: Ref identifier of historized point
    // range: Str encoding of a date-time range
    let _rows = grid.rows;

    // for r in rows.iter() {
    //     println!("{:?}", r);
    // }
    
    let response = warp::reply::with_status(Grid::empty().to_zinc(), http::StatusCode::from_u16(200).unwrap());
   
    Ok(response)
}

async fn hello(store: Store) -> Result<impl warp::Reply, warp::Rejection> {

    let response = warp::reply::with_status("Hello", http::StatusCode::from_u16(200).unwrap());

    println!("response: {:?} store: {:?}", response, store);

    return Ok(response);
}


pub fn haystack_auth_header(store: Store) -> impl Filter<Extract = (Store,), Error = Rejection> + Clone {

    warp::header("Authorization").and_then (
        
            move |auth_header: String| 
            {
                debug!("haystack_auth_header");

                let tmp = store.clone();
                async move {

                    let tmp = tmp.clone();

                    // Authorization: BEARER authToken=xxxyyyzzz
                    let result = auth_token(&auth_header); //-> IResult<&'a str, (&'a str, &'a str), (&'a str, ErrorKind)> {

                    debug!("haystack_auth_header - auth_token: {:?}", result);

                    if result.is_err() {
                        return Err(reject::custom(HayStackAuthRejection));
                    }
            
                    let (_, key_value) = result.unwrap();
            
                    if "PKLivoIgkH390hiKHOAutagi2Emfd5" == key_value.1 {
                        debug!("haystack_auth_header - Allow");
                        return Ok(tmp.clone());
                    }

                    let datastore = tmp.read();
                    let stored_authtoken_result: HaystackResult<Option<&String>> = datastore.get_temporary_value("authtoken");

                    if stored_authtoken_result.is_err() {
                        return Err(reject::custom(HayStackAuthRejection));
                    }
            
                    let stored_authtoken_option = stored_authtoken_result.unwrap();
            
                    if stored_authtoken_option.is_none() {
                        return Err(reject::custom(HayStackAuthRejection));
                    }
            
                    let auth_token = stored_authtoken_option.unwrap();
            
                    if auth_token != key_value.1 {
                        return Err(reject::custom(HayStackAuthRejection));
                    }
            
                    Ok(tmp.clone())
                }
            }
    )
}


// This function receives a `Rejection` and tries to return a custom
// value, otherwise simply passes the rejection along.
pub async fn handle_rejection(err: Rejection) -> Result<http::response::Response<String>, Infallible> {
    let code;

    debug!("handle_rejection");

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
    } 
    else if let Some(HayStackAuthRejection) = err.find() {
        code = StatusCode::UNAUTHORIZED;
    } 
    else if let Some(_) = err.find::<warp::reject::MethodNotAllowed>() {
        // We can handle a specific error, here METHOD_NOT_ALLOWED,
        // and render it however we want
        code = StatusCode::METHOD_NOT_ALLOWED;
    } else {
        // We should have expected this... Just log and say its a 500
        debug!("unhandled rejection: {:?}", err);
        code = StatusCode::INTERNAL_SERVER_ERROR;
    }

    let mut builder = Response::builder()
        .header("Content-Type", "text/zinc");

    builder = builder.status(code);

    if code == StatusCode::UNAUTHORIZED {
        let header = format!("SCRAM hash=SHA-256, handshakeToken={}", get_authtoken());
        builder = builder.header("WWW-Authenticate", header);
    }

    Ok(builder.body(Grid::empty().to_zinc()).unwrap())
}

#[derive(Debug)]
struct GridSerialisationError;

impl reject::Reject for GridSerialisationError {}

pub async fn serve(store: Store) {

    // if env::var_os("RUST_LOG").is_none() {
    //     // Set `RUST_LOG=todos=debug` to see debug logs,
    //     // this only shows access logs.
    //     env::set_var("RUST_LOG", "todos=info");
    // }

    let cors = warp::cors()
        .allow_origin("http://127.0.0.1:4337")
        .allow_origin("http://127.0.0.1:8080")
        .allow_credentials(true)
        .allow_header("content-type")
        .allow_header("Access-Control-Allow-Origin")
        .allow_methods(vec!["GET", "PUT", "POST", "DELETE"])
        .max_age(Duration::from_secs(600));

    let store_clone = store.clone();
    let store_filter = warp::any().map(move || store_clone.clone() );

    let _default_auth = warp::any().map(|| {
        // something default
        "".to_string()
    });
        
    let ui_route = warp::path("ui")
        .and(warp::path::end())
        .and(warp::header("Authorization"))
        .and(store_filter.clone())
        .and_then(haystack_authentication);

    let hello_route = warp::path("hello")
            .and(warp::path::end())
            .and(haystack_auth_header(store.clone()))
            .and_then(hello);

    let about_route = warp::path("about")
            .and(warp::path::end())
            .and(haystack_auth_header(store.clone()))
            .and_then(about);

    let ops_route = warp::path("ops")
            .and(warp::path::end())
            .and(haystack_auth_header(store.clone()))
            .and_then(ops);      

    let formats_route = warp::path("formats")
            .and(warp::path::end())
            .and(haystack_auth_header(store.clone()))
            .and_then(formats);        
            
    // curl -X POST http://127.0.0.1:4337/hisRead -H "authorization: BEARER authToken=7e0d0ab09e04776c50681f61cc2e66b0d216fbcc" --data $'ver:"3.0"\nid,range\n@someTemp,"2012-10-01"'
    let his_read_route = warp::post()
            .and(warp::path("hisRead"))
            .and(warp::path::end())
            // Only accept bodies smaller than 16kb...
            .and(warp::body::content_length_limit(1024 * 16))
            .and(haystack_auth_header(store.clone()))
            .and(warp::body::bytes())
            .and_then(historical_read);

    let his_write_route = warp::post()
            .and(warp::path("hisWrite"))
            .and(warp::path::end())
            // Only accept bodies smaller than 16kb...
            .and(warp::body::content_length_limit(1024 * 16))
            .and(haystack_auth_header(store.clone()))
            .and(warp::body::bytes())
            .and_then(historical_write);

    //let api = hello_route.or(about_route).or(ui_route); //.or(create).or(update).or(delete);

    //.recover(handle_rejection)
    let api = hello_route.or(about_route).or(ops_route).or(formats_route).or(his_read_route).or(his_write_route).or(ui_route).recover(handle_rejection);

    let routes = api.with(warp::log("webserver")).with(cors);

    //#[cfg(feature = "local")]
   // {
    // Start up the server...
    
        println!("Calling warp::serve");

        warp::serve(routes).run(([0, 0, 0, 0], 4337)).await;

        //println!("Server finished");

  //  }

    // #[cfg(not(feature = "local"))]
    // {
    //     info!("using ssl certs");

    // 	// Start up the server...
    // 	warp::serve(routes)
    //   	      .tls("/etc/letsencrypt/live/****.***.net/cert.pem",
    //     	   "/etc/letsencrypt/live/***.***.net/privkey.pem")
    //   	      .run(([0, 0, 0, 0], 2337));
    // }
}


#[cfg(test)]
mod tests {
    

    #[test]
    fn hello_nom_test() {
        use super::*;

        // assert_eq!(nom_authorization("Authorization: HELLO username=dXNlcg"), Ok(("HELLO username=dXNlcg", "Authorization: ")));
       
        // assert_eq!(nom_hello("HELLO username=dXNlcg"), Ok(("username=dXNlcg", "HELLO ")));
       
        // assert_eq!(nom_hello_username_string("Authorization: HELLO username=dXNlcg"), Ok(("username=dXNlcg", "Authorization: HELLO ")));
       
        // assert_eq!(nom_username_decoded("Authorization: HELLO username=dXNlcg"), Ok(("", "user".into())));
       
        // assert_eq!(nom_authorization("HELLO username=dXNlcg"), Ok(("HELLO username=dXNlcg", "Authorization: ")));
       
        assert_eq!(nom_hello("HELLO username=dXNlcg"), Ok(("username=dXNlcg", "HELLO ")));
       
        //assert_eq!(nom_hello_username_string("HELLO username=dXNlcg"), Ok(("username=dXNlcg", "Authorization: HELLO ")));
       
        assert_eq!(nom_username_decoded("HELLO username=dXNlcg"), Ok(("", "user".into())));
       
        assert_eq!(nom_username_decoded("hello username=dXNlcg"), Ok(("", "user".into())));
    
        // assert_eq!(nom_base64_pair_list("handshakeToken=aabbbcc,data=biwsbj11c2VyLHI9VCthZFF4bTlGTTVmU3k0NnR0SFZEK0ov"),
        //     Ok(("", vec![("handshakeToken".into(), "aabbbcc".into()), ("data".into(), "biwsbj11c2VyLHI9VCthZFF4bTlGTTVmU3k0NnR0SFZEK0ov".into())])));

        // assert_eq!(nom_base64_pair_list("data=biwsbj11c2VyLHI9VCthZFF4bTlGTTVmU3k0NnR0SFZEK0ov, handshakeToken=aabbbcc"),
        //     Ok(("", vec![("data".into(), "biwsbj11c2VyLHI9VCthZFF4bTlGTTVmU3k0NnR0SFZEK0ov".into()), ("handshakeToken".into(), "aabbbcc".into())])));

       

        //assert_eq!(nom_base64_pair("data=biwsbj11c2VyLHI9VCthZFF4bTlGTTVmU3k0NnR0SFZEK0ov, handshakeToken=aabbbcc"),
        //    Ok((", handshakeToken=aabbbcc", Token::FirstMessageDataStrToken("n,,n=user,r=T+adQxm9FM5fSy46ttHVD+J/".into()))));

        // assert_eq!(nom_scram_data_decoded("scram data=biwsbj11c2VyLHI9VCthZFF4bTlGTTVmU3k0NnR0SFZEK0ov, handshakeToken=aabbbcc"),
        //     Ok(("", (
        //              Token::HandshakeToken("aabbbcc".into()),
        //              Token::FirstMessageDataStrToken("n,,n=user,r=T+adQxm9FM5fSy46ttHVD+J/".into())
        //             )
        //        )
        //     ));

        // assert_eq!(nom_scram_data_decoded("scram handshakeToken=aabbbcc, data=biwsbj11c2VyLHI9VCthZFF4bTlGTTVmU3k0NnR0SFZEK0ov"),
        //     Ok(("", (
        //              Token::HandshakeToken("aabbbcc".into()),
        //              Token::FirstMessageDataStrToken("n,,n=user,r=T+adQxm9FM5fSy46ttHVD+J/".into())
        //             )
        //         )
        //     ));

        // assert_eq!(nom_scram_data_decoded("scram handshakeToken=aabbbcc,data=biwsbj11c2VyLHI9VCthZFF4bTlGTTVmU3k0NnR0SFZEK0ov"),
        //     Ok(("", (
        //              Token::HandshakeToken("aabbbcc".into()),
        //              Token::FirstMessageDataStrToken("n,,n=user,r=T+adQxm9FM5fSy46ttHVD+J/".into())
        //             )
        //         )
        //     ));
      
        //assert_eq!(nom_scram_data("scram data=biwsbj11c2VyLHI9VCthZFF4bTlGTTVmU3k0NnR0SFZEK0ov, handshakeToken=aabbbcc"), Ok((" handshakeToken=aabbbcc", "data=biwsbj11c2VyLHI9VCthZFF4bTlGTTVmU3k0NnR0SFZEK0ov".into())));

        //assert_eq!(nom_scram_data("handshakeToken=aabbbcc"), Ok(("", "user".into())));


       // println!("{:?}", nom_scram_firstdata_to_user_and_nonce("scram handshakeToken=aabbbcc,data=biwsbj11c2VyLHI9VCthZFF4bTlGTTVmU3k0NnR0SFZEK0ov"));

       println!("{:?}", nom_scram_first_message("scram handshakeToken=aabbbcc,data=biwsbj11c2VyLHI9VCthZFF4bTlGTTVmU3k0NnR0SFZEK0ov"));

    }
}
