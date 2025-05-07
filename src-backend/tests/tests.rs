#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]

use src_backend::*;
use anyhow::{anyhow,Error};

use src_backend::*;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msgraph_folder_link() -> Result<(),Error> {

        //SPE folder link to multi parts
        let url = "https://tinyurl.com/2p9k9dsn";
        let client_ctx = build_client_ctx()?;
        let token = msgraph::login(&client_ctx.client)?;
        let item = msgraph::get_shared_drive_item(&client_ctx.client,&token,url)?;

        return Ok(());
    }
}