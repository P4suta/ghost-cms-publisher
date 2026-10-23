use std::collections::BTreeMap;
use std::path::Path;

use crate::client::Ghost;
use crate::error::{CoreError, Result};
use crate::hash::StateCache;
use crate::preparation::{AssetKind, PreparedPost};
use crate::transport::HttpTransport;

pub(crate) struct ResolvedContent {
    pub(crate) html: String,
    pub(crate) feature_image: Option<String>,
    pub(crate) og_image: Option<String>,
    pub(crate) twitter_image: Option<String>,
}

pub(crate) async fn resolve<T: HttpTransport>(
    client: &Ghost<T>,
    post: &PreparedPost,
    cache: &mut StateCache,
    state_path: Option<&Path>,
) -> Result<BTreeMap<String, String>> {
    let namespace = client.cache_namespace();
    let mut urls = BTreeMap::new();
    for asset in post.assets.values() {
        let bytes = asset.read_verified()?;
        let cached = cache
            .sites
            .get(namespace)
            .and_then(|s| s.assets.get(&asset.key));
        let url = if let Some(url) = cached {
            validate_url(url)?;
            url.clone()
        } else {
            let filename = asset
                .path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| CoreError::FrontMatter("media filename is not UTF-8".to_owned()))?;
            let media = client.media();
            let url = match asset.kind {
                AssetKind::Image => media.upload_image(bytes, filename, &asset.mime).await?,
                AssetKind::Media => media.upload_media(bytes, filename, &asset.mime).await?,
                AssetKind::File => media.upload_file(bytes, filename, &asset.mime).await?,
            };
            validate_url(&url)?;
            cache
                .sites
                .entry(namespace.to_owned())
                .or_default()
                .assets
                .insert(asset.key.clone(), url.clone());
            if let Some(path) = state_path {
                cache.save(path, post.files.as_ref())?;
            }
            url
        };
        urls.insert(asset.key.clone(), url);
    }
    Ok(urls)
}

fn validate_url(value: &str) -> Result<()> {
    let url = reqwest::Url::parse(value)
        .map_err(|e| CoreError::Inconsistent(format!("invalid uploaded media URL: {e}")))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(CoreError::Inconsistent(
            "uploaded media URL must be an HTTP URL without credentials".to_owned(),
        ));
    }
    Ok(())
}
