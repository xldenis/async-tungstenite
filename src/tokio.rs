//! `tokio` integration.
use tungstenite::handshake::client::Response;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::Error;

use tokio::net::TcpStream;

use super::{domain, Request, WebSocketStream};

use futures::io::{AsyncRead, AsyncWrite};

#[cfg(feature = "tokio-tls")]
pub(crate) mod tokio_tls {
    use real_tokio_tls::TlsConnector as AsyncTlsConnector;
    use real_tokio_tls::TlsStream;

    use tungstenite::client::url_mode;
    use tungstenite::stream::Mode;
    use tungstenite::Error;

    use futures::io::{AsyncRead, AsyncWrite};

    use crate::stream::Stream as StreamSwitcher;
    use crate::{
        client_async_with_config, domain, Request, Response, WebSocketConfig, WebSocketStream,
    };

    use super::TokioAdapter;

    /// A stream that might be protected with TLS.
    pub type MaybeTlsStream<S> = StreamSwitcher<S, TokioAdapter<TlsStream<TokioAdapter<S>>>>;

    pub type AutoStream<S> = MaybeTlsStream<S>;

    pub type Connector = AsyncTlsConnector;

    async fn wrap_stream<S>(
        socket: S,
        domain: String,
        connector: Option<Connector>,
        mode: Mode,
    ) -> Result<AutoStream<S>, Error>
    where
        S: 'static + AsyncRead + AsyncWrite + Unpin,
    {
        match mode {
            Mode::Plain => Ok(StreamSwitcher::Plain(socket)),
            Mode::Tls => {
                let stream = {
                    let connector = if let Some(connector) = connector {
                        connector
                    } else {
                        let connector = real_native_tls::TlsConnector::builder().build()?;
                        AsyncTlsConnector::from(connector)
                    };
                    connector
                        .connect(&domain, TokioAdapter(socket))
                        .await
                        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?
                };
                Ok(StreamSwitcher::Tls(TokioAdapter(stream)))
            }
        }
    }

    /// Creates a WebSocket handshake from a request and a stream,
    /// upgrading the stream to TLS if required and using the given
    /// connector and WebSocket configuration.
    pub async fn client_async_tls_with_connector_and_config<R, S>(
        request: R,
        stream: S,
        connector: Option<AsyncTlsConnector>,
        config: Option<WebSocketConfig>,
    ) -> Result<(WebSocketStream<AutoStream<S>>, Response), Error>
    where
        R: Into<Request<'static>> + Unpin,
        S: 'static + AsyncRead + AsyncWrite + Unpin,
        AutoStream<S>: Unpin,
    {
        let request: Request = request.into();

        let domain = domain(&request)?;

        // Make sure we check domain and mode first. URL must be valid.
        let mode = url_mode(&request.url)?;

        let stream = wrap_stream(stream, domain, connector, mode).await?;
        client_async_with_config(request, stream, config).await
    }
}

#[cfg(not(any(feature = "async-tls", feature = "tokio-tls")))]
pub(crate) mod dummy_tls {
    use futures::io::{AsyncRead, AsyncWrite};

    use tungstenite::client::url_mode;
    use tungstenite::stream::Mode;
    use tungstenite::Error;

    use crate::{
        client_async_with_config, domain, Request, Response, WebSocketConfig, WebSocketStream,
    };

    pub type AutoStream<S> = S;
    type Connector = ();

    async fn wrap_stream<S>(
        socket: S,
        _domain: String,
        _connector: Option<()>,
        mode: Mode,
    ) -> Result<AutoStream<S>, Error>
    where
        S: 'static + AsyncRead + AsyncWrite + Unpin,
    {
        match mode {
            Mode::Plain => Ok(socket),
            Mode::Tls => Err(Error::Url("TLS support not compiled in.".into())),
        }
    }

    pub(crate) async fn client_async_tls_with_connector_and_config<R, S>(
        request: R,
        stream: S,
        connector: Option<Connector>,
        config: Option<WebSocketConfig>,
    ) -> Result<(WebSocketStream<AutoStream<S>>, Response), Error>
    where
        R: Into<Request<'static>> + Unpin,
        S: 'static + AsyncRead + AsyncWrite + Unpin,
        AutoStream<S>: Unpin,
    {
        let request: Request = request.into();

        let domain = domain(&request)?;

        // Make sure we check domain and mode first. URL must be valid.
        let mode = url_mode(&request.url)?;

        let stream = wrap_stream(stream, domain, connector, mode).await?;
        client_async_with_config(request, stream, config).await
    }
}

#[cfg(not(any(feature = "async-tls", feature = "tokio-tls")))]
use self::dummy_tls::{client_async_tls_with_connector_and_config, AutoStream};

#[cfg(all(feature = "async-tls", not(feature = "tokio-tls")))]
use crate::async_tls::{client_async_tls_with_connector_and_config, AutoStream};
#[cfg(all(feature = "async-tls", not(feature = "tokio-tls")))]
type Connector = real_async_tls::TlsConnector;

#[cfg(feature = "tokio-tls")]
use self::tokio_tls::{client_async_tls_with_connector_and_config, AutoStream, Connector};

#[cfg(feature = "tokio-tls")]
/// Creates a WebSocket handshake from a request and a stream,
/// upgrading the stream to TLS if required.
pub async fn client_async_tls<R, S>(
    request: R,
    stream: S,
) -> Result<(WebSocketStream<AutoStream<S>>, Response), Error>
where
    R: Into<Request<'static>> + Unpin,
    S: 'static + AsyncRead + AsyncWrite + Unpin,
    AutoStream<S>: Unpin,
{
    client_async_tls_with_connector_and_config(request, stream, None, None).await
}

#[cfg(feature = "tokio-tls")]
/// Creates a WebSocket handshake from a request and a stream,
/// upgrading the stream to TLS if required and using the given
/// WebSocket configuration.
pub async fn client_async_tls_with_config<R, S>(
    request: R,
    stream: S,
    config: Option<WebSocketConfig>,
) -> Result<(WebSocketStream<AutoStream<S>>, Response), Error>
where
    R: Into<Request<'static>> + Unpin,
    S: 'static + AsyncRead + AsyncWrite + Unpin,
    AutoStream<S>: Unpin,
{
    client_async_tls_with_connector_and_config(request, stream, None, config).await
}

#[cfg(feature = "tokio-tls")]
/// Creates a WebSocket handshake from a request and a stream,
/// upgrading the stream to TLS if required and using the given
/// connector.
pub async fn client_async_tls_with_connector<R, S>(
    request: R,
    stream: S,
    connector: Option<Connector>,
) -> Result<(WebSocketStream<AutoStream<S>>, Response), Error>
where
    R: Into<Request<'static>> + Unpin,
    S: 'static + AsyncRead + AsyncWrite + Unpin,
    AutoStream<S>: Unpin,
{
    client_async_tls_with_connector_and_config(request, stream, connector, None).await
}

/// Connect to a given URL.
pub async fn connect_async<R>(
    request: R,
) -> Result<
    (
        WebSocketStream<AutoStream<TokioAdapter<TcpStream>>>,
        Response,
    ),
    Error,
>
where
    R: Into<Request<'static>> + Unpin,
{
    connect_async_with_config(request, None).await
}

/// Connect to a given URL with a given WebSocket configuration.
pub async fn connect_async_with_config<R>(
    request: R,
    config: Option<WebSocketConfig>,
) -> Result<
    (
        WebSocketStream<AutoStream<TokioAdapter<TcpStream>>>,
        Response,
    ),
    Error,
>
where
    R: Into<Request<'static>> + Unpin,
{
    let request: Request = request.into();

    let domain = domain(&request)?;
    let port = request
        .url
        .port_or_known_default()
        .expect("Bug: port unknown");

    let try_socket = TcpStream::connect((domain.as_str(), port)).await;
    let socket = try_socket.map_err(Error::Io)?;
    client_async_tls_with_connector_and_config(request, TokioAdapter(socket), None, config).await
}

#[cfg(any(feature = "async-tls", feature = "tokio-tls"))]
/// Connect to a given URL using the provided TLS connector.
pub async fn connect_async_with_tls_connector<R>(
    request: R,
    connector: Option<Connector>,
) -> Result<
    (
        WebSocketStream<AutoStream<TokioAdapter<TcpStream>>>,
        Response,
    ),
    Error,
>
where
    R: Into<Request<'static>> + Unpin,
{
    connect_async_with_tls_connector_and_config(request, connector, None).await
}

#[cfg(any(feature = "async-tls", feature = "tokio-tls"))]
/// Connect to a given URL using the provided TLS connector.
pub async fn connect_async_with_tls_connector_and_config<R>(
    request: R,
    connector: Option<Connector>,
    config: Option<WebSocketConfig>,
) -> Result<
    (
        WebSocketStream<AutoStream<TokioAdapter<TcpStream>>>,
        Response,
    ),
    Error,
>
where
    R: Into<Request<'static>> + Unpin,
{
    let request: Request = request.into();

    let domain = domain(&request)?;
    let port = request
        .url
        .port_or_known_default()
        .expect("Bug: port unknown");

    let try_socket = TcpStream::connect((domain.as_str(), port)).await;
    let socket = try_socket.map_err(Error::Io)?;
    client_async_tls_with_connector_and_config(request, TokioAdapter(socket), connector, config)
        .await
}

use pin_project::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Adapter for `tokio::io::AsyncRead` and `tokio::io::AsyncWrite` to provide
/// the variants from the `futures` crate and the other way around.
#[pin_project]
#[derive(Debug, Clone)]
pub struct TokioAdapter<T>(#[pin] pub T);

impl<T: tokio::io::AsyncRead> AsyncRead for TokioAdapter<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        self.project().0.poll_read(cx, buf)
    }
}

impl<T: tokio::io::AsyncWrite> AsyncWrite for TokioAdapter<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        self.project().0.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        self.project().0.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        self.project().0.poll_shutdown(cx)
    }
}

impl<T: AsyncRead> tokio::io::AsyncRead for TokioAdapter<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        self.project().0.poll_read(cx, buf)
    }
}

impl<T: AsyncWrite> tokio::io::AsyncWrite for TokioAdapter<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        self.project().0.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        self.project().0.poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        self.project().0.poll_close(cx)
    }
}
