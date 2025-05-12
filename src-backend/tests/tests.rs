#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]

use src_backend::*;
use anyhow::{ anyhow, Error, Context };

use src_backend::*;
#[cfg(test)]
mod tests {
    use src_backend::msgraph::FsEntryType;

    use super::*;

    #[test]
    fn msgraph_folder_link() -> Result<(), Error> {
        println!("test entry point");
        //SPE folder link to multi parts
        let url = "https://tinyurl.com/2p9k9dsn";
        let url =
            "https://0jz1q-my.sharepoint.com/:f:/g/personal/brenner650_0jz1q_onmicrosoft_com/EvzzpAaqTodBss4ZD441otIBFQzXx3xQs7XQhz0_YpiZzg?e=9rz54p";

        let client_ctx = build_client_ctx()?;
        let token = msgraph::login(&client_ctx.client)?;
        let item = msgraph::get_shared_drive_item(&client_ctx.client, &token, url)?;

        match item.item {
            FsEntryType::File { hashes: _ } => {
                panic!("shared drive item is file not folder");
            }
            FsEntryType::Folder { child_count: _ } => {}
        }

        println!("item:\n{:?}", item);
        Ok(())
    }

    #[test]
    fn msgraph_download() -> Result<(), Error> {
        let client_ctx = build_client_ctx()?;
        let token = msgraph::login(&client_ctx.client)?;
        let url =
            "https://0jz1q-my.sharepoint.com/:u:/g/personal/brenner650_0jz1q_onmicrosoft_com/EeNzJRi0EWtAjUoEoakvWZgB4XmveMVxd6mH06fSPh6TBw?e=nZfx8s";
        msgraph::download_item(&client_ctx.client, &token, url)?;
        Ok(())
    }

    #[test]
    fn url_redirect() -> Result<(), Error> {
        let url = "https://tinyurl.com/uvs5dkdj";
        let client_ctx = build_client_ctx()?;
        let response = client_ctx.client.get(url).send()?;

        let new_url = response.url();
        println!("final url: {}", new_url);

        let token = msgraph::login(&client_ctx.client)?;
        let item = msgraph::get_shared_drive_item(&client_ctx.client, &token, new_url.as_str())?;
        println!("item:\n{:?}", item);

        Ok(())
    }

    #[test]
    fn msgraph_direct_link() -> Result<(), Error> {
        //CBA_A3 direct download link
        let url =
            "https://0jz1q-my.sharepoint.com/:u:/g/personal/brenner650_0jz1q_onmicrosoft_com/EeNzJRi0EWtAjUoEoakvWZgB4XmveMVxd6mH06fSPh6TBw?e=nZfx8s";
        let client_ctx = build_client_ctx()?;
        let token = msgraph::login(&client_ctx.client)?;
        let item = msgraph::get_shared_drive_item(&client_ctx.client, &token, url)?;
        println!("item:\n{:?}", item);
        match item.item {
            FsEntryType::File { hashes: _ } => {}
            FsEntryType::Folder { child_count: _ } => {
                panic!("shared drive item is folder not file");
            }
        }
        Ok(())
    }
}
