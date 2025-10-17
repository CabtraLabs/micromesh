use macros::remote_trait;

#[remote_trait]
pub trait PingTrait {
    async fn ping(&self, zid: String) -> String;
}