#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]

use src_backend::*;
use anyhow::{ anyhow, Error, Context };

use src_backend::*;
#[cfg(test)]
mod tests {
    use reqwest::Url;
    use src_backend::msgraph::FsEntryType;

    use super::*;

    #[tokio::test]
    async fn msgraph_folder_link() -> Result<(), Error> {
        //SPE folder link to multi parts
        let url = "https://tinyurl.com/2p9k9dsn";

        let client_ctx = build_client_ctx()?;
        let token = msgraph::login(&client_ctx.client).await?;
        let item = msgraph::get_shared_drive_item(client_ctx.client.clone(), token.clone(), url.to_string()).await?;
        if let FsEntryType::File { hashes: _ } = item.item {
            panic!("shared drive item is file not folder");
        }
        println!("item:\n{:?}", item);
        Ok(())
    }

    #[tokio::test]
    async fn msgraph_download() -> Result<(), Error> {
        let client_ctx = build_client_ctx()?;
        let token = msgraph::login(&client_ctx.client).await?;
        let url="https://tinyurl.com/uvs5dkdj";
        let item = msgraph::get_shared_drive_item(client_ctx.client.clone(), token.clone(), url.to_string()).await?;
        msgraph::download_item(client_ctx.client.clone(), token.clone(),item.clone(),"./tmp".to_string()).await?;
        Ok(())
    }

    #[tokio::test]
    async fn url_redirect() -> Result<(), Error> {
        let url = "https://tinyurl.com/uvs5dkdj";
        let client_ctx = build_client_ctx()?;
        let response = client_ctx.client.get(url).send().await?;

        let new_url = response.url();
        println!("final url: {}", new_url);

        let token = msgraph::login(&client_ctx.client).await?;
        let item = msgraph::get_shared_drive_item(client_ctx.client.clone(), token.clone(), new_url.to_string()).await?;
        println!("item:\n{:?}", item);

        Ok(())
    }

    #[tokio::test]
    async fn msgraph_direct_link() -> Result<(), Error> {
        //CBA_A3 direct download link
        let url =
            "https://tinyurl.com/uvs5dkdj";
        let client_ctx = build_client_ctx()?;
        let token = msgraph::login(&client_ctx.client).await?;
        let item = msgraph::get_shared_drive_item(client_ctx.client.clone(), token.clone(), url.to_string()).await?;
        println!("item:\n{:?}", item);
        if let FsEntryType::Folder { child_count: _ } = item.item {
            panic!("shared drive item is folder not file");
        }
        Ok(())
    }
}
