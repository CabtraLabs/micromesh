use std::str::FromStr;

use serde_json::json;

pub async fn create_session() -> zenoh::Session {
    let config = match zenoh::Config::from_env() {
        Ok(v) => v,
        Err(_) => {
            let mut config = zenoh::Config::default();
            if let Ok(mode) = std::env::var("ZENOH_MODE") {
                let mode = match zenoh::config::WhatAmI::from_str(&mode) {
                    Ok(v) => v,
                    Err(_) => zenoh::config::WhatAmI::Peer,
                };

                if let Err(e) = config.insert_json5("mode", &json!(mode).to_string()) {
                    tracing::error!("{}:{} {}", file!(), line!(), e);
                }
            }

            if let Ok(connect) = std::env::var("ZENOH_CONNECT") {
                let connect: Vec<String> = connect.split(",").map(|s| s.to_string()).collect();
                if let Err(e) =
                    config.insert_json5("connect/endpoints", &json!(connect).to_string())
                {
                    tracing::error!("{}:{} {}", file!(), line!(), e);
                }
            }
            if let Ok(listen) = std::env::var("ZENOH_LISTEN") {
                let listen: Vec<String> = listen.split(",").map(|s| s.to_string()).collect();
                if let Err(e) = config.insert_json5("listen/endpoints", &json!(listen).to_string())
                {
                    tracing::error!("{}:{} {}", file!(), line!(), e);
                }
            }
            if let Ok(is_closed) = std::env::var("ZENOH_NO_MULTICAST_SCOUTING") {
                let is_closed: i8 = is_closed.parse().unwrap_or_default();
                if let Err(e) = config.insert_json5(
                    "scouting/multicast/enabled",
                    &json!(is_closed == 0).to_string(),
                ) {
                    tracing::error!("{}:{} {}", file!(), line!(), e);
                }
            }

            if let Ok(is_closed) = std::env::var("ZENOH_NO_GOSSIP_SCOUTING") {
                let is_closed: i8 = is_closed.parse().unwrap_or_default();
                if let Err(e) = config.insert_json5(
                    "scouting/gossip/enabled",
                    &json!(is_closed == 0).to_string(),
                ) {
                    tracing::error!("{}:{} {}", file!(), line!(), e);
                }
            }

            if let Ok(links) = std::env::var("ZENOH_UNICAST_MAX_LINKS") {
                let links: i32 = links.parse().unwrap_or(255);
                if let Err(e) =
                    config.insert_json5("transport/unicast/max_links", &json!(links).to_string())
                {
                    tracing::error!("{}:{} {}", file!(), line!(), e);
                }
            }

            if let Ok(is_open) = std::env::var("ZENOH_ENABLE_SHM") {
                let is_open: i8 = is_open.parse().unwrap_or_default();
                if let Err(e) = config.insert_json5(
                    "transport/shared_memory/enabled",
                    &json!(is_open).to_string(),
                ) {
                    tracing::error!("{}:{} {}", file!(), line!(), e);
                }
            }
            config
        }
    };
    tracing::info!("[cluster] start service with config: {}", config);

    match zenoh::open(config).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("{}:{} {}", file!(), line!(), e);
            std::process::exit(crate::EXIT_START_NODE_ERROR);
        }
    }
}
