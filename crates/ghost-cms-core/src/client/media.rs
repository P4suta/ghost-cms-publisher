//! The media-upload resource view (images, media, arbitrary files).

use super::{ClientCtx, UploadResponse};
use crate::constants::MAX_PAYLOAD_BYTES;
use crate::domain::{FilesResponse, ImagesResponse, MediaResponse};
use crate::error::{CoreError, Operation, Resource, Result};
use crate::transport::{Body, HttpTransport, Method};

/// Binary asset uploads.
pub struct Media<'a, T: HttpTransport> {
    pub(super) ctx: &'a ClientCtx<T>,
}

impl<T: HttpTransport> Media<'_, T> {
    /// POST a multipart `file` upload to `endpoint` and return the stored URL.
    async fn upload<R: UploadResponse>(
        &self,
        endpoint: &str,
        resource: Resource,
        bytes: Vec<u8>,
        filename: &str,
        content_type: &str,
    ) -> Result<String> {
        let size = bytes.len() as u64;
        if size > MAX_PAYLOAD_BYTES {
            return Err(CoreError::TooLarge {
                size,
                limit: MAX_PAYLOAD_BYTES,
            });
        }
        let mut req = self.ctx.request(Method::Post, endpoint)?;
        req.body = Body::Multipart {
            field: "file".to_owned(),
            filename: filename.to_owned(),
            mime: content_type.to_owned(),
            bytes,
        };
        let parsed: R = self.ctx.send_json(req, resource, Operation::Upload).await?;
        parsed
            .first_url()
            .ok_or_else(|| CoreError::empty(resource, Operation::Upload))
    }

    /// `POST /images/upload/` — returns the CDN URL.
    ///
    /// # Errors
    /// Propagates size, transport and API errors.
    pub async fn upload_image(
        &self,
        bytes: Vec<u8>,
        filename: &str,
        content_type: &str,
    ) -> Result<String> {
        self.upload::<ImagesResponse>(
            "images/upload/",
            Resource::Image,
            bytes,
            filename,
            content_type,
        )
        .await
    }

    /// `POST /media/upload/` — audio/video; returns the CDN URL.
    ///
    /// # Errors
    /// Propagates size, transport and API errors.
    pub async fn upload_media(
        &self,
        bytes: Vec<u8>,
        filename: &str,
        content_type: &str,
    ) -> Result<String> {
        self.upload::<MediaResponse>(
            "media/upload/",
            Resource::Media,
            bytes,
            filename,
            content_type,
        )
        .await
    }

    /// `POST /files/upload/` — arbitrary file; returns the CDN URL.
    ///
    /// # Errors
    /// Propagates size, transport and API errors.
    pub async fn upload_file(
        &self,
        bytes: Vec<u8>,
        filename: &str,
        content_type: &str,
    ) -> Result<String> {
        self.upload::<FilesResponse>(
            "files/upload/",
            Resource::File,
            bytes,
            filename,
            content_type,
        )
        .await
    }
}
