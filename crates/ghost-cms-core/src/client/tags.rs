//! The `tags` resource view.

use super::{ClientCtx, json_body};
use crate::domain::{Tag, TagUpsertInput, TagsRequest, TagsResponse};
use crate::error::{Operation, Resource, Result};
use crate::transport::{HttpTransport, Method};

/// Operations on Ghost tags.
pub struct Tags<'a, T: HttpTransport> {
    pub(super) ctx: &'a ClientCtx<T>,
}

impl<T: HttpTransport> Tags<'_, T> {
    /// `GET /tags/` — list tags, newest-updated first, with post counts.
    ///
    /// # Errors
    /// Propagates transport and API errors.
    pub async fn list(&self, limit: u32, page: u32) -> Result<Vec<Tag>> {
        let mut req = self.ctx.request(Method::Get, "tags/")?;
        req.query = vec![
            ("limit".to_owned(), limit.to_string()),
            ("page".to_owned(), page.to_string()),
            ("include".to_owned(), "count.posts".to_owned()),
            ("order".to_owned(), "updated_at desc".to_owned()),
        ];
        let env: TagsResponse = self
            .ctx
            .send_json(req, Resource::Tag, Operation::Fetch)
            .await?;
        Ok(env.tags)
    }

    /// `GET /tags/?filter=slug:<slug>` — fetch a single tag by slug, if any.
    ///
    /// # Errors
    /// Propagates transport and API errors.
    pub async fn find_by_slug(&self, slug: &str) -> Result<Option<Tag>> {
        let mut req = self.ctx.request(Method::Get, "tags/")?;
        req.query = vec![
            ("filter".to_owned(), format!("slug:{slug}")),
            ("include".to_owned(), "count.posts".to_owned()),
            ("limit".to_owned(), "1".to_owned()),
        ];
        let env: TagsResponse = self
            .ctx
            .send_json(req, Resource::Tag, Operation::Fetch)
            .await?;
        Ok(env.tags.into_iter().next())
    }

    /// `POST /tags/` — create a tag.
    ///
    /// # Errors
    /// Propagates transport and API errors.
    pub async fn create(&self, input: &TagUpsertInput) -> Result<Tag> {
        let body = TagsRequest {
            tags: vec![input.clone()],
        };
        let mut req = self.ctx.request(Method::Post, "tags/")?;
        req.body = json_body(&body)?;
        self.ctx
            .fetch_one::<TagsResponse>(req, Resource::Tag, Operation::Create)
            .await
    }

    /// `PUT /tags/<id>/` — update a tag (with `updated_at` conflict detection).
    ///
    /// # Errors
    /// Propagates transport and API errors (including conflicts).
    pub async fn update(&self, id: &str, input: &TagUpsertInput, updated_at: &str) -> Result<Tag> {
        let mut input = input.clone();
        input.updated_at = Some(updated_at.to_owned());
        let body = TagsRequest { tags: vec![input] };
        let mut req = self.ctx.request(Method::Put, &format!("tags/{id}/"))?;
        req.body = json_body(&body)?;
        self.ctx
            .fetch_one::<TagsResponse>(req, Resource::Tag, Operation::Update)
            .await
    }

    /// `DELETE /tags/<id>/` — delete a tag.
    ///
    /// # Errors
    /// Propagates transport and API errors.
    pub async fn delete(&self, id: &str) -> Result<()> {
        let req = self.ctx.request(Method::Delete, &format!("tags/{id}/"))?;
        self.ctx
            .send_unit(req, Resource::Tag, Operation::Delete)
            .await
    }
}
