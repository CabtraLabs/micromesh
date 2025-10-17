use macros::remote_trait;
#[remote_trait]
pub trait GatewayTrait {
    async fn ping(&self, zid: String) -> String;
}