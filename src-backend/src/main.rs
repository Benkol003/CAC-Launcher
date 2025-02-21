#![allow(warnings)] //for debugging only, RM

mod secret;
mod msgraph {
    use std::collections::HashMap;
    

    use urlencoding;

    use serde::Deserialize;
    use base64::{self, prelude::BASE64_STANDARD_NO_PAD,Engine};

    use reqwest::{get, header::{CONTENT_TYPE, HOST}};

    

    use crate::secret;

    fn urlencode<'a>(s: &'a str) -> String {
        return urlencoding::encode(s).into_owned();
    }

    const TENANT_ID: &str = "4fd01353-8fd7-4a18-a3a1-7cd70f528afa";
    const APP_CLIENT_ID: &str = "9ecaa0e8-9caf-4f49-94e8-8430bbf57486";
    //const MSGPRAPH_KEY - place in secrets.rs
    
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


use std::error::Error;

use reqwest::{header, redirect::Policy};
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:135.0) Gecko/20100101 Firefox/135.0"; //app will be blocked without this. reccommend using a browser user agent string to prevent rate limiting.

fn url_download(url: &str) -> Result<bool,Box<dyn Error>> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("user-agent",USER_AGENT.parse()?);
    let client = reqwest::blocking::Client::builder().cookie_store(true).default_headers(headers).redirect(Policy::none()).build()?;

    //resolve tinyurl to underlying sharepoint url
    let response = client.get(url).send()?;
    if !response.status().is_redirection(){
        println!("redirection failed. HTTP status {}",response.status().as_str());
        return Ok(false);
    }
    let sharepoint_url = response.headers().get("Location").unwrap().to_str()?;
    println!("redirect 1: {}",sharepoint_url);

    //one-time use sharepoint url (associated with cookies from previous redirect)
    let response = client.get(sharepoint_url).send()?;
    if !response.status().is_redirection(){
        println!("redirection failed. HTTP status {}",response.status().as_str());
        return Ok(false);
    }
    let once_url = response.headers().get("Location").unwrap().to_str()?;
    println!("redirect 2: {}",once_url);

    let download_url = once_url.replace("onedrive.aspx", "download.aspx").replace("?id=","?SourceUrl=");
    println!("new download url: {}",download_url.as_str());

    //redirect to download 
    let response = client.get(&download_url).send()?;
    if !response.status().is_success(){
        println!("failed to fetch url {} - HTTP error {}: {}",&download_url,response.status().as_str(),response.text()?);
        return Ok(false);
    }

    let size: &str;
    match response.headers().get(reqwest::header::CONTENT_LENGTH) {
        Some(sz) => {
            size=sz.to_str()?; println!("file download size: {}",sz.to_str()?);
        }
        None => {
            println!("file download size unknown");
            return Ok(false);
        }
    }
    
    //filename
    let filename: String;
    match response.headers().get(reqwest::header::CONTENT_DISPOSITION) {
        None => {
            println!("failed to get filename"); 
            return Ok(false);
            
        }
        Some(v) => {
            let filename = v.to_str()?.replace("\"", "");
            println!("filename: {}",&filename);
        }
    }

    return Ok(false);
}

fn url_download_partial(url: String) -> Result<bool,reqwest::Error> {

    return Ok(false);
}

fn main() -> Result<(), Box<dyn Error>> {
    //msgraph::login();
    url_download("https://tinyurl.com/26h79782")?;
    Ok(())
}


