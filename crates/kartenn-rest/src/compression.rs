//! Response compression (REQ-006-32; layer 10): `zstd`, `br` or `gzip` after
//! `Accept-Encoding`, above 1 KiB, never for a response that shows a secret nor for the
//! event stream.

use axum::http::{HeaderValue, header};
use axum::response::{IntoResponseParts, ResponseParts};
use tower_http::compression::predicate::{NotForContentType, Predicate, SizeAbove};
use tower_http::compression::{CompressionLayer, CompressionLevel};

/// Compressed from this size up: more than 1 KiB.
pub const MIN_SIZE: u64 = 1025;

/// Marks a response that shows a secret (a service account created, a secret added): it
/// is never compressed and carries `Cache-Control: no-store`. A route returns it among
/// its response parts, `(Secret, Json(body))`.
#[derive(Clone, Copy, Debug)]
pub struct Secret;

impl IntoResponseParts for Secret {
    type Error = std::convert::Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        res.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        res.extensions_mut().insert(self);
        Ok(res)
    }
}

/// True unless the response carries [`Secret`].
#[derive(Clone, Copy, Debug)]
struct NotSecret;

impl Predicate for NotSecret {
    fn should_compress<B>(&self, response: &axum::http::Response<B>) -> bool
    where
        B: axum::body::HttpBody,
    {
        response.extensions().get::<Secret>().is_none()
    }
}

/// The compression layer, innermost (the route's response first).
pub fn layer() -> CompressionLayer<impl Predicate> {
    CompressionLayer::new()
        .zstd(true)
        .br(true)
        .gzip(true)
        .no_deflate()
        .quality(CompressionLevel::Default)
        .compress_when(
            SizeAbove::new(MIN_SIZE)
                .and(NotSecret)
                // The event stream is never compressed (spec 022, REQ-022-13).
                .and(NotForContentType::SSE),
        )
}
