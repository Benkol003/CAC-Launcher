use std::{cell::RefCell, path::PathBuf, sync::{Arc, Mutex}};

use anyhow::Error;
use indicatif::{ProgressBar, ProgressStyle};
use log::warn;
use tokio_util::sync::CancellationToken;

use crate::{configs::*, msgraph, unzip, ClientCtx, PROGRESS_STYLE_MESSAGE};

//TODO have args in a struct and cancel on drop
pub async fn download_items(items: Vec<String>,mut progress: ProgressBar, title_buf: Arc<Mutex<String>>, finish: CancellationToken) -> Result<(),Error> {
    let ret = di(items,&mut progress,title_buf,&finish).await;
    finish.cancel();
    ret
}

async fn di(items: Vec<String>,progress: &mut ProgressBar, title_buf: Arc<Mutex<String>>, finish: &CancellationToken) -> Result<(),Error>{
            let mut config = CACConfig::read()?;
            let client_ctx = ClientCtx::build()?; //TODO initialise elsewhere
            let token = msgraph::login(&client_ctx.client).await?;

            //TODO indicate on n/total items

            let content = CACContent::read()?;
            let content_map =  content.content_map();
            for (i,item) in items.iter().enumerate() {

                {
                    let mut lock = title_buf.lock().unwrap();
                    *lock = format!("{}/{}",i,items.len());
                }

                let links  =content_map.get(&item.to_string()).unwrap();
                
                let mut files: Vec<PathBuf> = Vec::new();
                for link in links.into_iter() {
                    progress.set_message(" Fetching info... ");
                    let item = msgraph::get_shared_drive_item(client_ctx.client.clone(), token.clone(),link.to_string() ).await?;
                    let f = msgraph::download_item(client_ctx.client.clone(), token.clone(),item, TMP_FOLDER.to_str().unwrap().to_string(), progress, finish.clone()).await?;
                    files.push(f);
                }
                let dest = match item.starts_with("@"){
                    //TODO  uhhh do i even need to say what the issue is...
                    false => {PathBuf::from(&config.arma_path).parent().unwrap().to_str().unwrap().to_string()}
                    true => {config.absolute_mod_dir()?.to_str().unwrap().to_string()}
                };

                unzip(files.get(0).unwrap().to_str().unwrap(),&dest,Some(progress))?;

                progress.set_style(ProgressStyle::with_template(PROGRESS_STYLE_MESSAGE)?);
                progress.set_message(" cleaning up..."); progress.set_length(1); progress.set_position(0);
                for f in files {
                    std::fs::remove_file(f)?;
                }

                config.pending_updates.remove(item);
                config.save()?;
            }
            finish.cancel();
            Ok(())
        }