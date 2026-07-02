//! The `posts` resource view.

use super::{ClientCtx, json_body};
use crate::constants::MAX_PAYLOAD_BYTES;
use crate::domain::{Post, PostInput, PostsRequest, PostsResponse};
use crate::error::{CoreError, Operation, Resource, Result};
use crate::transport::{HttpTransport, Method};

/// Operations on Ghost posts.
pub struct Posts<'a, T: HttpTransport> {
    pub(super) ctx: &'a ClientCtx<T>,
}

impl<T: HttpTransport> Posts<'_, T> {
    /// Reject payloads above the plan limit before hitting the network.
    fn check_size(body: &PostsRequest) -> Result<()> {
        let len = serde_json::to_vec(body)?.len() as u64;
        if len > MAX_PAYLOAD_BYTES {
            return Err(CoreError::TooLarge {
                size: len,
                limit: MAX_PAYLOAD_BYTES,
            });
        }
        Ok(())
    }

    /// `GET /posts/?filter=slug:<slug>` — the matching post, if any.
    ///
    /// # Errors
    /// Propagates transport and API errors.
    pub async fn find_by_slug(&self, slug: &str) -> Result<Option<Post>> {
        let mut req = self.ctx.request(Method::Get, "posts/")?;
        req.query = vec![
            ("filter".to_owned(), format!("slug:{slug}")),
            ("formats".to_owned(), "html".to_owned()),
            ("limit".to_owned(), "1".to_owned()),
        ];
        let env: PostsResponse = self
            .ctx
            .send_json(req, Resource::Post, Operation::Fetch)
            .await?;
        Ok(env.posts.into_iter().next())
    }

    /// `GET /posts/` — newest-updated first.
    ///
    /// # Errors
    /// Propagates transport and API errors.
    pub async fn list(&self, limit: u32, page: u32) -> Result<Vec<Post>> {
        let mut req = self.ctx.request(Method::Get, "posts/")?;
        req.query = vec![
            ("limit".to_owned(), limit.to_string()),
            ("page".to_owned(), page.to_string()),
            ("order".to_owned(), "updated_at desc".to_owned()),
        ];
        let env: PostsResponse = self
            .ctx
            .send_json(req, Resource::Post, Operation::Fetch)
            .await?;
        Ok(env.posts)
    }

    /// `GET /posts/<id>/` — one post, with HTML.
    ///
    /// # Errors
    /// Propagates transport and API errors.
    pub async fn get(&self, id: &str) -> Result<Post> {
        let mut req = self.ctx.request(Method::Get, &format!("posts/{id}/"))?;
        req.query = vec![("formats".to_owned(), "html".to_owned())];
        self.ctx
            .fetch_one::<PostsResponse>(req, Resource::Post, Operation::Fetch)
            .await
    }

    /// `POST /posts/?source=html`
    ///
    /// # Errors
    /// Propagates size, transport and API errors.
    pub async fn create(&self, input: &PostInput) -> Result<Post> {
        let body = PostsRequest {
            posts: vec![input.clone()],
        };
        Self::check_size(&body)?;
        let mut req = self.ctx.request(Method::Post, "posts/")?;
        req.query = vec![("source".to_owned(), "html".to_owned())];
        req.body = json_body(&body)?;
        self.ctx
            .fetch_one::<PostsResponse>(req, Resource::Post, Operation::Create)
            .await
    }

    /// `PUT /posts/<id>/?source=html`
    ///
    /// `updated_at` must be the value last seen from Ghost; a stale value is
    /// rejected with a 409 conflict.
    ///
    /// # Errors
    /// Propagates size, transport and API errors (including conflicts).
    pub async fn update(&self, id: &str, input: &PostInput, updated_at: &str) -> Result<Post> {
        let mut input = input.clone();
        input.updated_at = Some(updated_at.to_owned());
        let body = PostsRequest { posts: vec![input] };
        Self::check_size(&body)?;
        let mut req = self.ctx.request(Method::Put, &format!("posts/{id}/"))?;
        req.query = vec![("source".to_owned(), "html".to_owned())];
        req.body = json_body(&body)?;
        self.ctx
            .fetch_one::<PostsResponse>(req, Resource::Post, Operation::Update)
            .await
    }

    /// `DELETE /posts/<id>/` — 204 on success.
    ///
    /// # Errors
    /// Propagates transport and API errors.
    pub async fn delete(&self, id: &str) -> Result<()> {
        let req = self.ctx.request(Method::Delete, &format!("posts/{id}/"))?;
        self.ctx
            .send_unit(req, Resource::Post, Operation::Delete)
            .await
    }
}
