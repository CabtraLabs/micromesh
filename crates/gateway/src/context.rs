use traits::app::ContextTrait;
use utils::zenoh;

#[derive(Clone)]
pub struct AppContext {
   s: utils::zenoh::Session,
}

impl AppContext {
    pub async fn new() -> Self {
        Self { 
            s: utils::zenoh_zession::create_session().await,
        }
    }
}

impl ContextTrait for AppContext {
    fn session(&self) -> &zenoh::Session {
        &self.s
    }
}