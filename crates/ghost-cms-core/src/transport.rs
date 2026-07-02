//! HTTP transport abstraction.
//!
//! [`HttpTransport`] separates request construction/signing/interpretation (in
//! [`crate::client`]) from the I/O. [`ReqwestTransport`] wraps `reqwest`; tests
//! use an in-memory fake.

use std::future::Future;

use reqwest::Url;
use thiserror::Error;

/// A transport-level failure (connection, TLS, malformed multipart, …).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TransportError {
    /// The underlying `reqwest` client failed.
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
}

/// HTTP method for a [`HttpRequest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// `GET`.
    Get,
    /// `POST`.
    Post,
    /// `PUT`.
    Put,
    /// `DELETE`.
    Delete,
}

/// The body of a [`HttpRequest`].
#[derive(Debug)]
pub enum Body {
    /// No body.
    Empty,
    /// A pre-serialized JSON body.
    Json(Vec<u8>),
    /// A single-part multipart upload.
    Multipart {
        /// Form field name.
        field: String,
        /// Uploaded file name.
        filename: String,
        /// MIME content type.
        mime: String,
        /// Raw bytes.
        bytes: Vec<u8>,
    },
}

/// A fully-described, ready-to-send HTTP request.
#[derive(Debug)]
pub struct HttpRequest {
    /// HTTP method.
    pub method: Method,
    /// Absolute request URL.
    pub url: Url,
    /// Query-string parameters.
    pub query: Vec<(String, String)>,
    /// Request headers (including auth, already signed).
    pub headers: Vec<(String, String)>,
    /// Request body.
    pub body: Body,
}

/// A raw HTTP response: status plus the body bytes.
#[derive(Debug)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response body bytes.
    pub body: Vec<u8>,
}

/// Sends a fully-described [`HttpRequest`] and returns the raw response.
///
/// Implementations perform only I/O.
pub trait HttpTransport: Send + Sync {
    /// Execute a request.
    ///
    /// # Errors
    /// Returns a [`TransportError`] if the request cannot be sent or the
    /// response cannot be read.
    fn execute(
        &self,
        req: HttpRequest,
    ) -> impl Future<Output = Result<HttpResponse, TransportError>> + Send;
}

/// The default [`HttpTransport`], backed by `reqwest`.
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    http: reqwest::Client,
}

impl ReqwestTransport {
    /// Build a transport with the crate's user agent.
    ///
    /// # Errors
    /// Returns a [`TransportError`] if the `reqwest` client cannot be built.
    pub fn new() -> Result<Self, TransportError> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("ghost-cms-publisher/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { http })
    }
}

/// Test transport that always returns the same status and body.
#[cfg(test)]
pub(crate) struct MockTransport {
    status: u16,
    body: Vec<u8>,
}

#[cfg(test)]
impl MockTransport {
    /// Build a mock that always replies with `status` and `body`.
    pub(crate) fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            body: body.into(),
        }
    }
}

#[cfg(test)]
impl HttpTransport for MockTransport {
    async fn execute(&self, _req: HttpRequest) -> Result<HttpResponse, TransportError> {
        Ok(HttpResponse {
            status: self.status,
            body: self.body.clone(),
        })
    }
}

impl HttpTransport for ReqwestTransport {
    async fn execute(&self, req: HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut rb = match req.method {
            Method::Get => self.http.get(req.url),
            Method::Post => self.http.post(req.url),
            Method::Put => self.http.put(req.url),
            Method::Delete => self.http.delete(req.url),
        };
        if !req.query.is_empty() {
            rb = rb.query(&req.query);
        }
        for (name, value) in req.headers {
            rb = rb.header(name, value);
        }
        rb = match req.body {
            Body::Empty => rb,
            Body::Json(bytes) => rb
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(bytes),
            Body::Multipart {
                field,
                filename,
                mime,
                bytes,
            } => {
                let part = reqwest::multipart::Part::bytes(bytes)
                    .file_name(filename)
                    .mime_str(&mime)?;
                rb.multipart(reqwest::multipart::Form::new().part(field, part))
            },
        };
        let resp = rb.send().await?;
        let status = resp.status().as_u16();
        let body = resp.bytes().await?.to_vec();
        Ok(HttpResponse { status, body })
    }
}
