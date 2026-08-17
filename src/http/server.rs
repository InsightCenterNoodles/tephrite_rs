use std::{io::ErrorKind, net::TcpListener};

use super::client::HTTPClient;

use bevy::prelude::*;

#[derive(Debug, Component)]
pub struct HTTPServer {
    listener: TcpListener,
}

impl HTTPServer {
    pub fn new(bind_addr: &str) -> Result<Self> {
        let listener = TcpListener::bind(bind_addr)?;
        listener.set_nonblocking(true)?;

        Ok(Self { listener })
    }

    #[cfg(test)]
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }
}

pub(crate) fn http_service_system(servers: Query<&mut HTTPServer>, mut commands: Commands) {
    for server in servers {
        // this should not block
        loop {
            match server.listener.accept() {
                Ok((stream, _)) => {
                    let client = match HTTPClient::new(stream) {
                        Ok(x) => x,
                        Err(err) => {
                            warn!("Error creating client: {err}");
                            continue;
                        }
                    };

                    commands.spawn(client);
                }
                Err(x) => {
                    if matches!(x.kind(), ErrorKind::WouldBlock) {
                        break;
                    } else {
                        return;
                    }
                }
            }
        }
    }
}
