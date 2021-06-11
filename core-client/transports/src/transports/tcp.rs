//! JSON-RPC IPC client implementation using Unix Domain Sockets on UNIX-likes
//! and Named Pipes on Windows.

use crate::transports::duplex::duplex;
use crate::{RpcChannel, RpcError};
use futures::{SinkExt, StreamExt, TryStreamExt};
use jsonrpc_server_utils::codecs::StreamCodec;
use jsonrpc_server_utils::tokio;
use jsonrpc_server_utils::tokio_util::codec::Decoder as _;
use std::net::SocketAddr;

/// Connect to a JSON-RPC IPC server.
pub async fn connect<Client: From<RpcChannel>>(path: &SocketAddr) -> Result<Client, RpcError> {
	let connection = tokio::net::TcpStream::connect(path)
		.await
		.map_err(|e| RpcError::Other(Box::new(e)))?;
	let (sink, stream) = StreamCodec::stream_incoming().framed(connection).split();
	let sink = sink.sink_map_err(|e| RpcError::Other(Box::new(e)));
	let stream = stream.map_err(|e| log::error!("TCP stream error: {}", e));
	let (client, sender) = duplex(
		Box::pin(sink),
		Box::pin(
			stream
				.take_while(|x| futures::future::ready(x.is_ok()))
				.map(|x| x.expect("Stream is closed upon first error.")),
		),
	);
	tokio::spawn(client);
	Ok(sender.into())
}
