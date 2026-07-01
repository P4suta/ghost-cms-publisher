//! Async client for the Ghost Admin API.
//!
//! [`Ghost`] is a thin handle over an [`HttpTransport`]. Operations are grouped
//! into resource views — [`Ghost::posts`], [`Ghost::tags`], [`Ghost::site`] and
//! [`Ghost::media`] — each of which lives in its own module.

mod media;
mod posts;
mod site;
mod tags;

use reqwest::Url;
use reqwest::header::AUTHORIZATION;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::auth::{StaffToken, current_unix_time};
use crate::config::Config;
use crate::constants::{ACCEPT_VERSION_HEADER, ADMIN_API_PATH};
use crate::domain::{FilesResponse, ImagesResponse, MediaResponse, PostsResponse, TagsResponse};
use crate::error::{CoreError, Operation, Resource, Result};
use crate::transport::{Body, HttpRequest, HttpResponse, HttpTransport, Method, ReqwestTransport};

pub use media::Media;
pub use posts::Posts;
pub use site::Site;
pub use tags::Tags;

/// Mints the auth headers each request carries.
struct RequestSigner {
    token: StaffToken,
    accept_version: String,
}

impl RequestSigner {
    /// Build a freshly-signed set of auth/version headers.
    fn headers(&self) -> Result<Vec<(String, String)>> {
        let jwt = self.token.sign_jwt(current_unix_time())?;
        Ok(vec![
            (AUTHORIZATION.as_str().to_owned(), format!("Ghost {jwt}")),
            (
                ACCEPT_VERSION_HEADER.to_owned(),
                self.accept_version.clone(),
            ),
        ])
    }
}

/// Shared request machinery for every resource view.
struct ClientCtx<T: HttpTransport> {
    transport: T,
    signer: RequestSigner,
    base: Url,
}

impl<T: HttpTransport> ClientCtx<T> {
    /// Resolve a relative API path against the admin base.
    fn url(&self, path: &str) -> Result<Url> {
        self.base
            .join(path)
            .map_err(|e| CoreError::Config(format!("invalid path {path}: {e}")))
    }

    /// Start a signed request to `path` with no query or body.
    fn request(&self, method: Method, path: &str) -> Result<HttpRequest> {
        Ok(HttpRequest {
            method,
            url: self.url(path)?,
            query: Vec::new(),
            headers: self.signer.headers()?,
            body: Body::Empty,
        })
    }

    /// Send a request, surfacing transport failures as [`CoreError`].
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse> {
        Ok(self.transport.execute(req).await?)
    }

    /// Send a request and decode a JSON body, classifying non-2xx statuses.
    async fn send_json<R: DeserializeOwned>(
        &self,
        req: HttpRequest,
        resource: Resource,
        operation: Operation,
    ) -> Result<R> {
        let resp = self.send(req).await?;
        if (200..300).contains(&resp.status) {
            Ok(serde_json::from_slice(&resp.body)?)
        } else {
            Err(CoreError::api(resource, operation, resp.status, &resp.body))
        }
    }

    /// Send a request expecting an empty success body (e.g. 204 on delete).
    async fn send_unit(
        &self,
        req: HttpRequest,
        resource: Resource,
        operation: Operation,
    ) -> Result<()> {
        let resp = self.send(req).await?;
        if (200..300).contains(&resp.status) {
            Ok(())
        } else {
            Err(CoreError::api(resource, operation, resp.status, &resp.body))
        }
    }

    /// Fetch a single-item resource from a one-key envelope, mapping an empty
    /// envelope to [`CoreError::empty`].
    async fn fetch_one<E>(
        &self,
        req: HttpRequest,
        resource: Resource,
        operation: Operation,
    ) -> Result<E::Item>
    where
        E: Envelope + DeserializeOwned,
    {
        let env: E = self.send_json(req, resource, operation).await?;
        env.into_items()
            .into_iter()
            .next()
            .ok_or_else(|| CoreError::empty(resource, operation))
    }
}

/// Serialize a value into a JSON request [`Body`].
fn json_body<B: Serialize>(value: &B) -> Result<Body> {
    Ok(Body::Json(serde_json::to_vec(value)?))
}

/// A one-key Ghost envelope (`{"posts":[…]}`) yielding its inner items.
trait Envelope {
    /// The inner item type.
    type Item;
    /// Consume the envelope, returning its items.
    fn into_items(self) -> Vec<Self::Item>;
}

impl Envelope for PostsResponse {
    type Item = crate::domain::Post;
    fn into_items(self) -> Vec<Self::Item> {
        self.posts
    }
}

impl Envelope for TagsResponse {
    type Item = crate::domain::Tag;
    fn into_items(self) -> Vec<Self::Item> {
        self.tags
    }
}

/// An upload-endpoint envelope yielding the stored asset's CDN URL.
trait UploadResponse: DeserializeOwned {
    /// The CDN URL of the first returned asset, if any.
    fn first_url(self) -> Option<String>;
}

impl UploadResponse for ImagesResponse {
    fn first_url(self) -> Option<String> {
        self.images.into_iter().next().map(|i| i.url)
    }
}

impl UploadResponse for MediaResponse {
    fn first_url(self) -> Option<String> {
        self.media.into_iter().next().map(|i| i.url)
    }
}

impl UploadResponse for FilesResponse {
    fn first_url(self) -> Option<String> {
        self.files.into_iter().next().map(|i| i.url)
    }
}

/// An async client for one Ghost site, generic over its [`HttpTransport`].
pub struct Ghost<T: HttpTransport = ReqwestTransport> {
    ctx: ClientCtx<T>,
}

impl Ghost<ReqwestTransport> {
    /// Build a client from [`Config`], using the default `reqwest` transport.
    ///
    /// # Errors
    /// Returns an error if the token is malformed, the URL is invalid, or the
    /// HTTP client cannot be constructed.
    pub fn new(cfg: &Config) -> Result<Self> {
        Self::with_transport(ReqwestTransport::new()?, cfg)
    }
}

impl<T: HttpTransport> Ghost<T> {
    /// Build a client over an explicit transport (used in tests).
    ///
    /// # Errors
    /// Returns an error if the token is malformed or the URL is invalid.
    pub fn with_transport(transport: T, cfg: &Config) -> Result<Self> {
        let token = StaffToken::parse(&cfg.token)?;
        let base_str = cfg.api_url.trim_end_matches('/');
        let base = Url::parse(&format!("{base_str}{ADMIN_API_PATH}"))
            .map_err(|e| CoreError::Config(format!("invalid api url: {e}")))?;
        Ok(Self {
            ctx: ClientCtx {
                transport,
                signer: RequestSigner {
                    token,
                    accept_version: cfg.accept_version.clone(),
                },
                base,
            },
        })
    }

    /// Operations on posts.
    #[must_use]
    pub const fn posts(&self) -> Posts<'_, T> {
        Posts { ctx: &self.ctx }
    }

    /// Operations on tags.
    #[must_use]
    pub const fn tags(&self) -> Tags<'_, T> {
        Tags { ctx: &self.ctx }
    }

    /// Site metadata.
    #[must_use]
    pub const fn site(&self) -> Site<'_, T> {
        Site { ctx: &self.ctx }
    }

    /// Binary asset uploads (images, media, files).
    #[must_use]
    pub const fn media(&self) -> Media<'_, T> {
        Media { ctx: &self.ctx }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "unit tests panic on failure by design")]
mod tests {
    use super::Ghost;
    use crate::config::Config;
    use crate::error::{ApiError, CoreError};
    use crate::transport::MockTransport;

    const TOKEN: &str = "64abc:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn ghost(status: u16, body: &str) -> Ghost<MockTransport> {
        let cfg = Config::new(
            "https://example.ghost.io".to_owned(),
            TOKEN.to_owned(),
            "v5.0".to_owned(),
        );
        Ghost::with_transport(MockTransport::new(status, body.as_bytes().to_vec()), &cfg).unwrap()
    }

    #[tokio::test]
    async fn classifies_unauthorized() {
        let g = ghost(401, r#"{"errors":[{"message":"bad key"}]}"#);
        let err = g.site().get().await.unwrap_err();
        assert!(matches!(
            err,
            CoreError::Api {
                kind: ApiError::Unauthorized { .. },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn classifies_conflict() {
        let g = ghost(409, r#"{"errors":[{"message":"stale"}]}"#);
        let err = g
            .posts()
            .update("p1", &crate::domain::PostInput::default(), "t")
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            CoreError::Api {
                kind: ApiError::Conflict { .. },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn empty_envelope_is_empty_error() {
        let g = ghost(200, r#"{"posts":[]}"#);
        let err = g.posts().get("missing").await.unwrap_err();
        assert!(matches!(
            err,
            CoreError::Api {
                kind: ApiError::Empty(_),
                ..
            }
        ));
    }

    #[tokio::test]
    async fn find_by_slug_none_when_empty() {
        let g = ghost(200, r#"{"posts":[]}"#);
        assert!(g.posts().find_by_slug("x").await.unwrap().is_none());
    }
}
