

use std::env::VarError;
use once_cell::sync::Lazy;

pub fn msgraph_key() -> Result<String,VarError>{    
    std::env::var("MSGRAPH_KEY")
}