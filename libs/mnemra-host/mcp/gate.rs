//! The initialize gate: no request is served before the `initialize` handshake (#4295).
//!
//! rmcp 3.x lets a first message that is not `initialize` select an inline
//! (stateless) lifecycle from its own `_meta`, and serves it with no handshake.
//! rmcp has no switch to refuse that, but it records the peer's handshake info
//! only on the `initialize` paths: `Peer::peer_info()` stays `None` on the
//! inline path. [`InitializeGated`] reads that as "not initialized" and, until
//! the handshake has happened, allows only `initialize` and `ping`.
//!
//! The rule is an allowlist and fails closed: a request kind rmcp adds later is
//! refused before the handshake rather than served. Everything allowed is
//! delegated unchanged, so the post-handshake wire behaviour is the handler's.
//!
//! `initialize` itself is allowed exactly once, and only on rmcp's handshake
//! path. rmcp records the peer's handshake info before it dispatches the
//! handshake's `initialize`, so an `initialize` that reaches the gate with no
//! peer info is a late one, behind a request that already put the connection in
//! rmcp's inline mode (a latch rmcp never clears): it is refused. A second
//! `initialize` on an established session is refused too, because the default
//! handler would replace the stored handshake info with the client's raw request.

use rmcp::RoleServer;
use rmcp::model::ServerResult;
use rmcp::model::{ClientNotification, ClientRequest, ErrorData, ProtocolVersion, ServerConfig};
use rmcp::service::{
    NotificationContext, RequestContext, RunningService, ServerInitializeError, Service,
};
use rmcp::transport::IntoTransport;
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::mcp::server::MnemraMcpServer;

/// [`MnemraMcpServer`] behind the initialize gate. Build it through [`serve`].
pub struct InitializeGated {
    server: MnemraMcpServer,
    /// Set when the first `initialize` is forwarded; any later one is refused.
    initialize_forwarded: AtomicBool,
}

/// Serve `server` over `transport` behind the initialize gate.
///
/// The one place a host MCP session is started: production (`serve_stdio`) and
/// the tests of the gate and the transport cap both go through it, so they
/// exercise the same gated service. The error is boxed: rmcp's is large.
pub async fn serve<T, E, A>(
    server: MnemraMcpServer,
    transport: T,
) -> Result<RunningService<RoleServer, InitializeGated>, Box<ServerInitializeError>>
where
    T: IntoTransport<RoleServer, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    let gated = InitializeGated {
        server,
        initialize_forwarded: AtomicBool::new(false),
    };
    rmcp::service::serve_server(gated, transport)
        .await
        .map_err(Box::new)
}

impl Service<RoleServer> for InitializeGated {
    async fn handle_request(
        &self,
        request: ClientRequest,
        context: RequestContext<RoleServer>,
    ) -> Result<ServerResult, ErrorData> {
        let initialized = context.peer.peer_info().is_some();
        match request {
            // Only the handshake path has peer info set when its `initialize`
            // arrives, so one that finds none is late.
            ClientRequest::InitializeRequest(_) if !initialized => {
                return Err(ErrorData::invalid_request("initialize required", None));
            }
            ClientRequest::InitializeRequest(_)
                if self.initialize_forwarded.swap(true, Ordering::AcqRel) =>
            {
                return Err(ErrorData::invalid_request("already initialized", None));
            }
            ClientRequest::InitializeRequest(_) | ClientRequest::PingRequest(_) => {}
            _ if !initialized => {
                return Err(ErrorData::invalid_request("initialize required", None));
            }
            _ => {}
        }
        Service::<RoleServer>::handle_request(&self.server, request, context).await
    }

    async fn handle_notification(
        &self,
        notification: ClientNotification,
        context: NotificationContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        Service::<RoleServer>::handle_notification(&self.server, notification, context).await
    }

    fn get_info(&self) -> ServerConfig {
        Service::<RoleServer>::get_info(&self.server)
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Service::<RoleServer>::supported_protocol_versions(&self.server)
    }
}
