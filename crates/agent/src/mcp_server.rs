use std::io;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::agent_context::AgentContext;
use crate::mcp_connection::McpConnection;
use crate::mcp_session::McpSession;
use crate::private_socket::PrivateSocket;

#[derive(Debug)]
pub(crate) struct McpServer<'a> {
    socket: PrivateSocket,
    connection: Option<McpConnection>,
    session: McpSession<'a>,
}

impl<'a> McpServer<'a> {
    pub(crate) fn bind(context: &'a AgentContext<'a>) -> io::Result<Self> {
        Ok(Self {
            socket: PrivateSocket::bind()?,
            connection: None,
            session: McpSession::new(context),
        })
    }

    pub(crate) fn socket_path(&self) -> &Path {
        self.socket.path()
    }

    pub(crate) fn session(&self) -> &McpSession<'a> {
        &self.session
    }

    pub(crate) fn into_session(self) -> McpSession<'a> {
        self.session
    }

    pub(crate) fn serve_for(&mut self, wait: Duration) -> io::Result<()> {
        match &mut self.connection {
            Some(connection) => {
                if !connection.serve(&mut self.session)? {
                    self.connection = None;
                }
            }
            None => match self.socket.accept()? {
                Some(stream) => self.connection = Some(McpConnection::new(stream, wait)?),
                None => thread::sleep(wait),
            },
        }
        Ok(())
    }
}
