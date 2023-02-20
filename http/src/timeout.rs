use std::time::Duration;
use pin_project::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use std::{error, fmt};
use tower::Service;
use jsonrpc_server_utils::tokio;

#[pin_project]
#[derive(Debug)]
pub struct ResponseFuture<T> {
    #[pin]
    response: T,
    #[pin]
    sleep: tokio::time::Sleep,
}

impl<T> ResponseFuture<T> {
    pub(crate) fn new(response: T, sleep: tokio::time::Sleep) -> Self {
	ResponseFuture { response, sleep }
    }
}

impl<F, T, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<T, E>>,
    E: Into<Error>,
{
    type Output = Result<T, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
	let this = self.project();

	// First, try polling the future
	match this.response.poll(cx) {
	    Poll::Ready(v) => return Poll::Ready(v.map_err(Into::into)),
	    Poll::Pending => {}
	}

	// Now check the sleep
	match this.sleep.poll(cx) {
	    Poll::Pending => Poll::Pending,
	    Poll::Ready(_) => Poll::Ready(Err(Elapsed(()).into())),
	}
    }
}

pub(crate) type Error = Box<dyn error::Error + Send + Sync>;

/// The timeout elapsed.
#[derive(Debug)]
pub struct Elapsed(pub(super) ());

impl Elapsed {
    /// Construct a new elapsed error
    pub fn new() -> Self {
	Elapsed(())
    }
}

impl fmt::Display for Elapsed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
	f.pad("request timed out")
    }
}

impl error::Error for Elapsed {}

/// Applies a timeout to requests.
#[derive(Debug, Clone)]
pub struct Timeout<T> {
    inner: T,
    timeout: Duration,
}

impl<T> Timeout<T> {
    /// Creates a new Timeout
    pub fn new(inner: T, timeout: Duration) -> Self {
	Timeout { inner, timeout }
    }
}

impl<S, Request> Service<Request> for Timeout<S>
where
    S: Service<Request>,
    S::Error: Into<Error>,
{
    type Response = S::Response;
    type Error = Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
	match self.inner.poll_ready(cx) {
	    Poll::Pending => Poll::Pending,
	    Poll::Ready(r) => Poll::Ready(r.map_err(Into::into)),
	}
    }

    fn call(&mut self, request: Request) -> Self::Future {
	let response = self.inner.call(request);
	let sleep = tokio::time::sleep(self.timeout);
	ResponseFuture::new(response, sleep)
    }
}
