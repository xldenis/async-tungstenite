//! `gio` integration.
use tungstenite::Error;

use std::io;

use gio::prelude::*;

use futures::io::{AsyncRead, AsyncWrite};

use tungstenite::client::url_mode;
use tungstenite::stream::Mode;

use crate::{
    client_async_with_config, domain, Request, Response, WebSocketConfig, WebSocketStream,
};

type MaybeTlsStream = IOStreamAsyncReadWrite<gio::SocketConnection>;

/// Connect to a given URL.
pub async fn connect_async<R>(
    request: R,
) -> Result<(WebSocketStream<MaybeTlsStream>, Response), Error>
where
    R: Into<Request<'static>> + Unpin,
{
    connect_async_with_config(request, None).await
}

/// Connect to a given URL with a given WebSocket configuration.
pub async fn connect_async_with_config<R>(
    request: R,
    config: Option<WebSocketConfig>,
) -> Result<(WebSocketStream<MaybeTlsStream>, Response), Error>
where
    R: Into<Request<'static>> + Unpin,
{
    let request: Request = request.into();

    let domain = domain(&request)?;
    let port = request
        .url
        .port_or_known_default()
        .expect("Bug: port unknown");

    let client = gio::SocketClient::new();

    // Make sure we check domain and mode first. URL must be valid.
    let mode = url_mode(&request.url)?;
    if let Mode::Tls = mode {
        client.set_tls(true);
    } else {
        client.set_tls(false);
    }

    let connectable = gio::NetworkAddress::new(domain.as_str(), port);

    let socket = client
        .connect_async_future(&connectable)
        .await
        .map_err(|err| to_std_io_error(err))?;
    let socket = IOStreamAsyncReadWrite::new(socket)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Unsupported gio::IOStream"))?;

    client_async_with_config(request, socket, config).await
}

/// Adapter for `gio::IOStream` to provide `AsyncRead` and `AsyncWrite`.
#[derive(Debug)]
pub struct IOStreamAsyncReadWrite<T: IsA<gio::IOStream>> {
    io_stream: T,
    read: gio::InputStreamAsyncRead<gio::PollableInputStream>,
    write: gio::OutputStreamAsyncWrite<gio::PollableOutputStream>,
}

impl<T: IsA<gio::IOStream>> IOStreamAsyncReadWrite<T> {
    /// Create a new `gio::IOStream` adapter
    pub fn new(stream: T) -> Result<IOStreamAsyncReadWrite<T>, T> {
        let write = stream
            .get_output_stream()
            .and_then(|s| s.dynamic_cast::<gio::PollableOutputStream>().ok())
            .and_then(|s| s.into_async_write().ok());

        let read = stream
            .get_input_stream()
            .and_then(|s| s.dynamic_cast::<gio::PollableInputStream>().ok())
            .and_then(|s| s.into_async_read().ok());

        let (read, write) = match (read, write) {
            (Some(read), Some(write)) => (read, write),
            _ => return Err(stream),
        };

        Ok(IOStreamAsyncReadWrite {
            io_stream: stream,
            read,
            write,
        })
    }
}

use std::pin::Pin;
use std::task::{Context, Poll};

impl<T: IsA<gio::IOStream> + Unpin> AsyncRead for IOStreamAsyncReadWrite<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut Pin::get_mut(self).read).poll_read(cx, buf)
    }
}

impl<T: IsA<gio::IOStream> + Unpin> AsyncWrite for IOStreamAsyncReadWrite<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut Pin::get_mut(self).write).poll_write(cx, buf)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut Pin::get_mut(self).write).poll_close(cx)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut Pin::get_mut(self).write).poll_flush(cx)
    }
}

fn to_std_io_error(error: glib::Error) -> io::Error {
    match error.kind::<gio::IOErrorEnum>() {
        Some(io_error_enum) => io::Error::new(io_error_enum.into(), error),
        None => io::Error::new(io::ErrorKind::Other, error),
    }
}
