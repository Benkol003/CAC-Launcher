#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]

use src_backend::*;
use anyhow::{ anyhow, Error, Context };

use src_backend::*;
#[cfg(test)]
mod tests {
    use indicatif::ProgressBar;
    use reqwest::Url;
    use src_backend::msgraph::FsEntryType;
    use tokio_util::sync::CancellationToken;
    use std::{env, path::{Path, PathBuf}};

    use super::*;

    const MOD_LINK: &str = "https://tinyurl.com/3f3tp9x4"; //@CBA_A3

    //const MOD_LINK_MULTIPART_FOLDER: &str = "???"; //multipart mods arent in a folder atm
    // #[tokio::test]
    // async fn msgraph_folder_link() -> Result<(), Error> {
    //     //SPE folder link to multi parts
    //     let url = Url::parse(MOD_LINK_MULTIPART_FOLDER)?;

    //     let client_ctx = ClientCtx::build()?;
    //     let token = msgraph::login(&client_ctx.client).await?;
    //     let item = msgraph::get_shared_drive_item(client_ctx.client.clone(), token.clone(), url).await?;
    //     if let FsEntryType::File { hashes: _ } = item.item {
    //         panic!("shared drive item is file not folder");
    //     }
    //     println!("item:\n{:?}", item);
    //     Ok(())
    // }

    fn tmp_dir() -> Result<String,std::io::Error> {
        let t = match env::var("RUNNER_TEMP") {
        Ok(s) => s,
        Err(_) => "./tmp".to_string()
        };
        if !std::fs::exists(&t)? {
            std::fs::create_dir(&t)?;
        }
        Ok(t)
    }

    #[tokio::test]
    async fn msgraph_download() -> Result<(), Error> {
        let client_ctx = ClientCtx::build()?;
        let token = msgraph::login(&client_ctx.client).await?;
        let url=Url::parse(MOD_LINK)?;
        let item = msgraph::get_shared_drive_item(client_ctx.client.clone(), token.clone(), url).await?;
        let mut bar =ProgressBar::new(0);
        msgraph::download_item(client_ctx.client.clone(), token.clone(),item.clone(),tmp_dir()?.to_string(),&mut bar, CancellationToken::new()).await?;
        Ok(())
    }

    #[tokio::test]
    async fn url_redirect() -> Result<(), Error> {
        let url = Url::parse(MOD_LINK)?;
        let client_ctx = ClientCtx::build()?;
        let response = client_ctx.client.get(url).timeout(TIMEOUT).send().await?;

        let new_url = response.url();
        println!("final url: {}", new_url);

        let token = msgraph::login(&client_ctx.client).await?;
        let item = msgraph::get_shared_drive_item(client_ctx.client.clone(), token.clone(), new_url.clone()).await?;
        println!("item:\n{:?}", item);

        Ok(())
    }

    #[tokio::test]
    async fn msgraph_direct_link() -> Result<(), Error> {
        //CBA_A3 direct download link
        let url = Url::parse(MOD_LINK)?;
        let client_ctx = ClientCtx::build()?;
        let token = msgraph::login(&client_ctx.client).await?;
        let item = msgraph::get_shared_drive_item(client_ctx.client.clone(), token.clone(), url).await?;
        println!("item:\n{:?}", item);
        if let FsEntryType::Folder { child_count: _ } = item.item {
            panic!("shared drive item is folder not file");
        }
        Ok(())
    }
}
