use std::env;
use std::fs::File;
use std::io::Write;
use std::process::ExitCode;
use std::time::Instant;

use clap::Parser;
use futures::StreamExt;
use reqwest::Url;
use reqwest::{self, header::CONTENT_TYPE};

mod utility;
use utility::RequestOptions;

use crate::utility::{
    format_size, get_target_platform, input, install_extension, move_to, Ansi, Error,
    ExpectedAnswer, FilterType, RequestCriteria, RequestFilters, RequestFlags,
};

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// The name of the extension you are looking for
    #[arg(required = true)]
    search: String,
    /// URL for the Visual Studio Code marketplace
    #[arg(
        short,
        long,
        default_value = "https://marketplace.visualstudio.com/_apis/public/gallery/extensionquery"
    )]
    api: String,
    /// How many extensions to show
    #[arg(short, long, default_value_t = 5)]
    limit: i16,
    /// The version of the api
    #[arg(short = 'v', long, default_value = "7.2-preview.1")]
    api_version: String,
    /// The program to use to install the extension
    #[arg(short, long, default_value = "codium")]
    program: String,
    /// Where the file is saved
    #[arg(short, long, default_value = "./")]
    output: String,
}

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(error) = get_vsix().await {
        eprintln!("{}", error);
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

async fn get_vsix() -> Result<(), Error> {
    let args = Args::parse();

    let resp = reqwest::Client::new()
        .post(format!("{}?api-version={}", &args.api, &args.api_version))
        .header(CONTENT_TYPE, "application/json")
        .json(&RequestOptions {
            filters: vec![RequestFilters {
                pageNumber: 1,
                pageSize: args.limit,
                criteria: vec![
                    RequestCriteria {
                        filterType: FilterType::SearchText as i8,
                        value: args.search.clone(),
                    },
                    RequestCriteria {
                        filterType: FilterType::Target as i8,
                        value: "Microsoft.VisualStudio.Code".to_string(),
                    },
                    RequestCriteria {
                        filterType: FilterType::ExcludeWithFlags as i8,
                        value: (RequestFlags::Unpublished as i16).to_string(),
                    },
                ],
            }],
        })
        .send()
        .await
        .map_err(Error::ReqwestDns)?;

    let answer = resp
        .json::<ExpectedAnswer>()
        .await
        .map_err(Error::JsonParse)?;

    if answer.results[0].extensions.is_empty() {
        return Err(Error::Search(args.search.clone()));
    } else {
        let extension = if answer.results[0].extensions.len() > 1 {
            println!("Found {} extensions", &answer.results[0].extensions.len());
            println!();

            for (i, extension) in answer.results[0].extensions.iter().enumerate() {
                let publisher_name = &extension.publisher.publisherName;
                let extension_name = &extension.extensionName;
                let version = &extension.versions[0].version;

                println!(
                    "[{}] : {} by {} v{}",
                    i + 1,
                    extension_name,
                    publisher_name,
                    version
                );
            }

            println!();

            let choice: usize =
                input("Input the index of the extension you want to download: ".to_owned())?
                    .trim()
                    .parse()
                    .map_err(Error::ParseInt)?;

            println!();

            match &answer.results[0].extensions.get(choice - 1) {
                Some(i) => i,
                None => return Err(Error::IndexOutOfBound()),
            }
        } else {
            println!("Found 1 extension");
            &answer.results[0].extensions[0]
        };

        let publisher_name = &extension.publisher.publisherName;
        let extension_name = &extension.extensionName;

        let description = match &extension.shortDescription {
            Some(desc) => desc,
            _ => "",
        };

        let target_platform = get_target_platform();

        let index = &extension
            .versions
            .iter()
            .position(|r| match r.targetPlatform {
                Some(t) => t == target_platform,
                None => false,
            });

        let index = match index {
            Some(i) => i,
            None => &0,
        };

        let version = &extension.versions[*index].version;

        println!("{}:", extension_name);
        println!("{}", description);
        println!();
        println!("\tPublisher: {}", publisher_name);
        println!("\tVersion: {}", version);
        println!("\tFlags: {}", &extension.flags);
        println!("\tLast updated: {}", &extension.lastUpdated);
        println!("\tPublished date: {}", &extension.publishedDate);
        println!("\tRelease date: {}", &extension.releaseDate);
        println!();

        let confirm = input("Do you want to continue? [Y/n]: ".to_owned())?
            .trim()
            .to_lowercase();

        match confirm.as_str() {
            "y" => {
                let download_index = &extension.versions[*index]
                    .files
                    .iter()
                    .position(|r| r.assetType == "Microsoft.VisualStudio.Services.VSIXPackage")
                    .ok_or(Error::IndexOutOfBound())?;

                let download_url =
                    match Url::parse(&extension.versions[*index].files[*download_index].source) {
                        Ok(parsed) => Ok(parsed),
                        Err(_) => Err(Error::UrlParse()),
                    }?;

                let resp = reqwest::get(download_url)
                    .await
                    .map_err(Error::ReqwestDns)?;

                let total_size = resp.content_length().ok_or(Error::ReqwestLength())?;

                let total_size_format = format_size(total_size as usize);

                println!("Downloading {}...", total_size_format);

                let filename = format!("{}.{}-{}.vsix", publisher_name, extension_name, version);
                let tmp_path = format!("{}/{}", env::temp_dir().display(), &filename);

                let mut file = File::create(&tmp_path).map_err(Error::FileWrite)?;
                let mut stream = resp.bytes_stream();

                let mut progress = 0;
                let start = Instant::now();
                while let Some(byte) = stream.next().await {
                    let chunk = byte.map_err(Error::ReqwestDns)?;
                    progress += chunk.len();

                    let progress_format = format_size(progress);

                    let percentage: f64 = (progress as f64 / total_size as f64) * 100.0;

                    let elapsed = if start.elapsed().as_secs() <= 0 {
                        1
                    } else {
                        start.elapsed().as_secs()
                    } as usize;

                    let download_speed = (progress - chunk.len()) / elapsed;

                    print!(
                        "{}{}\r{}% [{}{}] {}",
                        Ansi::CursorUp.to_string(),
                        Ansi::ClearLine.to_string(),
                        percentage as usize,
                        {
                            let mut bar = "=".repeat(percentage as usize / 3);
                            if percentage < 100.0 {
                                bar += ">"
                            }
                            bar
                        },
                        " ".repeat(100 / 3 - percentage as usize / 3),
                        progress_format,
                    );

                    print!(
                        "{}\r{}{}/s",
                        Ansi::CursorDown.to_string(),
                        Ansi::ClearLine.to_string(),
                        format_size(download_speed)
                    );

                    std::io::stdout().flush().map_err(Error::Flush)?;
                    file.write_all(&chunk).map_err(Error::FileWrite)?;
                }

                println!("\nDownload successful.");

                let choice = input(
                    "Do you want me to install the extension you downloaded? [Y/n]: ".to_owned(),
                )?
                .trim()
                .to_lowercase();

                match choice.as_str() {
                    "y" => install_extension(tmp_path, args.program),
                    _ => {
                        let path = format!("{}/{}", &args.output, &filename);
                        move_to(tmp_path, path)
                    }
                }?;
            }
            _ => return Ok(()),
        }
    }

    Ok(())
}
