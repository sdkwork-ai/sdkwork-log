//! Response body capture: tees the streamed body into a bounded buffer and
//! signals completion through a oneshot channel.

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
