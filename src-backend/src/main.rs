mod secret;

mod msgraph {
    use urlencoding;
    use std::collections::HashMap;
    use serde::Deserialize;
    use base64::{self, prelude::BASE64_STANDARD_NO_PAD,Engine};

    use reqwest::{get, header::{CONTENT_TYPE, HOST}};

    use crate::secret;

    fn urlencode<'a>(s: &'a str) -> String {
        return urlencoding::encode(s).into_owned();
    }

    const TENANT_ID: &str = "4fd01353-8fd7-4a18-a3a1-7cd70f528afa";
    //const KEY: &str = "YOUR-SECRET-KEY"; //or use secret.rs 
    const APP_CLIENT_ID: &str = "9ecaa0e8-9caf-4f49-94e8-8430bbf57486";

    #[derive(Deserialize)]
    struct TokenResponse{
        token_type : String,
        expires_in: usize,
        ext_expires_in: usize,
        access_token: String
    }

pub fn getDriveItem(token: &str, url: &str) -> () {
    let client = reqwest::blocking::Client::new();
    //let mut params = HashMap::new();
    
    let _ = BASE64_STANDARD_NO_PAD.encode(url.as_bytes());
    
}

//todo switch to using a certificate to really overkill things
pub fn login() -> Result<bool,reqwest::Error> {

    let client = reqwest::blocking::Client::new();
    //get an access token
    let mut params = HashMap::new();
    params.insert("client_id",APP_CLIENT_ID);
    params.insert("scope","https://graph.microsoft.com/.default");
    params.insert("client_secret",secret::MSGRAPH_KEY);
    params.insert("grant_type","client_credentials");

    let response = client.post(format!("https://login.microsoftonline.com/{TENANT_ID}/oauth2/v2.0/token"))
    .header(HOST, "login.microsoftonline.com")
    .header(CONTENT_TYPE,"application/x-www-form-urlencoded")
    .form(&params)
    .send()?;

    if response.status().is_success() {
        //println!("access token: {}",response.text().as_ref()?);
        let json: TokenResponse = response.json()?;
        println!("(type: {}) access token: {}",json.token_type,json.access_token);
    }else{
        println!("failed to get access token. status code: {}",response.status().as_str());
        println!("{}",response.text()?);
        return Ok(false)
    }

    return Ok(false);        
 }
}

fn main() {
    println!("Hello, world!");
    msgraph::login();
}


