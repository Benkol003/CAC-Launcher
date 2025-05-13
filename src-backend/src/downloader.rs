use std::fs::OpenOptions;
use std::io::Read;

use anyhow::{anyhow, Error};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use ripunzip::*;

use reqwest::Url;
use src_backend::*;
use src_backend::msgraph::FsEntryType;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    #[clap(flatten)]
    args: ArgsGroup,
}

#[derive(Parser, Debug)]
#[group(required = true, multiple = false)]
struct ArgsGroup {

    #[arg(short, conflicts_with = "url")]
    url_list_file: Option<String>,

    #[arg(index = 1, conflicts_with = "url_list_file")]
    url: Option<String>,
}

struct UnzipProgresshandler<'a> {
    progress_bar: &'a ProgressBar
}
impl<'a> UnzipProgressReporter for UnzipProgresshandler<'a> {

    fn extraction_starting(&self, _display_name: &str) { 
        self.progress_bar.reset();
    }
    fn extraction_finished(&self, _display_name: &str) {
        self.progress_bar.finish();
     }
    fn total_bytes_expected(&self, _expected: u64) {
        self.progress_bar.set_length(_expected);
     }
    fn bytes_extracted(&self, _count: u64) {
        self.progress_bar.set_position(_count);
     }
}

//TODO remove duplicates
const PROGRESS_STYLE: &str = "{spinner} {msg:.green.bold} {percent}% {decimal_bytes}/{decimal_total_bytes} [{decimal_bytes_per_sec}], Elapsed: {elapsed}, ETA: {eta}";

fn main() -> Result<(),Error> {
    let args = Args::parse();
    let ctx = build_client_ctx()?;
    let token = msgraph::login(&ctx.client)?;
    let mut urls: Vec<String> = Vec::new();
    if let Some(path) = args.args.url_list_file {
        if !std::fs::exists(&path)? {
            return Err(anyhow!("file {} does not exist",&path));
        }
        let mut file = std::fs::File::open(&path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        urls.extend(content.lines().map(|x| x.to_string()));

    }else if let Some(url) = args.args.url {
        urls.push(url);
    }

    for url in urls {
        let item = msgraph::get_shared_drive_item(&ctx.client, &token, &url)?;
        msgraph::download_item(&ctx.client, &token, &item, "./")?;

        //TODO: can potentially directly unzip from the download link with from_uri
        match item.name.rsplit_once('.'){
            None => {},
            Some(tuple) => {
                let ext = tuple.1.to_ascii_lowercase();
                if ext=="7z" || ext=="zip" {
                    let zfile = std::fs::File::open(item.name)?;
                    let engine = UnzipEngine::for_file(zfile)?;

                    let progress = ProgressBar::new(item.size as u64).with_style(ProgressStyle::with_template(PROGRESS_STYLE)?);
                    let zprogress = Box::new(UnzipProgresshandler{
                        progress_bar: &progress
                    });

                    let options = UnzipOptions{
                        output_directory: Some("./".into()),
                        password: None,
                        single_threaded: false,
                        filename_filter: None,
                        progress_reporter: zprogress
                    };
                    engine.unzip(options)?;
                }
            }
        }
    }
    Ok(())
}
