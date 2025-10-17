// External crate imports
use types::{ClusterRequest, ClusterResponse};
use std::{path::Path, str::FromStr, sync::Arc};
use tokio_util::sync::{CancellationToken, DropGuard};
use utils::{round_robin::RoundRobinDashMap, vars::get_env_var};
use traits::app::{RpcTrait, ContextTrait};
use zenoh::{config::ZenohId, query::QueryTarget};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Node represents a service node in the cluster
/// It handles RPC calls and pub/sub messages using the Zenoh protocol
pub struct NodeInner<H: RpcTrait> {
    handler: H,
    context: Arc<H::Context>,
    services: RoundRobinDashMap<ZenohId>,
    rpc_timeout: u64,
}

impl<H> NodeInner<H>
where
    H: RpcTrait + Send + Sync + 'static,
{
    /// Updates the internal service registry based on liveliness updates
    /// Called when service status changes are detected
    fn sync_service(&self, online: &zenoh::sample::Sample) {
        if let Some((service, zid)) = extract_server_and_name(online.key_expr()) {
            match online.kind() {
                zenoh::sample::SampleKind::Put => {
                    self.services.insert(service, zid);
                }
                zenoh::sample::SampleKind::Delete => {
                    self.services.remove(service, zid);
                }
            }
        }
    }
}

pub struct Node<H: RpcTrait> {
    inner: Arc<NodeInner<H>>,
    _guard: DropGuard,
}

/// Extracts the service name and ZenohId from a path string
/// Returns a tuple of (service_name, ZenohId) if successful
fn extract_server_and_name(path_str: &str) -> Option<(String, ZenohId)> {
    let path = Path::new(path_str);
    let components: Vec<_> = path.iter().collect();

    if components.len() >= 3 {
        let service_name = components[components.len() - 2].to_str()?.to_string();
        let zid_str = components[components.len() - 1].to_str()?.to_string();
        let zid = match ZenohId::from_str(&zid_str) {
            Ok(v) => v,
            Err(_) => {
                tracing::error!("{}:{} Invalid zid {zid_str}", file!(), line!());
                return None;
            }
        };
        Some((service_name, zid))
    } else {
        None
    }
}

impl<H> Node<H>
where
    H: RpcTrait + Send + Sync + 'static,
{
    /// Creates a new Node instance with the given service handler
    /// Initializes Zenoh configuration from environment variables
    pub async fn new(context: Arc<H::Context>, handler: H) -> Self {
        let rpc_timeout = get_env_var("ZENOH_RPC_TIMEOUT", 10 * 1000);
        let shutdown_token = CancellationToken::new();
        let task_token = shutdown_token.clone();
        let _guard = shutdown_token.drop_guard();
        let inner =  Arc::new(NodeInner {
            handler,
            context,
            rpc_timeout,
            services: RoundRobinDashMap::default(),
        });
        tokio::spawn(Self::run(inner.clone(), task_token));
        Self {
            inner,
            _guard
        }
    }

    /// Starts the node and handles incoming requests
    /// - Declares RPC endpoint
    /// - Sets up pub/sub channels
    /// - Manages service liveliness
    /// - Handles shutdown gracefully
    async fn run(inner: Arc<NodeInner<H>>, shutdown_token: CancellationToken) {
        let zid = inner.context.session().zid();
        let service = inner.handler.name();
        let rpc = match inner.context.session()
            .declare_queryable(format!("@rpc/{service}/{zid}"))
            // // By default queryable receives queries from a FIFO.
            // // Uncomment this line to use a ring channel instead.
            // .with(zenoh::handlers::RingChannel::default())
            .complete(true)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("{}:{} {}", file!(), line!(), e);
                std::process::exit(utils::EXIT_START_NODE_ERROR);
            }
        };

        let token = match inner.context.session()
            .liveliness()
            .declare_token(format!("@live/{service}/{zid}"))
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("{}:{} {}", file!(), line!(), e);
                std::process::exit(utils::EXIT_START_NODE_ERROR);
            }
        };

        let liveliness_key = "@live/**";

        let liveliness = match inner.context.session()
            .liveliness()
            .declare_subscriber(liveliness_key)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("{}:{} {}", file!(), line!(), e);
                std::process::exit(utils::EXIT_START_NODE_ERROR);
            }
        };

        let replies = match inner.context.session().liveliness().get(liveliness_key).await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("{}:{} {}", file!(), line!(), e);
                std::process::exit(utils::EXIT_START_NODE_ERROR);
            }
        };
        while let Ok(reply) = replies.recv_async().await {
            match reply.result() {
                Ok(online) => {
                    inner.sync_service(online);
                }
                Err(e) => {
                    tracing::error!("{}:{} {e:?}", file!(), line!());
                    continue;
                }
            }
        }

        loop {
            tokio::select! {
                _ = shutdown_token.cancelled() => {
                    tracing::info!("[cluster] {} node stopped", inner.context.session().zid());
                    break;
                },

                online = liveliness.recv_async() => {
                    if let Err(e) = online {
                        tracing::error!("{}:{} {}", file!(), line!(), e);
                        continue;
                    }
                    inner.sync_service(&online.unwrap());
                },

                rpc = rpc.recv_async()=> {
                    let handler = inner.handler.clone();
                    let context = inner.context.clone();
                    tokio::spawn(async move {
                        if let Err(e) = rpc {
                            tracing::error!("{}:{} {}", file!(), line!(), e);
                            return;
                        }
                        let rpc = rpc.unwrap();
                        let key_expr = rpc.key_expr();
                        match rpc.payload(){
                            Some(payload) => {
                                let payload = payload.to_bytes();
                                let req = match bitcode::decode(&payload) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        tracing::error!("{}:{} {}", file!(), line!(), e);
                                        let error: types::Error = types::ERROR_CODE_INTERNAL_ERROR.into();
                                        let bytes = bitcode::encode(&error);
                                        if let Err(e) = rpc.reply_err(&bytes).await {
                                            tracing::error!("{}:{} {}", file!(), line!(), e);
                                        }
                                        return;
                                    }
                                };
                                let result = handler.rpc_call(context, req).await;
                                let bytes = bitcode::encode(&result);
                                if let Err(e) = rpc.reply(key_expr.clone(), &bytes).await {
                                    tracing::error!("{}:{} {}", file!(), line!(), e);
                                }
                            },
                            None => {
                                tracing::error!("{}:{} Invalid request data of rpc", file!(), line!());
                                let e: types::Error = types::ERROR_CODE_INTERNAL_ERROR.into();
                                let bytes = bitcode::encode(&e);
                                if let Err(e) = rpc.reply_err(&bytes).await {
                                    tracing::error!("{}:{} {}", file!(), line!(), e);
                                }
                            },
                        };
                    });
                },
            }
        }
        if let Err(e) = token.undeclare().await {
            tracing::error!("{}:{} {}", file!(), line!(), e);
        }
    }

    pub async fn rpc(
        &self,
        service: &str,
        request: &ClusterRequest,
    ) -> types::Result<ClusterResponse> {
        let zid = self.inner
            .services
            .get_round_robin(service)
            .ok_or_else(|| { let error: types::Error = types::ERROR_CODE_SERVICE_NOT_FOUND.into(); error})?;

        let payload = bitcode::encode(request);

        let replies = match self.inner.context.session()
            .get(format!("@rpc/{service}/{zid}"))
            .payload(&payload)
            .target(QueryTarget::BestMatching)
            .timeout(std::time::Duration::from_millis(self.inner.rpc_timeout))
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("{}:{} {}", file!(), line!(), e);
                return Err(types::ERROR_CODE_INTERNAL_ERROR.into());
            }
        };
        match replies.recv_async().await {
            Ok(reply) => match reply.result() {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes();
                    bitcode::decode(&payload).map_err(|e| {
                        tracing::error!("{}:{} {}", file!(), line!(), e);
                        types::ERROR_CODE_INTERNAL_ERROR.into()
                    })
                }
                Err(err) => {
                    let payload = err.payload().to_bytes();
                    match bitcode::decode(&payload){
                        Ok(v) => Err(v),
                            Err(e) => {
                            tracing::error!("{}:{} {}", file!(), line!(), e);
                            Err(types::ERROR_CODE_INTERNAL_ERROR.into())
                        }
                    }
                }
            },
            Err(_) => Err(types::ERROR_CODE_RPC_TIMEOUT.into()),
        }
    }

    pub async fn push(
        &self,
        service: &str,
        request: &ClusterRequest,
    ) -> types::Result<()> {
        let zid = self.inner
            .services
            .get_round_robin(service)
            .ok_or_else(|| {let error: types::Error = types::ERROR_CODE_SERVICE_NOT_FOUND.into(); error})?;
        let payload = bitcode::encode(request);
        self.inner.context.session()
            .put(format!("@chl/{service}/{zid}"), &payload)
            .await.map_err(|e|{
                tracing::error!("{}:{} {}", file!(), line!(), e);
                let error: types::Error = types::ERROR_CODE_SERVICE_NOT_FOUND.into(); 
                error
            })
    }

    pub fn zid(&self) -> String {
        self.inner.context.session().zid().to_string()
    }
}

#[cfg(test)]
mod tests {
    use traits::test::{PingTraitRpcWrapper, PingTrait};

    use super::*;
    use std::time::Duration;

    #[derive(Clone)]
    pub struct AppContext {
        session: utils::zenoh::Session,
    }

    impl AppContext {
        pub async fn new() -> Self {
            Self { 
                session: utils::zenoh_zession::create_session().await,
            }
        }
    }

    impl traits::app::ContextTrait for AppContext {
        fn session(&self) -> &zenoh::Session {
            &self.session
        }
    }

    #[derive(Clone)]
    struct PingHandler{
        id: i32,
    }

    #[async_trait::async_trait]
    impl PingTrait for PingHandler {
        type Context = AppContext;
        async fn ping(&self,context: std::sync::Arc<Self::Context> , _zid:String) -> String {
           "Pong".to_string()
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_ping_pong() {
        unsafe {std::env::set_var("RUST_LOG", "info")};
        // Start server node
        utils::setup_env();

        let state1 = Arc::new(AppContext::new().await);
        let state2 = Arc::new(AppContext::new().await);
        let state3 = Arc::new(AppContext::new().await);

        let node1 = Node::new(state1.clone(), PingTraitRpcWrapper(PingHandler{id: 1})).await;
        let node2 =  Node::new(state2.clone(),PingTraitRpcWrapper(PingHandler{id: 2})).await;
        let node3 =  Node::new(state3.clone(),PingTraitRpcWrapper(PingHandler{id: 3})).await;

        // Wait for nodes to initialize
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Make RPC call
        for _ in 0..100 {
            let request = ClusterRequest{
                zid: state3.session.zid().to_string(), 
                query: "test".to_string(), 
                version: "".to_string(), 
                payload: b"Ping".to_vec(),
            };
            let instant = tokio::time::Instant::now();
            let response = node3.rpc("ping_service", &request).await;
            tracing::info!("elapsed: {:?}", instant.elapsed());
            assert!(response.is_ok());
            assert_eq!(response.unwrap().payload.unwrap(),  b"Pong".to_vec());
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }


        // Make push
        for _ in 0..100 {
            let request = ClusterRequest{
                zid: state3.session.zid().to_string(), 
                version: "".to_string(), 
                query: "test".to_string(), 
                payload: b"Test".to_vec(),
            };
            let instant = tokio::time::Instant::now();
            let response = node3.push("ping_service", &request).await;
            tracing::info!("elapsed: {:?}", instant.elapsed());
            assert!(response.is_ok());
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        drop(node1);
        drop(node2);
        drop(node3);
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    #[test]
    fn test_extract_server_and_name() {
        let path = "@live/test_service/0123456789ABCDEF";
        let result = extract_server_and_name(path);
        assert!(result.is_none());

        let zid = ZenohId::default();
        let path = format!("@live/test_service/{zid}");
        let result = extract_server_and_name(&path);
        assert!(result.is_some());

        let (service, _zid) = result.unwrap();
        assert_eq!(service, "test_service");
    }
}
