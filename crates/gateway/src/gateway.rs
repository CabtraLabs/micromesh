
use std::sync::Arc;

use axum::{body::Bytes, debug_handler, extract::{ws::WebSocket, Path, State, WebSocketUpgrade}, response::IntoResponse};
use traits::{app::ContextTrait, gateway::{GatewayTrait, GatewayTraitRpcWrapper}};
use crate::context::AppContext;



#[derive(Clone, Debug)]
pub struct GatewaytHandler;

pub type Node = cluster::Node<GatewayTraitRpcWrapper<GatewaytHandler>>;

#[async_trait::async_trait]
impl GatewayTrait for GatewaytHandler{
    type Context = AppContext;
    async fn ping(&self, context: std::sync::Arc<Self::Context> ,zid:String) -> String {
        context.session().zid().to_string()
    } 
}

#[debug_handler]
pub async fn handler_gateway(
    State(node): State<Arc<Node>>,
    Path((service, version, query)): Path<(String, String, String)>,
    body: Bytes
) -> Result<impl IntoResponse, types::Error> {
    let req = types::ClusterRequest {
        zid: node.zid(),
        version,
        query,
        payload: body.to_vec(), 
    };
    let reply: types::ClusterResponse = node.rpc(&service, &req).await?;
    Ok(reply)
}

#[debug_handler]
pub async fn handler_websocket(
    State(state): State<Arc<Node>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(state, socket))
}

async fn handle_socket(state: Arc<Node>, socket: WebSocket) {
    
}