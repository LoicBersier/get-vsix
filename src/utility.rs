use serde::{Deserialize, Serialize};
use std::io::Write;
use std::num::ParseIntError;
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::{env, fs, io};
use thiserror::Error;

pub fn format_size(size: usize) -> String {
    if size / 1000 / 1000 > 0 {
        format!("{} mb", size / 1000 / 1000)
    } else if size / 1000 > 0 {
        format!("{} kb", size / 1000)
    } else {
        format!("{} b", size)
    }
}

pub fn install_extension(path: String, program: String) -> Result<(), Error> {
    Command::new(program)
        .arg("--install-extension")
        .arg(&path)
        .arg("--force")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(Error::Command)?;

    Ok(())
}

pub fn move_to(tmp_path: String, path: String) -> Result<(), Error> {
    match fs::rename(&tmp_path, &path) {
        Ok(_) => println!("Moved file to {}", &path),
        Err(_) => {
            // If an error occured during the rename its probably because the tmp dir isn't on the same disk as the output
            let tmp_file = fs::read(&tmp_path).map_err(Error::FileRead)?;
            fs::write(&path, tmp_file).map_err(Error::FileWrite)?;
            fs::remove_file(&tmp_path).map_err(Error::FileDelete)?;
            println!("Copied file to {}", &path);
        }
    }

    Ok(())
}

pub fn input(prompt: String) -> Result<String, Error> {
    print!("{}", prompt);
    std::io::stdout().flush().map_err(Error::Flush)?;

    let mut choice = String::new();
    io::stdin()
        .read_line(&mut choice)
        .expect("Failed to read line");

    Ok(choice)
}

pub fn get_target_platform() -> TargetPlatform {
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

    TargetPlatform::from_str(&format!("{}-{}", os, arch)).unwrap()
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Couldn't resolve the site: {}", .0)]
    ReqwestDns(#[source] reqwest::Error),

    #[error("Error while trying to get the content length")]
    ReqwestLength(),

    #[error("The json recieved doesn't match what is expected: {:?}", .0)]
    JsonParse(#[source] reqwest::Error),

    #[error("Error while writing a file: {}", .0)]
    FileWrite(#[source] std::io::Error),

    #[error("Error while reading a file: {}", .0)]
    FileRead(#[source] std::io::Error),

    #[error("Error while deleting a file: {}", .0)]
    FileDelete(#[source] std::io::Error),

    #[error("Couldn't find the extension: {}", .0)]
    Search(String),

    #[error("Couldn't find the program used to install the extension.")]
    Command(#[source] std::io::Error),

    #[error("The index you selected is invalid.")]
    IndexOutOfBound(),

    #[error("Couldn't parse a string to an integer.")]
    ParseInt(ParseIntError),

    #[error("Couldn't parse a url.")]
    UrlParse(),

    #[error("Error while trying to flush the buffer: {:?}", .0)]
    Flush(#[source] std::io::Error),
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct Publisher {
    pub publisherId: String,
    pub publisherName: String,
    pub displayName: String,
    pub flags: String,
    pub domain: Option<String>,
    pub isDomainVerified: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct Files {
    pub assetType: String,
    pub source: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct Properties {
    pub key: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct Versions {
    pub version: String,
    pub targetPlatform: Option<TargetPlatform>,
    pub flags: String,
    pub lastUpdated: String,
    pub files: Vec<Files>,
    pub properties: Vec<Properties>,
    pub assetUri: String,
    pub fallbackAssetUri: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct ExpectedAnswer {
    pub results: Vec<Results>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct Results {
    pub extensions: Vec<Extension>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct Extension {
    pub publisher: Publisher,
    pub extensionId: String,
    pub extensionName: String,
    pub displayName: String,
    pub flags: String,
    pub lastUpdated: String,
    pub publishedDate: String,
    pub releaseDate: String,
    pub shortDescription: Option<String>,
    pub versions: Vec<Versions>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct RequestOptions {
    pub filters: Vec<RequestFilters>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct RequestFilters {
    pub criteria: Vec<RequestCriteria>,
    pub pageNumber: i8,
    pub pageSize: i16,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct RequestCriteria {
    pub filterType: i8,
    pub value: String,
}

// https://learn.microsoft.com/en-us/javascript/api/azure-devops-extension-api/extensionqueryfiltertype
#[derive(Serialize, Deserialize, Debug)]
pub enum FilterType {
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
pub enum RequestFlags {
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
pub enum TargetPlatform {
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
    #[serde(rename = "darwin-arm64")]
    DarwinArm64,

    #[serde(rename = "WEB")]
    Web,

    #[serde(rename = "UNIVERSAL")]
    Universal,
    #[serde(rename = "UNKNOWN")]
    Unknown,
    #[serde(rename = "UNDEFINED")]
    Undefined,
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
