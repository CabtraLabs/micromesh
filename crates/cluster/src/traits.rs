use std::sync::Arc;

use types::{ClusterRequest, ClusterResponse};

pub trait StateTrait: Sized + Clone {
    fn session(&self) -> &zenoh::Session;
}

pub trait ServiceHandlerTrait: Sized + Clone {
    type State: StateTrait + Send + Unpin + Sync + Clone;
    fn name(&self) -> &str;
    fn handle_rpc(&self, state: Arc<Self::State>, req: &ClusterRequest) -> impl std::future::Future<Output = types::Result<ClusterResponse>> + Send {
        tracing::warn!("{}:{} {} unimplemented {req:?}", file!(), line!(), state.session().zid());
        async move {
            Ok(ClusterResponse{
                zid: state.session().zid().to_string(),
                status: 200,
                payload: Some(b"cluster handle_rpc handler not implemented!".to_vec()),
            })
        }
    }
    fn handle_push(&self, state: Arc<Self::State>, req: &ClusterRequest) -> impl std::future::Future<Output = types::Result<()>> + Send  {
        tracing::warn!("{}:{} {} unimplemented {req:?}", file!(), line!(), state.session().zid());
        async move {
            Ok(())
        }
    }
}