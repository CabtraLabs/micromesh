use jsonwebtoken::{DecodingKey, Validation};
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
struct Claims {
    #[serde(skip_serializing_if = "Option::is_none")]
    aud: Option<String>,         // Optional. Audience
    exp: usize,                  // Required (validate_exp defaults to true in validation). Expiration time (as UTC timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    iat: Option<usize>,          // Optional. Issued at (as UTC timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    iss: Option<String>,         // Optional. Issuer
    #[serde(skip_serializing_if = "Option::is_none")]
    nbf: Option<usize>,          // Optional. Not Before (as UTC timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    sub: Option<String>,         // Optional. Subject (whom token refers to)
    #[serde(skip_serializing_if = "Option::is_none")]
    typ: Option<String>,         // Optional. Type of token.
    #[serde(skip_serializing_if = "Option::is_none")]
    jti: Option<i64>,            // Optional. JWT ID. Unique identifier for the token
}

pub fn create_token(uid: &str, key: &[u8]) -> String {
    let now = chrono::Utc::now();
    let iat = now.timestamp() as usize;
    let jti = crate::snowflake::generate_id();
    let duration: i64 = crate::vars::get_jwt_duration();
    let exp = (now + chrono::Duration::try_seconds(duration).unwrap_or_default()).timestamp() as usize;
    let claims = Claims {
        sub: Some(uid.to_string()),
        exp,
        iat: Some(iat),
        typ: None,
        aud: None,
        iss: None,
        jti: Some(jti),
        nbf: None,
    };

    match jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(key),
    ){
        Ok(v) => v,
        Err(e) => {
            tracing::error!("create jwt failed {e:?}");
            "".to_string()
        },
    }
}

pub fn verify_token(token: &str, key: &[u8]) -> Option<String> {
    let mut validation = Validation::default();
    validation.validate_aud = false;
    validation.leeway = 0;
    match jsonwebtoken::decode::<Claims>(
        token, 
        &DecodingKey::from_secret(key), 
        &validation
    ){
        Ok(v) => {
            v.claims.sub
        },
        Err(_) => {
            None
        },
    }
}

