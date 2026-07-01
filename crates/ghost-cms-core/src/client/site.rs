//! The `site` resource view.

use super::ClientCtx;
use crate::domain::{SiteInfo, SiteResponse};
use crate::error::{Operation, Resource, Result};
use crate::transport::{HttpTransport, Method};

/// Read-only site metadata.
pub struct Site<'a, T: HttpTransport> {
    pub(super) ctx: &'a ClientCtx<T>,
}

impl<T: HttpTransport> Site<'_, T> {
    /// `GET /site/` — used to validate the token and identify the site.
    ///
    /// # Errors
    /// Propagates transport and API errors.
    pub async fn get(&self) -> Result<SiteInfo> {
        let req = self.ctx.request(Method::Get, "site/")?;
        let env: SiteResponse = self
            .ctx
            .send_json(req, Resource::Site, Operation::Fetch)
            .await?;
        Ok(env.site)
    }
}
