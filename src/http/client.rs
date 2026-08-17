use std::{io::Read, net::TcpStream, str::FromStr};

use anyhow::{Context, bail};
use bevy::prelude::*;
use thiserror::Error;

use super::resources::HTTPResources;

#[derive(Component)]
pub(crate) struct HTTPClient {
    stream: TcpStream,
    state: ClientState,
}

impl HTTPClient {
    pub(crate) fn new(stream: TcpStream) -> anyhow::Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            state: ClientState::RequestAssemblePartial(RequestAssemblePartial(vec![0u8; 4096], 0)),
        })
    }
}

pub(crate) fn http_client_service_system(world: &mut World) {
    world.resource_scope(|world, resources: Mut<HTTPResources>| {
        let mut query = world.query_filtered::<Entity, With<HTTPClient>>();
        let clients = query.iter(world).collect::<Vec<_>>();

        for client_e in clients {
            let Ok(mut entity) = world.get_entity_mut(client_e) else {
                continue;
            };
            let Some(mut client) = entity.take::<HTTPClient>() else {
                continue;
            };

            match service(&mut client, &resources, world) {
                Ok(true) => {
                    if let Ok(entity) = world.get_entity_mut(client_e) {
                        entity.despawn();
                    }
                }
                Ok(false) => {
                    if let Ok(mut entity) = world.get_entity_mut(client_e) {
                        entity.insert(client);
                    }
                }
                Err(err) => {
                    warn!("Error handling client connection: {err}");
                    if let Ok(entity) = world.get_entity_mut(client_e) {
                        entity.despawn();
                    }
                }
            }
        }
    });
}

fn service(
    client: &mut HTTPClient,
    resources: &HTTPResources,
    world: &mut World,
) -> anyhow::Result<bool> {
    client.state = match &mut client.state {
        ClientState::RequestAssemblePartial(x) => {
            handle_preamble(&mut client.stream, std::mem::take(x))?
        }
        ClientState::RequestAssembled(x) => {
            handle_assembled(&mut client.stream, std::mem::take(x))?
        }
        ClientState::RequestReady(request) => {
            handle_ready_request(&mut client.stream, resources, world, request)?
        }
        ClientState::Closed => {
            return Ok(true);
        }
    };

    Ok(matches!(client.state, ClientState::Closed))
}

#[derive(Default)]
struct RequestAssemblePartial(Vec<u8>, usize);

#[derive(Default)]
struct RequestHeaderAssembled {
    method: http::Method,
    path: String,
    offset: usize,
    content_length: usize,
    req_headers: Vec<(String, Vec<u8>)>,
    buffer: Vec<u8>,
    read_bytes: usize,
}

enum ClientState {
    RequestAssemblePartial(RequestAssemblePartial),
    RequestAssembled(RequestHeaderAssembled),
    RequestReady(http::Request<bytes::Bytes>),
    Closed,
}

fn handle_preamble(
    stream: &mut TcpStream,
    partial: RequestAssemblePartial,
) -> anyhow::Result<ClientState> {
    let RequestAssemblePartial(mut buffer, mut read_bytes) = partial;

    let n = match stream.read(&mut buffer[read_bytes..]) {
        Ok(x) => x,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::WouldBlock {
                return Ok(ClientState::RequestAssemblePartial(RequestAssemblePartial(
                    buffer, read_bytes,
                )));
            } else {
                bail!("Connection error");
            }
        }
    };

    if n == 0 {
        bail!("Connection closed prematurely by client");
    }

    read_bytes += n;

    // Parse headers using current buffer view
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req = httparse::Request::new(&mut headers);

    match req.parse(&buffer[..read_bytes])? {
        httparse::Status::Complete(offset) => {
            // Find Content-Length to determine body sizing
            let mut content_length = 0;
            for header in req.headers.iter() {
                if header.name.eq_ignore_ascii_case("Content-Length") {
                    let val_str = str::from_utf8(header.value)?;
                    content_length = val_str.trim().parse::<usize>()?;
                    break;
                }
            }

            // if request is unknown, bail

            Ok(ClientState::RequestAssembled(RequestHeaderAssembled {
                method: extract_method(req.method).context("extracting method")?,
                path: req.path.map(String::from).unwrap_or_default(),
                offset,
                req_headers: req
                    .headers
                    .iter()
                    .map(|x| (x.name.into(), x.value.to_vec()))
                    .collect(),
                content_length,
                buffer,
                read_bytes,
            }))
        }
        httparse::Status::Partial => {
            // Resize buffer if it is full but headers are still incomplete
            if read_bytes == buffer.len() {
                let new_size = (buffer.len() * 2).min(MAX_BUFFER_SIZE);
                grow_buffer(&mut buffer, new_size)?;
            }

            Ok(ClientState::RequestAssemblePartial(RequestAssemblePartial(
                buffer, read_bytes,
            )))
        }
    }
}

const MAX_BUFFER_SIZE: usize = 1_000_000;

fn grow_buffer(buffer: &mut Vec<u8>, new_size: usize) -> anyhow::Result<()> {
    anyhow::ensure!(new_size >= buffer.len(), "attempting to shrink data buffer");
    anyhow::ensure!(new_size <= MAX_BUFFER_SIZE, "buffer size limit reached");

    buffer.resize(new_size, 0);

    Ok(())
}

fn handle_assembled(
    stream: &mut TcpStream,
    assembled: RequestHeaderAssembled,
) -> anyhow::Result<ClientState> {
    let RequestHeaderAssembled {
        method,
        path,
        offset,
        content_length,
        req_headers,
        mut buffer,
        mut read_bytes,
    } = assembled;

    // Ensure the entire body length is read into the buffer
    while read_bytes < offset + content_length {
        if read_bytes == buffer.len() {
            grow_buffer(&mut buffer, offset + content_length)?;
        }

        let n = match stream.read(&mut buffer[read_bytes..]) {
            Ok(x) => x,
            Err(err) => {
                if err.kind() == std::io::ErrorKind::WouldBlock {
                    return Ok(ClientState::RequestAssembled(RequestHeaderAssembled {
                        method,
                        path,
                        offset,
                        content_length,
                        buffer,
                        req_headers,
                        read_bytes,
                    }));
                } else {
                    bail!("Connection error");
                }
            }
        };

        if n == 0 {
            bail!("Connection closed while reading body");
        }

        read_bytes += n;
    }

    // Payload
    let body_bytes = &buffer[offset..(offset + content_length)];

    let mut builder = http::Request::builder().method(method).uri(path);

    for header in req_headers {
        builder = builder.header(header.0, header.1);
    }
    let builder = builder.body(bytes::Bytes::copy_from_slice(body_bytes));

    Ok(ClientState::RequestReady(builder?))
}

fn handle_ready_request(
    stream: &mut TcpStream,
    resources: &HTTPResources,
    world: &mut World,
    request: &mut http::Request<bytes::Bytes>,
) -> anyhow::Result<ClientState> {
    let response = handle_request(request, resources, world).unwrap_or_else(error_to_response);

    write_response(response, stream)?;

    stream.shutdown(std::net::Shutdown::Both)?;

    Ok(ClientState::Closed)
}

fn extract_method(text: Option<&str>) -> anyhow::Result<http::Method> {
    let Some(text) = text else {
        bail!("Missing method");
    };

    Ok(http::Method::from_str(text)?)
}

#[derive(Debug, Error)]
enum RequestError {
    #[error("Resource is missing")]
    Missing,
    #[error("Unsupported method")]
    BadMethod,
}

fn handle_request(
    request: &mut http::Request<bytes::Bytes>,
    resources: &HTTPResources,
    world: &mut World,
) -> Result<http::Response<bytes::Bytes>, RequestError> {
    let path = request.uri().path();

    let Some(find) = resources.find(path) else {
        return Err(RequestError::Missing.into());
    };

    let method = request.method();

    debug!("Calling method {method} on {path}");

    let response = if method == http::Method::GET {
        find.on_get(world, request)
    } else if method == http::Method::POST {
        find.on_post(world, request)
    } else {
        return Err(RequestError::BadMethod.into());
    };

    let Some(response) = response else {
        return Err(RequestError::BadMethod.into());
    };

    Ok(response)
}

fn error_to_response(err: RequestError) -> http::Response<bytes::Bytes> {
    let mut builder = http::Response::builder()
        .version(http::Version::HTTP_11)
        .header("Connection", "close");

    match err {
        RequestError::Missing => {
            builder = builder.status(http::StatusCode::NOT_FOUND);
        }
        RequestError::BadMethod => {
            builder = builder.status(http::StatusCode::BAD_REQUEST);
        }
    }

    builder.body(Default::default()).unwrap()
}

fn write_response(
    mut response: http::Response<bytes::Bytes>,
    stream: &mut TcpStream,
) -> std::io::Result<()> {
    // Add/overwrite content length
    {
        let len = response.body().len();

        // Apparently HeaderValue has a From for integers.
        response.headers_mut().insert("Content-Length", len.into());
    }

    let (parts, body) = response.into_parts();

    let mut buffer = Vec::with_capacity(1024);

    use std::io::Write;

    write!(buffer, "{:?} {}\r\n", parts.version, parts.status)?;
    for (k, v) in parts.headers.iter() {
        write!(buffer, "{}: ", k)?;
        buffer.write_all(v.as_bytes())?;
        write!(buffer, "\r\n")?;
    }
    write!(buffer, "\r\n")?;

    stream.write_all(&buffer)?;
    stream.write_all(&body)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use http::HeaderValue;

    #[test]
    fn header_value() {
        let hv = HeaderValue::from(1024usize);
        let s = hv.to_str().unwrap();
        assert_eq!(s, "1024");
    }
}
