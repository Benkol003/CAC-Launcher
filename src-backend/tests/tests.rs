#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]

use src_backend::*;
use anyhow::{anyhow,Error,Context};

use src_backend::*;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msgraph_folder_link() -> Result<(),Error> {
        println!("test enrty point");
        //SPE folder link to multi parts
        let url = "https://tinyurl.com/2p9k9dsn";
        let url = "https://0jz1q-my.sharepoint.com/:f:/g/personal/brenner650_0jz1q_onmicrosoft_com/EvzzpAaqTodBss4ZD441otIBFQzXx3xQs7XQhz0_YpiZzg?e=9rz54p";

        let client_ctx = build_client_ctx()?;
        let token = msgraph::login(&client_ctx.client)?;
        let item = msgraph::get_shared_drive_item(&client_ctx.client,&token,url)?;

        assert!(item.as_object().context("serde_json::Value.as_object failed")?.contains_key("folder"));

        println!("item:\n{}",serde_json::to_string_pretty(&item)?);
        Ok(())
    }

    #[test]
    fn msgraph_direct_link() -> Result<(),Error> {
        //CBA_A3 direct download link
        let url = "https://0jz1q-my.sharepoint.com/:u:/g/personal/brenner650_0jz1q_onmicrosoft_com/EeNzJRi0EWtAjUoEoakvWZgB4XmveMVxd6mH06fSPh6TBw?e=nZfx8s";


        let client_ctx = build_client_ctx()?;
        let token = msgraph::login(&client_ctx.client)?;
        let item = msgraph::get_shared_drive_item(&client_ctx.client,&token,url)?;
        println!("item:\n{}",serde_json::to_string_pretty(&item)?);

        assert!(item.as_object().context("serde_json::Value.as_object failed")?.contains_key("file"));

        Ok(())
    }
}