pub fn get_env_var<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|val| val.parse::<T>().ok())
        .unwrap_or(default)
}

pub fn get_server_bind()-> String {
    get_env_var("SERVER_BIND", "0.0.0.0:8080".to_string())
}

pub fn get_allow_origins()-> String {
    get_env_var("SERVER_ALLOW_ORIGINS", "*".to_string()).replace(";", " ").replace(",", " ")
}

pub fn get_jwt_duration()-> i64 {
    get_env_var("ACCESS_TOKEN_DURATION", 3600)
}

pub fn get_server_id() -> Option<i64> {
    std::env::var("SERVER_ID")
        .ok()
        .and_then(|val| val.parse::<i64>().ok())
}


