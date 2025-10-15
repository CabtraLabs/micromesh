
use std::sync::Arc;

use axum::{body::Bytes, debug_handler, extract::{ws::WebSocket, Path, State, WebSocketUpgrade}, response::IntoResponse};
use cluster::traits::StateTrait;
use types::{ClusterRequest, ClusterResponse, ERROR_CODE_RPC_NOT_IMPLEMENTED};

use crate::state::AppState;


#[derive(Clone, Debug)]
pub struct GatewaytHandler;

pub type Node = cluster::Node<GatewaytHandler>;

impl cluster::traits::ServiceHandlerTrait for GatewaytHandler{
    type State = AppState;

    fn name(&self) -> &str {
        "gateway"
    }

    async fn handle_rpc(&self, state: Arc<Self::State>, req: &ClusterRequest) -> types::Result<ClusterResponse> {
        Ok(ClusterResponse{
            zid: state.session().zid().to_string(),
            status: 200,
            payload: Some(
                serde_json::to_vec(&ERROR_CODE_RPC_NOT_IMPLEMENTED).unwrap_or_default()
            ),
        })
    }
    async fn handle_push(&self, state: Arc<Self::State>, query: &ClusterRequest) -> types::Result<()> {
        Ok(())
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