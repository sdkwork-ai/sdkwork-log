//! Body capture: tees a streamed body into a bounded buffer without changing
//! what the stream delivers.
//!
//! [`CaptureBody`] wraps a **response** body and signals completion through a
//! oneshot channel. [`RequestCaptureBody`] wraps a **request** body: it copies
//! a bounded prefix into a shared buffer while every frame continues to the
//! inner service untouched.
//!
//! The distinction matters for correctness: a request-body reader must never be
//! able to consume the payload the handler still has to read (`OBSERVABILITY`
//! capture is observational, never destructive).

use bytes::Bytes;
use http_body::{Body, Frame};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::sync::oneshot;

/// Wraps a response body, copying its bytes (up to `max_bytes`) into a shared
/// buffer while the stream is polled, and sends the captured bytes through
/// `finished` when the stream completes or errors. The response stream itself
/// is passed through unchanged.
pub struct CaptureBody<B> {
    inner: B,
    captured: Arc<Mutex<Vec<u8>>>,
    max_bytes: usize,
    finished: Option<oneshot::Sender<Vec<u8>>>,
}

impl<B> CaptureBody<B> {
    /// Wraps `inner`; the first `max_bytes` of payload are retained in
    /// `captured`, and `finished` fires once with the retained bytes.
    pub fn new(
        inner: B,
        captured: Arc<Mutex<Vec<u8>>>,
        max_bytes: usize,
        finished: oneshot::Sender<Vec<u8>>,
    ) -> Self {
        Self {
            inner,
            captured,
            max_bytes: max_bytes.max(1),
            finished: Some(finished),
        }
    }

    fn finish(&mut self) {
        if let Some(sender) = self.finished.take() {
            let _ = sender.send(self.captured.lock().expect("capture lock").to_vec());
        }
    }
}

impl<B> Body for CaptureBody<B>
where
    B: Body<Data = Bytes> + Unpin,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    let mut captured = this.captured.lock().expect("capture lock");
                    let remaining = this.max_bytes.saturating_sub(captured.len());
                    if remaining > 0 {
                        captured.extend_from_slice(&data[..data.len().min(remaining)]);
                    }
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(error))) => {
                this.finish();
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                this.finish();
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

/// Wraps a request body, copying a bounded prefix into `captured` while the
/// stream is polled. Every frame — and therefore every byte — is passed through
/// to the inner service unchanged regardless of the body's size.
///
/// A request body may be read exactly once. Any stage that observes it
/// (request logging, auditing, context building) must therefore either forward
/// the exact bytes it took or not touch the stream at all; rebuilding the
/// forwarded request from a capture is only safe when the capture is complete.
/// Forwarding the original stream through a tee keeps that true for bodies of
/// any size, including bodies with no declared length.
pub struct RequestCaptureBody<B> {
    inner: Pin<Box<B>>,
    captured: Arc<Mutex<Vec<u8>>>,
    max_bytes: usize,
}

impl<B> RequestCaptureBody<B> {
    /// Wraps `inner`; at most `max_bytes` payload bytes are retained in
    /// `captured`, and the stream itself is never altered or buffered whole.
    pub fn new(inner: B, captured: Arc<Mutex<Vec<u8>>>, max_bytes: usize) -> Self {
        Self {
            inner: Box::pin(inner),
            captured,
            max_bytes: max_bytes.max(1),
        }
    }
}

impl<B> Body for RequestCaptureBody<B>
where
    B: Body<Data = Bytes>,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        match this.inner.as_mut().poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    let mut captured = this.captured.lock().expect("capture lock");
                    let remaining = this.max_bytes.saturating_sub(captured.len());
                    if remaining > 0 {
                        captured.extend_from_slice(&data[..data.len().min(remaining)]);
                    }
                }
                Poll::Ready(Some(Ok(frame)))
            }
            // Pass errors through untouched: the caller (and the client) must
            // observe the transport's own failure, not a silently short body.
            other => other,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}
