use cluster::traits::StateTrait;
use utils::zenoh;

#[derive(Clone)]
pub struct AppState {
   session: utils::zenoh::Session,
}

impl AppState {
    pub async fn new() -> Self {
        Self { 
            session: utils::zenoh_zession::create_session().await,
        }
    }
}

impl StateTrait for AppState {
    fn session(&self) -> &zenoh::Session {
        &self.session
    }
}