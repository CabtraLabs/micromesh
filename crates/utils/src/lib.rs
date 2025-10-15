use chrono::{Datelike, TimeZone};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use crate::vars::get_env_var;
pub use zenoh;

pub mod vars;
pub mod round_robin;
pub mod xid;
pub mod jwt;
pub mod snowflake;
pub mod zenoh_zession;

pub const EXIT_OK: i32 = 0;
pub const EXIT_START_NODE_ERROR: i32 = 10;

pub fn get_tz() -> String {
    get_env_var("SERVICE_TZ", "Asia/Tokyo".to_string())
}

/// Get the UNIX timestamp (in seconds) for the start of "today"
/// in the time zone specified by the `TZ` environment variable.
/// Defaults to Asia/Tokyo if the environment variable is not set.
pub fn start_of_today() -> i64 {
    // Read the time zone name from the "TZ" environment variable
    // Use "Asia/Tokyo" as the default if not set
    let tz_name = get_tz();

    // Parse the time zone name into a `chrono_tz::Tz` type
    let tz: chrono_tz::Tz = tz_name.parse().unwrap_or(chrono_tz::Tz::Asia__Tokyo);

    // Get the current time in UTC
    let now_utc =  chrono::Utc::now();

    // Convert UTC time to the specified time zone
    let now_in_tz = now_utc.with_timezone(&tz);

    // Extract the year/month/day in the target time zone (without time)
    let date = now_in_tz.date_naive();

    // Build the DateTime at 00:00:00 (start of day) in the specified time zone
    match tz.with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0){
        chrono::offset::LocalResult::Single(v) => v.timestamp(),
        chrono::offset::LocalResult::Ambiguous(_, v2) => v2.timestamp(),
        chrono::offset::LocalResult::None => chrono::Local::now().timestamp(),
    }
}


/// Get the datetime string from a timestamp
/// in the time zone specified by the `TZ` environment variable.
/// Defaults to Asia/Tokyo if the environment variable is not set.
pub fn get_local_datetime_formarted(timestamp: i64) -> String {
    // Read the time zone name from the "TZ" environment variable
    // Use "Asia/Tokyo" as the default if not set
    let tz_name = get_tz();

    // Parse the time zone name into a `chrono_tz::Tz` type
    let tz: chrono_tz::Tz = tz_name.parse().unwrap_or(chrono_tz::Tz::Asia__Tokyo);

    // Get the current time in UTC
    let now_utc =  chrono::DateTime::from_timestamp(timestamp, 0).unwrap_or_default();

    // Convert UTC time to the specified time zone
    let now_in_tz = now_utc.with_timezone(&tz);

    // Extract the year/month/day in the target time zone (without time)
    format!("{}", now_in_tz.format("%Y-%m-%d %H:%M:%S"))
}

/// Get the datetime string from a timestamp
/// in the time zone specified by the `TZ` environment variable.
/// Defaults to Asia/Tokyo if the environment variable is not set.
pub fn get_local_date_formarted(timestamp: i64) -> String {
    // Read the time zone name from the "TZ" environment variable
    // Use "Asia/Tokyo" as the default if not set
    let tz_name = get_tz();

    // Parse the time zone name into a `chrono_tz::Tz` type
    let tz: chrono_tz::Tz = tz_name.parse().unwrap_or(chrono_tz::Tz::Asia__Tokyo);

    // Get the current time in UTC
    let now_utc =  chrono::DateTime::from_timestamp(timestamp, 0).unwrap_or_default();

    // Convert UTC time to the specified time zone
    let now_in_tz = now_utc.with_timezone(&tz);

    // Extract the year/month/day in the target time zone (without time)
    format!("{}", now_in_tz.format("%Y-%m-%d"))
}

pub fn get_timestamp_from_local(datetime: &str, fmt: &str) -> i64 {
    // Read the time zone name from the "TZ" environment variable
    // Use "Asia/Tokyo" as the default if not set
    let tz_name = get_tz();

    // Parse the time zone name into a `chrono_tz::Tz` type
    let tz: chrono_tz::Tz = tz_name.parse().unwrap_or(chrono_tz::Tz::Asia__Tokyo);
   

    let local = match chrono::NaiveDateTime::parse_from_str(datetime, fmt){
        Ok(v) => v,
        Err(e) => {
            tracing::error!("{}:{} failed: {e:?}", file!(), line!());
            return 0;
        },
    };
    match tz.from_local_datetime(&local){
        chrono::offset::LocalResult::Single(v) => v.timestamp(),
        chrono::offset::LocalResult::Ambiguous(_, v2) => v2.timestamp(),
        chrono::offset::LocalResult::None => 0,
    }
}

pub fn get_timestamp_from_utc(datetime: &str, fmt: &str) -> i64 {
    let naive = match chrono::NaiveDateTime::parse_from_str(datetime, fmt){
        Ok(v) => v,
        Err(e) => {
            tracing::error!("{}:{} failed: {e:?}", file!(), line!());
            return 0;
        },
    };
    let utc = naive.and_utc();
    utc.timestamp()
}

pub fn setup_env() {
    dotenv::dotenv().ok();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}




        
