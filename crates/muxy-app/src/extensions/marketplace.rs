use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use muxy_app_core::extensions::{Extension, Marketplace};
use muxy_ui::tr;
use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

mod cache;
mod version;
pub(crate) use version::{is_update, is_valid as is_valid_version};

const ASSET_HOSTS: [&str; 2] = [
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
];
const SCHEMA: u64 = 1;
const MAX_INDEX: u64 = 16 * 1024 * 1024;
const MAX_ARCHIVE: u64 = 256 * 1024 * 1024;
const MAX_EXPANDED: u64 = 512 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;
const PER_PAGE: usize = 24;

type Result<T> = std::result::Result<T, String>;

static INDEXES: Mutex<cache::Indexes<Arc<[Listing]>>> = Mutex::new(cache::Indexes::new());
static PICTURES_PAUSED: Mutex<Option<Instant>> = Mutex::new(None);

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct MarketplaceItem {
    #[serde(deserialize_with = "required_name")]
    pub(crate) name: String,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) description: String,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) version: Option<String>,
    #[serde(default, rename = "icon_url", deserialize_with = "lenient")]
    pub(crate) icon: Option<String>,
    #[serde(default, rename = "screenshot_urls", deserialize_with = "links")]
    pub(crate) screenshots: Vec<String>,
    #[serde(skip)]
    pub(crate) source: Marketplace,
    #[serde(flatten)]
    pub(crate) about: About,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct About {
    #[serde(default, deserialize_with = "author")]
    pub(crate) author: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) categories: Vec<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) downloads: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) official: bool,
    #[serde(default, deserialize_with = "web_link")]
    pub(crate) homepage: Option<String>,
    #[serde(default, deserialize_with = "web_link")]
    pub(crate) repository: Option<String>,
}

impl From<&muxy_app_core::extensions::Listing> for About {
    fn from(listing: &muxy_app_core::extensions::Listing) -> Self {
        let link = |url: &Option<String>| url.as_deref().and_then(checked_link);
        Self {
            author: listing.author.clone(),
            categories: listing.categories.clone(),
            downloads: None,
            official: false,
            homepage: link(&listing.homepage),
            repository: link(&listing.repository),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Sort {
    #[default]
    Popular,
    Recent,
    Name,
}

#[derive(Debug, PartialEq)]
pub(crate) struct MarketplacePage {
    pub(crate) items: Vec<MarketplaceItem>,
    pub(crate) has_next: bool,
    pub(crate) unavailable: Vec<Marketplace>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct ExtensionDetails {
    #[serde(deserialize_with = "required_name")]
    pub(crate) name: String,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) description: String,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) version: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) current_version: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) permissions: Option<BTreeSet<String>>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) download_url: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) size: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) sha256: Option<String>,
    #[serde(default, rename = "icon_url", deserialize_with = "lenient")]
    pub(crate) icon: Option<String>,
    #[serde(default, rename = "screenshot_urls", deserialize_with = "links")]
    pub(crate) screenshots: Vec<String>,
    #[serde(skip)]
    pub(crate) source: Marketplace,
    #[serde(flatten)]
    pub(crate) about: About,
}

impl From<&Extension> for ExtensionDetails {
    fn from(extension: &Extension) -> Self {
        Self {
            name: extension.name.clone(),
            description: extension.manifest.description.clone(),
            version: Some(extension.version.clone()),
            current_version: None,
            permissions: Some(extension.manifest.permissions.clone()),
            download_url: None,
            size: None,
            sha256: None,
            icon: None,
            screenshots: Vec::new(),
            source: Marketplace::official(),
            about: About::from(&extension.manifest.marketplace),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Listing {
    item: MarketplaceItem,
    details: ExtensionDetails,
    published_at: Option<String>,
}

impl Listing {
    fn matches(&self, words: &[String], category: Option<&str>) -> bool {
        let about = &self.item.about;
        let text = [
            self.item.name.as_str(),
            self.item.description.as_str(),
            about.author.as_deref().unwrap_or_default(),
            &about.categories.join(" "),
        ]
        .join(" ")
        .to_lowercase();
        category.is_none_or(|category| {
            about
                .categories
                .iter()
                .any(|listed| listed.eq_ignore_ascii_case(category))
        }) && words.iter().all(|word| text.contains(word.as_str()))
    }
}

fn required_name<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<String, D::Error> {
    match Value::deserialize(deserializer)? {
        Value::String(name) if !name.is_empty() => Ok(name),
        _ => Err(D::Error::custom("name must be a non-empty string")),
    }
}

fn author<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    let value = Value::deserialize(deserializer)?;
    let name = match &value {
        Value::String(name) => Some(name.as_str()),
        Value::Object(author) => author.get("name").and_then(Value::as_str),
        _ => None,
    };
    Ok(name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned))
}

fn web_link<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    Ok(Value::deserialize(deserializer)?
        .as_str()
        .and_then(checked_link))
}

fn checked_link(url: &str) -> Option<String> {
    reqwest::Url::parse(url.trim())
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
        .map(String::from)
}

fn links<'de, D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Vec<String>, D::Error> {
    Ok(Value::deserialize(deserializer)?
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect())
}

fn lenient<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(
    deserializer: D,
) -> std::result::Result<T, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(deserializer)?).unwrap_or_default())
}

fn client() -> Result<reqwest::blocking::Client> {
    crate::proxy::configure(
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= MAX_REDIRECTS {
                    attempt.error("too many redirects")
                } else if asset_redirect(attempt.url()) {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .user_agent(format!(
                "Muxy/{} ({}; {}) Extensions",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH,
            )),
    )
    .and_then(reqwest::blocking::ClientBuilder::build)
    .map_err(|error| error.to_string())
}

fn bytes(response: reqwest::blocking::Response, limit: u64) -> Result<Vec<u8>> {
    let response = response
        .error_for_status()
        .map_err(|error| error.to_string())?;
    if response.content_length().is_some_and(|size| size > limit) {
        return Err(tr!("marketplace response is too large").into());
    }
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err(tr!("marketplace response is too large").into());
    }
    Ok(bytes)
}

fn index(source: &Marketplace) -> Result<Arc<[Listing]>> {
    let directory = cache::directory();
    let parse = |body: &[u8]| parse_index(body, source).map(Into::into);
    let lookup = INDEXES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .lookup(source, Instant::now(), directory.as_deref(), parse);
    let etag = match lookup {
        cache::Lookup::Ready(listings) => return Ok(listings),
        cache::Lookup::Fetch { etag } => etag,
    };
    let fetched = fetch_index(source, etag.as_deref());
    INDEXES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .settle(source, Instant::now(), directory.as_deref(), parse, fetched)
}

#[cfg(test)]
pub(crate) fn serve_index(source: &Marketplace, body: &[u8]) -> Result<()> {
    let parse = |body: &[u8]| parse_index(body, source).map(Into::into);
    INDEXES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .load(source, Instant::now(), None, parse, |_| {
            Ok(cache::Fetched::Modified {
                body: body.to_vec(),
                etag: None,
            })
        })
        .map(drop)
}

pub(crate) fn refresh() {
    INDEXES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .expire();
}

fn fetch_index(
    source: &Marketplace,
    etag: Option<&str>,
) -> std::result::Result<cache::Fetched, cache::Failure> {
    let mut request = client()
        .map_err(cache::Failure::new)?
        .get(source.index_url());
    if let Some(etag) = etag {
        request = request.header(reqwest::header::IF_NONE_MATCH, etag);
    }
    let response = request
        .send()
        .map_err(|error| cache::Failure::new(error.to_string()))?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(cache::Fetched::NotModified);
    }
    if limited(status) || status.is_server_error() {
        return Err(cache::Failure {
            message: tr!("GitHub is not answering right now. Try again in a few minutes.").into(),
            retry_after: retry_after(&response),
        });
    }
    let etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = bytes(response, MAX_INDEX).map_err(cache::Failure::new)?;
    Ok(cache::Fetched::Modified { body, etag })
}

fn limited(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status == reqwest::StatusCode::FORBIDDEN
}

fn retry_after(response: &reqwest::blocking::Response) -> Option<Duration> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
        .map(Duration::from_secs)
}

fn gather<T>(
    sources: &[Marketplace],
    mut load: impl FnMut(&Marketplace) -> Result<T>,
) -> Result<(Vec<T>, Vec<Marketplace>)> {
    let mut loaded = Vec::new();
    let mut unavailable = Vec::new();
    let mut first_error = None;
    for source in sources {
        match load(source) {
            Ok(value) => loaded.push(value),
            Err(error) => {
                crate::diagnostics::event(
                    "marketplace-unavailable",
                    format_args!("source={source} error={error}"),
                );
                first_error.get_or_insert(error);
                unavailable.push(source.clone());
            }
        }
    }
    match first_error {
        Some(error) if loaded.is_empty() => Err(error),
        _ => Ok((loaded, unavailable)),
    }
}

fn parse_index(body: &[u8], source: &Marketplace) -> Result<Vec<Listing>> {
    let index: Value = serde_json::from_slice(body).map_err(|error| error.to_string())?;
    match index.get("schema").and_then(Value::as_u64) {
        Some(SCHEMA) => {}
        Some(_) => {
            return Err(tr!(
                "This version of Muxy can't read the marketplace. Update Muxy and try again."
            )
            .into());
        }
        None => return Err(tr!("Invalid marketplace response").into()),
    }
    let Some(entries) = index.get("extensions").and_then(Value::as_array) else {
        return Err(tr!("Invalid marketplace response").into());
    };
    let mut names = BTreeSet::new();
    Ok(entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| match parse_listing(entry, source) {
            Ok(listing) => names.insert(listing.item.name.clone()).then_some(listing),
            Err(error) => {
                crate::diagnostics::event(
                    "marketplace-listing-skipped",
                    format_args!("source={source} index={index} error={error}"),
                );
                None
            }
        })
        .collect())
}

fn parse_listing(entry: &Value, source: &Marketplace) -> Result<Listing> {
    if !entry.is_object() {
        return Err("not an object".into());
    }
    let mut item = MarketplaceItem::deserialize(entry).map_err(|error| error.to_string())?;
    let mut details = ExtensionDetails::deserialize(entry).map_err(|error| error.to_string())?;
    details.current_version = details.version.take();
    let owned = |url: &String| owns(source, url);
    item.icon = item.icon.filter(owned);
    item.screenshots.retain(owned);
    details.icon = details.icon.filter(owned);
    details.screenshots.retain(owned);
    item.source = source.clone();
    details.source = source.clone();
    if !source.is_official() {
        item.about.official = false;
        details.about.official = false;
    }
    Ok(Listing {
        item,
        details,
        published_at: entry
            .get("published_at")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

pub(crate) fn list(
    sources: &[Marketplace],
    search: &str,
    page: u32,
    category: Option<&str>,
    sort: Sort,
) -> Result<MarketplacePage> {
    let (loaded, unavailable) = gather(sources, index)?;
    let listings: Vec<&Listing> = loaded.iter().flat_map(|listings| listings.iter()).collect();
    Ok(MarketplacePage {
        unavailable,
        ..select(&listings, search, page, category, sort)
    })
}

fn select(
    listings: &[&Listing],
    search: &str,
    page: u32,
    category: Option<&str>,
    sort: Sort,
) -> MarketplacePage {
    let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
    let mut found: Vec<&Listing> = listings
        .iter()
        .copied()
        .filter(|listing| listing.matches(&words, category))
        .collect();
    found.sort_by(|a, b| {
        match sort {
            Sort::Popular => b.item.about.downloads.cmp(&a.item.about.downloads),
            Sort::Recent => b.published_at.cmp(&a.published_at),
            Sort::Name => Ordering::Equal,
        }
        .then_with(|| a.item.name.to_lowercase().cmp(&b.item.name.to_lowercase()))
        .then_with(|| {
            b.item
                .source
                .is_official()
                .cmp(&a.item.source.is_official())
        })
        .then_with(|| a.item.source.cmp(&b.item.source))
    });
    let skipped = (page.max(1) as usize - 1).saturating_mul(PER_PAGE);
    MarketplacePage {
        items: found
            .iter()
            .skip(skipped)
            .take(PER_PAGE)
            .map(|listing| listing.item.clone())
            .collect(),
        has_next: found.len() > skipped.saturating_add(PER_PAGE),
        unavailable: Vec::new(),
    }
}

pub(crate) fn detail(name: &str) -> Result<ExtensionDetails> {
    detail_from(&Marketplace::official(), name)
}

pub(crate) fn detail_from(source: &Marketplace, name: &str) -> Result<ExtensionDetails> {
    find(&index(source)?, name)
}

fn find(listings: &[Listing], name: &str) -> Result<ExtensionDetails> {
    listings
        .iter()
        .find(|listing| listing.item.name == name)
        .map(|listing| listing.details.clone())
        .ok_or_else(|| tr!("This extension isn't in the marketplace.").into())
}

pub(crate) fn picture(url: &str, limit: u64) -> Result<Vec<u8>> {
    let url = reqwest::Url::parse(url).map_err(|error| error.to_string())?;
    if release_source(&url).is_none() {
        return Err(tr!("untrusted marketplace picture URL").into());
    }
    let directory = cache::directory();
    if let Some(bytes) = directory
        .as_deref()
        .and_then(|directory| cache::read_picture(directory, url.as_str(), limit))
    {
        return Ok(bytes);
    }
    let paused = tr!("GitHub is limiting requests. Pictures load again in a few minutes.");
    if PICTURES_PAUSED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some_and(|until| Instant::now() < until)
    {
        return Err(paused.into());
    }
    let response = client()?
        .get(url.clone())
        .send()
        .map_err(|error| error.to_string())?;
    if limited(response.status()) {
        *PICTURES_PAUSED
            .lock()
            .unwrap_or_else(PoisonError::into_inner) =
            Some(Instant::now() + cache::retry_delay(retry_after(&response)));
        return Err(paused.into());
    }
    let bytes = bytes(response, limit)?;
    if let Some(directory) = directory {
        cache::write_picture(&directory, url.as_str(), &bytes);
    }
    Ok(bytes)
}

fn secure(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
}

fn release_source(url: &reqwest::Url) -> Option<Marketplace> {
    if !secure(url) || url.host_str() != Some("github.com") {
        return None;
    }
    let segments: Vec<&str> = url.path_segments()?.collect();
    let [owner, repo, "releases", "download", tag, file] = segments.as_slice() else {
        return None;
    };
    if tag.is_empty() || file.is_empty() {
        return None;
    }
    Marketplace::parse(&format!("{owner}/{repo}")).ok()
}

fn owns(source: &Marketplace, url: &str) -> bool {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|url| release_source(&url))
        .is_some_and(|owner| owner == *source)
}

fn asset_redirect(url: &reqwest::Url) -> bool {
    secure(url)
        && url
            .host_str()
            .is_some_and(|host| ASSET_HOSTS.contains(&host))
}

pub(crate) fn available_update<'a>(
    extension: &Extension,
    installed_from: &Marketplace,
    details: &'a ExtensionDetails,
) -> Option<&'a str> {
    let version = details.current_version.as_deref()?;
    (details.name == extension.name
        && details.source == *installed_from
        && is_update(&extension.version, version))
    .then_some(version)
}

pub(crate) fn versions(installed: &[(String, Marketplace)]) -> Result<BTreeMap<String, String>> {
    let sources: Vec<Marketplace> = installed
        .iter()
        .map(|(_, source)| source.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let (loaded, _) = gather(&sources, |source| Ok((source.clone(), index(source)?)))?;
    Ok(loaded
        .iter()
        .flat_map(|(source, listings)| current_versions(listings, source, installed))
        .collect())
}

fn current_versions(
    listings: &[Listing],
    source: &Marketplace,
    installed: &[(String, Marketplace)],
) -> BTreeMap<String, String> {
    installed
        .iter()
        .filter(|(_, from)| from == source)
        .filter_map(|(name, _)| {
            let listing = listings.iter().find(|listing| &listing.item.name == name)?;
            Some((name.clone(), listing.details.current_version.clone()?))
        })
        .collect()
}

pub(crate) fn download(details: &ExtensionDetails, directory: &Path) -> Result<tempfile::TempDir> {
    let url = reqwest::Url::parse(
        details
            .download_url
            .as_deref()
            .ok_or_else(|| tr!("missing archive URL").to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if release_source(&url).as_ref() != Some(&details.source) {
        return Err(tr!("untrusted marketplace download URL").into());
    }
    let size = details
        .size
        .filter(|size| *size > 0 && *size <= MAX_ARCHIVE)
        .ok_or_else(|| tr!("invalid archive size").to_string())?;
    let expected = details
        .sha256
        .as_deref()
        .filter(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| tr!("invalid archive checksum").to_string())?;
    let permissions = details.permissions.as_ref().ok_or_else(|| {
        tr!("The marketplace did not list this extension's permissions.").to_string()
    })?;
    let data = bytes(
        client()?
            .get(url)
            .send()
            .map_err(|error| error.to_string())?,
        size,
    )?;
    if data.len() as u64 != size {
        return Err(tr!("archive size does not match the marketplace").into());
    }
    if !muxy_core::sha256::matches(&muxy_core::sha256::digest(&data), expected) {
        return Err(tr!("archive checksum does not match the marketplace").into());
    }
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let stage = tempfile::Builder::new()
        .prefix(".install-")
        .tempdir_in(directory)
        .map_err(|error| error.to_string())?;
    let extracted = stage.path().join("package");
    extract(&data, &extracted)?;
    if !extracted.join("package.json").is_file() {
        let roots: Vec<_> = std::fs::read_dir(&extracted)
            .map_err(|e| e.to_string())?
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir() && path.join("package.json").is_file())
            .collect();
        if roots.len() != 1 {
            return Err(tr!("archive must contain one extension package").into());
        }
        let nested = stage.path().join("nested");
        std::fs::rename(&roots[0], &nested).map_err(|e| e.to_string())?;
        std::fs::remove_dir_all(&extracted).map_err(|e| e.to_string())?;
        std::fs::rename(nested, &extracted).map_err(|e| e.to_string())?;
    }
    let extension = Extension::load(&extracted)?;
    if extension.name != details.name
        || Some(extension.version.as_str()) != details.current_version.as_deref()
    {
        return Err(tr!("archive identity does not match the marketplace").into());
    }
    if extension.manifest.permissions != *permissions {
        return Err(tr!("archive permissions do not match the marketplace").into());
    }
    Ok(stage)
}

fn extract(data: &[u8], directory: &Path) -> Result<()> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|error| error.to_string())?;
    if archive.len() > 40000 {
        return Err(tr!("too many files in extension archive").into());
    }
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = entry.name().map_err(|error| error.to_string())?;
        if name.starts_with('/') || name.contains('\\') || name.contains(':') {
            return Err(tr!("unsafe path in extension archive").into());
        }
        if entry.is_symlink() {
            return Err(tr!("extension archives cannot contain symbolic links").into());
        }
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| tr!("unsafe path in extension archive").to_string())?;
        if relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err(tr!("unsafe path in extension archive").into());
        }
        let path = directory.join(relative);
        total = total
            .checked_add(entry.size())
            .filter(|total| *total <= MAX_EXPANDED)
            .ok_or_else(|| tr!("extension archive expands beyond its limit").to_string())?;
        if entry.is_dir() {
            std::fs::create_dir_all(path).map_err(|error| error.to_string())?;
            continue;
        }
        let parent = path
            .parent()
            .ok_or_else(|| tr!("invalid archive path").to_string())?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| error.to_string())?;
        let size = entry.size();
        let copied = std::io::copy(&mut entry.by_ref().take(size + 1), &mut file)
            .map_err(|error| error.to_string())?;
        if copied != size {
            return Err(tr!("archive entry size mismatch").into());
        }
        file.flush().map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn install(stage: &tempfile::TempDir, directory: &Path, name: &str) -> Result<()> {
    let package = stage.path().join("package");
    let extension = Extension::load(&package)?;
    if extension.name != name {
        return Err(tr!("extension identity changed during installation").into());
    }
    let destination = directory.join(name);
    if destination.exists() {
        return Err(tr!(
            "extension is already installed; uninstall it before installing this version"
        )
        .into());
    }
    std::fs::rename(package, destination).map_err(|error| error.to_string())
}

pub(crate) fn update(
    stage: &tempfile::TempDir,
    directory: &Path,
    expected: &Extension,
) -> Result<()> {
    let package = stage.path().join("package");
    let next = Extension::load(&package)?;
    let destination = directory.join(&expected.name);
    let current = Extension::load(&destination)?;
    if next.name != expected.name
        || current.name != expected.name
        || current.version != expected.version
        || !is_update(&current.version, &next.version)
    {
        return Err(tr!("The extension changed. Check for updates and try again.").into());
    }
    let log = current.directory.join("logs/output.log");
    if log.is_file() {
        let logs = next.directory.join("logs");
        std::fs::create_dir_all(&logs).map_err(|error| error.to_string())?;
        std::fs::copy(log, logs.join("output.log")).map_err(|error| error.to_string())?;
    }
    let backup = tempfile::Builder::new()
        .prefix(".update-")
        .tempdir_in(directory)
        .map_err(|error| error.to_string())?;
    replace(&destination, &package, backup)
}

fn replace(current: &Path, next: &Path, backup: tempfile::TempDir) -> Result<()> {
    replace_with(current, next, backup, |from, to| std::fs::rename(from, to))
}

fn replace_with(
    current: &Path,
    next: &Path,
    backup: tempfile::TempDir,
    mut rename: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<()> {
    let previous = backup.path().join("package");
    rename(current, &previous).map_err(|error| error.to_string())?;
    if let Err(error) = rename(next, current) {
        if let Err(rollback) = rename(&previous, current) {
            let saved = backup.keep().join("package");
            return Err(tr!(
                "Could not update the extension (%@) or restore it (%@). The previous version is saved at %@",
                error.to_string(),
                rollback.to_string(),
                saved.display().to_string()
            ).into());
        }
        return Err(tr!(
            "Could not update the extension; the previous version was restored: %@",
            error.to_string()
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::panic,
        reason = "Tests fail immediately on fixture errors"
    )]
    use super::*;

    const RELEASE: &str = "https://github.com/muxy-app/extensions/releases/download";

    fn entry(name: &str, downloads: u64, published_at: &str, categories: &[&str]) -> Value {
        serde_json::json!({
            "name": name,
            "description": format!("{name} for Muxy"),
            "version": "1.2.0",
            "permissions": ["files:read"],
            "author": {"name": "Saeed", "github": "saeed"},
            "categories": categories,
            "downloads": downloads,
            "published_at": published_at,
            "sha256": "a".repeat(64),
            "size": 100,
            "download_url": format!("{RELEASE}/{name}%401.2.0/{name}-1.2.0.zip"),
            "icon_url": format!("{RELEASE}/{name}%401.2.0/icon.svg"),
            "screenshot_urls": [format!("{RELEASE}/{name}%401.2.0/screenshot-1.png")],
        })
    }

    fn official() -> Marketplace {
        Marketplace::official()
    }

    fn theirs() -> Marketplace {
        Marketplace::parse("someone/extensions").unwrap()
    }

    fn index(entries: &[Value]) -> Result<Vec<Listing>> {
        index_from(&official(), entries)
    }

    fn index_from(source: &Marketplace, entries: &[Value]) -> Result<Vec<Listing>> {
        parse_index(
            serde_json::json!({"schema": 1, "extensions": entries})
                .to_string()
                .as_bytes(),
            source,
        )
    }

    fn parse_listing_official(entry: &Value) -> Result<Listing> {
        parse_listing(entry, &official())
    }

    fn details() -> ExtensionDetails {
        find(
            &index(&[entry("reader", 0, "2026-10-11T09:18:43Z", &[])]).unwrap(),
            "reader",
        )
        .unwrap()
    }

    fn rejection(details: &ExtensionDetails) -> String {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("extensions");
        let error = download(details, &directory).unwrap_err();
        assert!(!directory.exists());
        error
    }

    #[test]
    fn the_index_keeps_good_listings_and_skips_bad_ones() {
        let listings = index(&[
            entry("git", 10, "2026-10-01T00:00:00Z", &["git"]),
            serde_json::json!({"description": "no name"}),
            serde_json::json!({"name": "", "description": "empty name"}),
            serde_json::json!({"name": 12}),
            serde_json::json!("not an object"),
            entry("git", 99, "2026-10-02T00:00:00Z", &[]),
            serde_json::json!({"name": "beads", "description": null}),
        ])
        .unwrap();
        let names: Vec<_> = listings.iter().map(|l| l.item.name.as_str()).collect();
        assert_eq!(names, ["git", "beads"]);
        let git = &listings[0];
        assert_eq!(git.item.about.downloads, Some(10));
        assert_eq!(git.item.version.as_deref(), Some("1.2.0"));
        assert_eq!(git.details.current_version.as_deref(), Some("1.2.0"));
        assert_eq!(git.details.version, None);
        assert_eq!(git.details.size, Some(100));
        assert!(
            git.details
                .permissions
                .as_ref()
                .unwrap()
                .contains("files:read")
        );
        assert_eq!(
            git.item.icon.as_deref(),
            Some(format!("{RELEASE}/git%401.2.0/icon.svg").as_str())
        );
        assert_eq!(git.details.screenshots.len(), 1);
        assert_eq!(listings[1].item.description, "");
        assert_eq!(listings[1].item.icon, None);
    }

    #[test]
    fn an_index_needs_a_known_schema_and_a_list_of_extensions() {
        for body in [
            r#"{"extensions": []}"#,
            r#"{"schema": 1}"#,
            r#"{"schema": 1, "extensions": {}}"#,
            r#"{"schema": "1", "extensions": []}"#,
            "[]",
        ] {
            assert_eq!(
                parse_index(body.as_bytes(), &official()).unwrap_err(),
                "Invalid marketplace response"
            );
        }
        assert!(
            parse_index(br#"{"schema": 2, "extensions": []}"#, &official())
                .unwrap_err()
                .contains("Update Muxy")
        );
        assert!(index(&[]).unwrap().is_empty());
    }

    #[test]
    fn browsing_searches_filters_sorts_and_pages_the_index() {
        let mut entries = vec![
            entry("beads", 5, "2026-09-01T00:00:00Z", &["productivity"]),
            entry("Git", 50, "2026-08-01T00:00:00Z", &["git"]),
            entry("ai-usage", 50, "2026-10-01T00:00:00Z", &["ai"]),
        ];
        entries.extend(
            (0..30).map(|n| entry(&format!("pack-{n:02}"), 0, "2026-01-01T00:00:00Z", &[])),
        );
        let listings = index(&entries).unwrap();
        let listings: Vec<&Listing> = listings.iter().collect();
        let names = |page: &MarketplacePage| -> Vec<String> {
            page.items.iter().map(|item| item.name.clone()).collect()
        };

        let popular = select(&listings, "", 1, None, Sort::Popular);
        assert_eq!(names(&popular)[..3], ["ai-usage", "Git", "beads"]);
        assert_eq!(popular.items.len(), PER_PAGE);
        assert!(popular.has_next);
        let last = select(&listings, "", 2, None, Sort::Popular);
        assert_eq!(last.items.len(), 33 - PER_PAGE);
        assert!(!last.has_next);

        let recent = select(&listings, "", 1, None, Sort::Recent);
        assert_eq!(names(&recent)[..3], ["ai-usage", "beads", "Git"]);
        let by_name = select(&listings, "", 1, None, Sort::Name);
        assert_eq!(names(&by_name)[..3], ["ai-usage", "beads", "Git"]);

        assert_eq!(
            names(&select(&listings, "GIT", 1, None, Sort::Name)),
            ["Git"]
        );
        assert_eq!(
            names(&select(&listings, "saeed beads", 1, None, Sort::Name)),
            ["beads"]
        );
        assert_eq!(
            names(&select(&listings, "", 1, Some("Productivity"), Sort::Name)),
            ["beads"]
        );
        assert!(
            select(&listings, "nothing", 1, None, Sort::Name)
                .items
                .is_empty()
        );
    }

    #[test]
    fn details_and_versions_come_from_the_index() {
        let listings = index(&[
            entry("reader", 0, "2026-10-11T09:18:43Z", &[]),
            serde_json::json!({"name": "unversioned"}),
        ])
        .unwrap();
        assert_eq!(find(&listings, "reader").unwrap().name, "reader");
        assert_eq!(
            find(&listings, "missing").unwrap_err(),
            "This extension isn't in the marketplace."
        );
        let names = ["reader", "unversioned", "missing"].map(|name| (name.to_owned(), official()));
        assert_eq!(
            current_versions(&listings, &official(), &names),
            BTreeMap::from([("reader".to_owned(), "1.2.0".to_owned())])
        );
    }

    #[test]
    fn listing_details_read_leniently() {
        let item = parse_listing_official(&serde_json::json!({
            "name": "git",
            "version": "1.0.0",
            "official": true,
            "downloads": "many",
            "categories": ["git", 3],
            "author": "  ",
            "homepage": "javascript:alert(1)",
            "repository": "https://github.com/muxy-app/extensions",
        }))
        .unwrap()
        .item;
        assert_eq!(item.version.as_deref(), Some("1.0.0"));
        assert_eq!(
            item.about,
            About {
                author: None,
                categories: Vec::new(),
                downloads: None,
                official: true,
                homepage: None,
                repository: Some("https://github.com/muxy-app/extensions".into()),
            }
        );
    }

    #[test]
    fn pictures_and_downloads_come_only_from_the_stores_releases() {
        let listing = parse_listing_official(&serde_json::json!({
            "name": "git",
            "icon_url": "https://muxy.app/extensions/git/icon",
            "screenshot_urls": [
                format!("{RELEASE}/git%401.0.0/screenshot-1.png"),
                "https://github.com/someone/else/releases/download/v1/screenshot.png",
                7,
            ],
        }))
        .unwrap();
        assert_eq!(listing.item.icon, None);
        assert_eq!(
            listing.details.screenshots,
            [format!("{RELEASE}/git%401.0.0/screenshot-1.png")]
        );
        for url in [
            "http://github.com/muxy-app/extensions/releases/download/a/icon.svg",
            "https://example.com/muxy-app/extensions/releases/download/a/icon.svg",
            "https://user@github.com/muxy-app/extensions/releases/download/a/icon.svg",
            "https://github.com:8443/muxy-app/extensions/releases/download/a/icon.svg",
            "https://github.com/muxy-app/extensions/raw/main/icon.svg",
            "https://release-assets.githubusercontent.com/github-production-release-asset/1/2",
        ] {
            assert_eq!(
                picture(url, 1024).unwrap_err(),
                "untrusted marketplace picture URL",
                "{url}"
            );
        }
        let redirect = |url: &str| asset_redirect(&reqwest::Url::parse(url).unwrap());
        assert!(redirect(
            "https://release-assets.githubusercontent.com/github-production-release-asset/1/2"
        ));
        assert!(redirect(
            "https://objects.githubusercontent.com/github-production-release-asset-2e65be/1"
        ));
        assert!(!redirect("http://release-assets.githubusercontent.com/a"));
        assert!(!redirect(
            "https://github.com/someone/else/releases/download/git%402.1.0/git-2.1.0.zip"
        ));
        assert!(!redirect(
            "https://raw.githubusercontent.com/muxy-app/extensions/main/a"
        ));
        assert!(!redirect("https://example.com/a"));
    }

    #[test]
    fn skipped_listings_are_recorded_without_their_contents() {
        for listing in [
            serde_json::json!("private text"),
            serde_json::json!(["private text"]),
            serde_json::json!({"name": "", "description": "private text"}),
            serde_json::json!({"name": ["private text"]}),
        ] {
            assert!(
                !parse_listing(&listing, &official())
                    .unwrap_err()
                    .contains("private")
            );
        }
    }

    #[test]
    fn malformed_fields_parse_and_are_rejected_before_any_request() {
        let mut wrong_size = details();
        wrong_size.size =
            parse_listing_official(&serde_json::json!({"name": "reader", "size": "100"}))
                .unwrap()
                .details
                .size;
        assert_eq!(wrong_size.size, None);
        assert_eq!(rejection(&wrong_size), "invalid archive size");

        for body in [
            serde_json::json!({"name": "reader"}),
            serde_json::json!({"name": "reader", "permissions": ["files:read", 7]}),
        ] {
            let mut unlisted = details();
            unlisted.permissions = parse_listing(&body, &official())
                .unwrap()
                .details
                .permissions;
            assert_eq!(unlisted.permissions, None);
            assert_eq!(
                rejection(&unlisted),
                "The marketplace did not list this extension's permissions."
            );
        }
    }

    #[test]
    fn downloads_reject_untrusted_urls_and_malformed_checksums() {
        for url in [
            "https://muxy.app/api/extensions/reader/download",
            "https://example.com/archive.zip",
            "http://github.com/muxy-app/extensions/releases/download/reader%401.2.0/reader-1.2.0.zip",
            "https://user:secret@github.com/muxy-app/extensions/releases/download/r/r.zip",
            "https://github.com:8443/muxy-app/extensions/releases/download/r/r.zip",
            "https://github.com/someone/extensions/releases/download/r/r.zip",
            "https://release-assets.githubusercontent.com/github-production-release-asset/1/2",
        ] {
            let mut foreign = details();
            foreign.download_url = Some(url.into());
            assert_eq!(
                rejection(&foreign),
                "untrusted marketplace download URL",
                "{url}"
            );
        }
        for checksum in ["a".repeat(63), "g".repeat(64)] {
            let mut malformed = details();
            malformed.sha256 = Some(checksum);
            assert_eq!(rejection(&malformed), "invalid archive checksum");
        }
    }

    #[test]
    fn an_added_marketplace_keeps_only_its_own_release_assets_and_is_never_official() {
        let theirs = theirs();
        let own = "https://github.com/Someone/Extensions/releases/download/git%401.0.0";
        let listings = index_from(
            &theirs,
            &[serde_json::json!({
                "name": "git",
                "version": "1.0.0",
                "official": true,
                "download_url": format!("{own}/git-1.0.0.zip"),
                "icon_url": format!("{RELEASE}/git%402.1.0/icon.svg"),
                "screenshot_urls": [
                    format!("{own}/screenshot-1.png"),
                    format!("{RELEASE}/git%402.1.0/screenshot-1.png"),
                    "https://github.com/someone/extensions-evil/releases/download/a/b.png",
                    "https://github.com/someone/extensions/raw/main/b.png",
                ],
            })],
        )
        .unwrap();
        let git = &listings[0];
        assert_eq!(git.item.source, theirs);
        assert_eq!(git.details.source, theirs);
        assert!(!git.item.about.official && !git.details.about.official);
        assert_eq!(git.item.icon, None);
        assert_eq!(git.details.screenshots, [format!("{own}/screenshot-1.png")]);
    }

    #[test]
    fn downloads_must_come_from_the_marketplace_the_details_came_from() {
        let mut from_theirs = details();
        from_theirs.source = theirs();
        assert_eq!(
            rejection(&from_theirs),
            "untrusted marketplace download URL"
        );
        from_theirs.download_url = Some(
            "https://github.com/someone/extensions/releases/download/reader%401.2.0/reader-1.2.0.zip"
                .into(),
        );
        from_theirs.sha256 = Some("bad".into());
        assert_eq!(rejection(&from_theirs), "invalid archive checksum");

        let mut official = details();
        official.download_url = from_theirs.download_url.clone();
        assert_eq!(rejection(&official), "untrusted marketplace download URL");
    }

    #[test]
    fn an_update_comes_only_from_the_marketplace_it_was_installed_from() {
        let root = tempfile::tempdir().unwrap();
        let installed = package(&root.path().join("reader"), "reader", "1.0.0");
        let mut listed = details();
        listed.source = theirs();
        assert_eq!(available_update(&installed, &official(), &listed), None);
        assert_eq!(
            available_update(&installed, &theirs(), &listed),
            Some("1.2.0")
        );

        let official_index = index(&[entry("reader", 0, "2026-10-11T09:18:43Z", &[])]).unwrap();
        let installed_from = [("reader".to_owned(), theirs())];
        assert!(current_versions(&official_index, &official(), &installed_from).is_empty());
    }

    #[test]
    fn browsing_merges_marketplaces_and_lists_the_official_listing_first() {
        let ours = index(&[entry("git", 1, "2026-01-01T00:00:00Z", &[])]).unwrap();
        let other = index_from(
            &theirs(),
            &[
                entry("git", 1, "2026-01-01T00:00:00Z", &[]),
                entry("notes", 1, "2026-01-01T00:00:00Z", &[]),
            ],
        )
        .unwrap();
        let merged: Vec<&Listing> = other.iter().chain(ours.iter()).collect();
        let page = select(&merged, "", 1, None, Sort::Name);
        let listed: Vec<(String, String)> = page
            .items
            .iter()
            .map(|item| (item.name.clone(), item.source.to_string()))
            .collect();
        assert_eq!(
            listed,
            [
                ("git".to_owned(), "muxy-app/extensions".to_owned()),
                ("git".to_owned(), "someone/extensions".to_owned()),
                ("notes".to_owned(), "someone/extensions".to_owned()),
            ]
        );
    }

    #[test]
    fn a_broken_marketplace_is_reported_without_hiding_the_others() {
        let sources = [official(), theirs()];
        let (loaded, unavailable) = gather(&sources, |source| {
            if source.is_official() {
                Ok(1)
            } else {
                Err("offline".to_owned())
            }
        })
        .unwrap();
        assert_eq!(loaded, [1]);
        assert_eq!(unavailable, [theirs()]);
        assert_eq!(
            gather(&sources, |_| Err::<(), _>("offline".to_owned())).unwrap_err(),
            "offline"
        );
    }

    #[test]
    fn installed_details_never_offer_an_update() {
        let root = tempfile::tempdir().unwrap();
        let installed = package(&root.path().join("reader"), "reader", "1.0.0");
        let local = ExtensionDetails::from(&installed);
        assert_eq!(local.version.as_deref(), Some("1.0.0"));
        assert_eq!(available_update(&installed, &official(), &local), None);

        let mut listed = details();
        assert_eq!(
            available_update(&installed, &official(), &listed),
            Some("1.2.0")
        );
        listed.name = "other".into();
        assert_eq!(available_update(&installed, &official(), &listed), None);
    }

    fn archive(path: &str, mode: Option<u32>) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        if mode == Some(0o120_777) {
            writer
                .add_symlink(
                    path,
                    "/tmp/outside",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
        } else {
            writer
                .start_file(path, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"hello").unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn archives_cannot_escape_staging_or_install_symlinks() {
        let root = tempfile::tempdir().unwrap();
        assert!(extract(&archive("../outside", None), root.path()).is_err());
        assert!(extract(&archive("/tmp/outside", None), root.path()).is_err());
        assert!(extract(&archive("link", Some(0o120_777)), root.path()).is_err());
        extract(&archive("panel/index.html", None), root.path()).unwrap();
        assert_eq!(
            std::fs::read(root.path().join("panel/index.html")).unwrap(),
            b"hello"
        );
    }

    fn package(root: &Path, name: &str, version: &str) -> Extension {
        std::fs::create_dir_all(root).unwrap();
        std::fs::write(
            root.join("package.json"),
            serde_json::json!({
                "name": name, "version": version, "muxy": {}
            })
            .to_string(),
        )
        .unwrap();
        Extension::load(root).unwrap()
    }

    #[test]
    fn updates_replace_code_and_preserve_logs_across_resource_layout_changes() {
        let root = tempfile::tempdir().unwrap();
        let installed = package(&root.path().join("reader/dist"), "reader", "1.0.0");
        std::fs::write(installed.directory.join("old.js"), "old code").unwrap();
        std::fs::create_dir(installed.directory.join("logs")).unwrap();
        std::fs::write(
            installed.directory.join("logs/output.log"),
            "earlier output",
        )
        .unwrap();
        let stage = tempfile::tempdir_in(root.path()).unwrap();
        let next = package(&stage.path().join("package"), "reader", "1.1.0");
        std::fs::write(next.directory.join("new.js"), "new code").unwrap();

        update(&stage, root.path(), &installed).unwrap();

        let installed = Extension::load(&root.path().join("reader")).unwrap();
        assert_eq!(installed.version, "1.1.0");
        assert_eq!(
            std::fs::read_to_string(installed.directory.join("new.js")).unwrap(),
            "new code"
        );
        assert_eq!(
            std::fs::read_to_string(installed.directory.join("logs/output.log")).unwrap(),
            "earlier output"
        );
        assert!(!installed.directory.join("dist").exists());
    }

    #[test]
    fn updates_reject_wrong_identity_older_versions_and_stale_installs() {
        let root = tempfile::tempdir().unwrap();
        let installed = package(&root.path().join("reader"), "reader", "1.1.0");
        for (name, version) in [("other", "2.0.0"), ("reader", "1.0.0"), ("reader", "1.1.0")] {
            let stage = tempfile::tempdir_in(root.path()).unwrap();
            package(&stage.path().join("package"), name, version);
            assert!(update(&stage, root.path(), &installed).is_err());
            assert_eq!(
                Extension::load(&root.path().join("reader"))
                    .unwrap()
                    .version,
                "1.1.0"
            );
        }
        let stage = tempfile::tempdir_in(root.path()).unwrap();
        package(&stage.path().join("package"), "reader", "2.0.0");
        package(&root.path().join("reader"), "reader", "1.2.0");
        assert!(update(&stage, root.path(), &installed).is_err());
        assert_eq!(
            Extension::load(&root.path().join("reader"))
                .unwrap()
                .version,
            "1.2.0"
        );
    }

    #[test]
    fn failed_replacement_restores_the_installed_extension() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("reader");
        package(&current, "reader", "1.0.0");
        let backup = tempfile::tempdir_in(root.path()).unwrap();
        assert!(replace(&current, &root.path().join("missing"), backup).is_err());
        assert_eq!(Extension::load(&current).unwrap().version, "1.0.0");
    }

    #[test]
    fn failed_rollback_keeps_the_backup_and_reports_its_location() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("reader");
        package(&current, "reader", "1.0.0");
        let backup = tempfile::tempdir_in(root.path()).unwrap();
        let saved = backup.path().join("package");
        let mut calls = 0;
        let result = replace_with(
            &current,
            &root.path().join("missing"),
            backup,
            |from, to| {
                calls += 1;
                if calls == 1 {
                    std::fs::rename(from, to)
                } else {
                    Err(std::io::Error::other("test failure"))
                }
            },
        );
        assert!(result.unwrap_err().contains(&saved.display().to_string()));
        assert_eq!(Extension::load(&saved).unwrap().version, "1.0.0");
    }
}
