pub trait ContextTrait: Sized {
    fn session(&self) -> &zenoh::Session;
}

#[async_trait::async_trait]
pub trait RpcTrait: Sized + Clone {
    type Context: ContextTrait + Send + Unpin + Sync + 'static;
    type Params: bitcode::Encode + bitcode::DecodeOwned + Send + Unpin + Sync + 'static;
    type Result: bitcode::Encode + bitcode::DecodeOwned + Send + Unpin + Sync + 'static;
    fn name(&self) -> &str;
    async fn rpc_call(&self,context: std::sync::Arc<Self::Context>, params: Self::Params) -> Self::Result;
}