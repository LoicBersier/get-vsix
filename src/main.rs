use std::fs::File;
use std::io::Write;
use std::num::ParseIntError;
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::{env, fs, io};

use clap::Parser;
use futures::StreamExt;
use reqwest::Url;
use reqwest::{self, header::CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

#[derive(Error, Debug)]
enum Error {
    #[error("Error while performing a tcp connect: {:?}", .0)]
    ReqwestError(#[source] reqwest::Error),

    #[error("The json recieved doesn't match what is expected: {:?}", .0)]
    JsonError(#[source] reqwest::Error),

    #[error("Problem writing to the file: {:?}", .0)]
    FileWriteError(#[source] std::io::Error),

    #[error("Couldn't find the extension: {:?}", .0)]
    SearchError(String),

    #[error("Couldn't install the extension: {:?}", .0)]
    CommandError(#[source] std::io::Error),

    #[error("Problem moving the file: {:?}", .0)]
    FileMoveError(#[source] std::io::Error),

    #[error("The index you select is invalid")]
    IndexOutOfBoundError(),

    #[error("Couldn't parse a string to an integer")]
    ParseIntError(ParseIntError),

    #[error("Couldn't parse a url")]
    UrlParseError(),

    #[error("Error while trying to flush the buffer: {:?}", .0)]
    FlushError(#[source] std::io::Error),
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
struct Publisher {
    publisherId: String,
    publisherName: String,
    displayName: String,
    flags: String,
    domain: Option<String>,
    isDomainVerified: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
struct Files {
    assetType: String,
    source: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
struct Properties {
    key: String,
    value: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
struct Versions {
    version: String,
    targetPlatform: Option<TargetPlatform>,
    flags: String,
    lastUpdated: String,
    files: Vec<Files>,
    properties: Vec<Properties>,
    assetUri: String,
    fallbackAssetUri: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
struct ExpectedAnswer {
    results: Vec<Results>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
struct Results {
    extensions: Vec<Extension>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
struct Extension {
    publisher: Publisher,
    extensionId: String,
    extensionName: String,
    displayName: String,
    flags: String,
    lastUpdated: String,
    publishedDate: String,
    releaseDate: String,
    shortDescription: Option<String>,
    versions: Vec<Versions>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
struct RequestOptions {
    filters: Vec<RequestFilters>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
struct RequestFilters {
    criteria: Vec<RequestCriteria>,
    pageNumber: i8,
    pageSize: i16,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
struct RequestCriteria {
    filterType: i8,
    value: String,
}

// https://learn.microsoft.com/en-us/javascript/api/azure-devops-extension-api/extensionqueryfiltertype
#[derive(Serialize, Deserialize, Debug)]
enum FilterType {
    Tag = 1,
    DisplayName = 2,
    Private = 3,
    ExtensionId = 4,
    Category = 5,
    ContributionType = 6,
    Name = 7,
    Target = 8,
    Featured = 9,
    SearchText = 10,
    FeaturedInCategory = 11,
    ExcludeWithFlags = 12,
    IncludeWithFlags = 13,
    Lcid = 14,
    InstallationTargetVersion = 15,
    InstallationTargetVersionRange = 16,
    VsixMetadata = 17,
    PublisherName = 18,
    PublisherDisplayName = 19,
    IncludeWithPublisherFlags = 20,
    OrganizationSharedWith = 21,
    ProductArchitecture = 22,
    TargetPlatform = 23,
    ExtensionName = 24,
}

// https://github.com/microsoft/vscode/blob/main/src/vs/platform/extensionManagement/common/extensionGalleryService.ts#L103
#[derive(Serialize, Deserialize, Debug)]
enum RequestFlags {
    None = 0x0,
    IncludeVersions = 0x1,
    IncludeFiles = 0x2,
    IncludeCategoryAndTags = 0x4,
    IncludeSharedAccounts = 0x8,
    IncludeVersionProperties = 0x10,
    ExcludeNonValidated = 0x20,
    IncludeInstallationTargets = 0x40,
    IncludeAssetUri = 0x80,
    IncludeStatistics = 0x100,
    IncludeLatestVersionOnly = 0x200,
    Unpublished = 0x1000,
    IncludeNameConflictInfo = 0x8000,
}

// https://github.com/microsoft/vscode/blob/main/src/vs/platform/extensions/common/extensions.ts#L306
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
enum TargetPlatform {
    #[serde(rename = "win32-ia32")]
    Win32ia32,
    #[serde(rename = "win32-x64")]
    Win32X64,
    #[serde(rename = "win32-arm64")]
    Win32Arm64,

    #[serde(rename = "linux-ia32")]
    Linuxia32,
    #[serde(rename = "linux-x64")]
    LinuxX64,
    #[serde(rename = "linux-arm64")]
    LinuxArm64,
    #[serde(rename = "linux-armhf")]
    LinuxArmhf,

    #[serde(rename = "alpine-ia32")]
    Alpineia32,
    #[serde(rename = "alpine-x64")]
    AlpineX64,
    #[serde(rename = "alpine-arm64")]
    AlpineArm64,

    #[serde(rename = "darwin-x64")]
    DarwinX64,
    #[serde(alias = "darwin-arm64")]
    DarwinArm64,

    WEB,

    UNIVERSAL,
    UNKNOWN,
    UNDEFINED,
}

impl FromStr for TargetPlatform {
    type Err = ();
    fn from_str(input: &str) -> Result<TargetPlatform, Self::Err> {
        match input {
            "win32-x64" => Ok(TargetPlatform::Win32X64),
            "win32-arm64" => Ok(TargetPlatform::Win32Arm64),

            "linux-x64" => Ok(TargetPlatform::LinuxX64),
            "linux-armhf" => Ok(TargetPlatform::LinuxArmhf),
            "linux-arm64" => Ok(TargetPlatform::LinuxArm64),

            "darwin-x64" => Ok(TargetPlatform::DarwinX64),
            "darwin-arm64" => Ok(TargetPlatform::DarwinArm64),
            _ => Err(()),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
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
                        value: (RequestFlags::Unpublished as i8).to_string(),
                    },
                ],
            }],
        })
        .send()
        .await
        .map_err(Error::ReqwestError)?;

    let answer = resp
        .json::<ExpectedAnswer>()
        .await
        .map_err(Error::JsonError)?;

    if answer.results[0].extensions.len() == 0 {
        return Err(Error::SearchError(args.search.clone()));
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
                    .map_err(Error::ParseIntError)?;

            println!();

            match &answer.results[0].extensions.get(choice - 1) {
                Some(i) => i,
                None => return Err(Error::IndexOutOfBoundError()),
            }
        } else {
            println!("Found 1 extension");
            &answer.results[0].extensions[0]
        };

        let publisher_name = &extension.publisher.publisherName;
        let extension_name = &extension.extensionName;

        let arch = match env::consts::ARCH {
            "x86" => "ia32",
            "x86_64" => "x64",
            "arm" => "armhf",
            "aarch64" => "arm64",
            _ => "x64",
        };

        let os = match env::consts::OS {
            "windows" => "win32",
            "linux" => "linux",
            "macos" => "darwin",
            _ => "linux",
        };

        let target_platform = TargetPlatform::from_str(&format!("{}-{}", os, arch)).unwrap();

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
                    .ok_or(Error::IndexOutOfBoundError())?;

                let download_url =
                    match Url::parse(&extension.versions[*index].files[*download_index].source) {
                        Ok(parsed) => Ok(parsed),
                        Err(_) => Err(Error::UrlParseError()),
                    }?;

                let resp = reqwest::get(download_url)
                    .await
                    .map_err(Error::ReqwestError)?;

                let total_size: u64 = resp
                    .content_length()
                    .ok_or(Error::SearchError("AA".to_string()))?;

                let total_size_format = if total_size / 1000 / 1000 > 0 {
                    format!("{} mb", total_size / 1000 / 1000)
                } else if total_size / 1000 > 0 {
                    format!("{} kb", total_size / 1000)
                } else {
                    format!("{} b", total_size)
                };

                println!("Downloading {}", total_size_format);

                let filename = format!("{}.{}-{}.vsix", publisher_name, extension_name, version);
                let tmp_path = format!("{}{}", env::temp_dir().display(), &filename);

                let mut file = File::create(&tmp_path).map_err(Error::FileWriteError)?;
                let mut stream = resp.bytes_stream();

                while let Some(byte) = stream.next().await {
                    let chunk = byte.map_err(Error::ReqwestError)?;
                    print!(".");
                    std::io::stdout().flush().map_err(Error::FlushError)?;
                    file.write_all(&chunk).map_err(Error::FileWriteError)?;
                }

                println!("Download successful.");

                let choice = input(
                    "Do you want me to install the extension you downloaded? [Y/n]: ".to_owned(),
                )?
                .trim()
                .to_lowercase();

                match choice.as_str() {
                    "y" => install_extension(tmp_path, args.program),
                    _ => {
                        let path = format!("{}{}", &args.output, &filename);
                        move_to(tmp_path, path)
                    }
                }?;
            }
            _ => return Ok(()),
        }
    }

    Ok(())
}

fn install_extension(path: String, program: String) -> Result<(), Error> {
    Command::new(program)
        .arg("--install-extension")
        .arg(&path)
        .arg("--force")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(Error::CommandError)?;

    Ok(())
}

fn move_to(tmp_path: String, path: String) -> Result<(), Error> {
    fs::rename(&tmp_path, &path).map_err(Error::FileMoveError)?;
    println!("Wrote to {}", &path);

    Ok(())
}

fn input(prompt: String) -> Result<String, Error> {
    print!("{}", prompt);
    std::io::stdout().flush().map_err(Error::FlushError)?;

    let mut choice = String::new();
    io::stdin()
        .read_line(&mut choice)
        .expect("Failed to read line");

    Ok(choice)
}
