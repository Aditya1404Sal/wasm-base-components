use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{de::DeserializeOwned, Deserialize};
use std::collections::HashMap;

pub mod bindings {
    wit_bindgen::generate!({ generate_all });
}

use crate::bindings::exports::betty_blocks::auth::jwt::{AuthError, Configuration, Guest};
use crate::bindings::wasi::http::types::IncomingRequest;

struct Component;

const CONFIG_KEY_AUTHENTICATION_PROFILES: &str = "authentication_profiles";
const CONFIG_KEY_ACTIONS: &str = "actions";
const CONFIG_KEY_MCPS: &str = "mcps";

#[derive(Debug, Deserialize)]
struct JwtPayload {
    auth_profile_id: String,
}

#[derive(Deserialize)]
struct AuthProfileConfig {
    value: String,
    is_encrypted: bool,
}

#[derive(Deserialize)]
struct ResourceAuthConfig {
    #[serde(rename = "authentication-profile-id")]
    authentication_profile_id: String,
}

fn load_config<T: DeserializeOwned>(key: &str) -> Result<T, AuthError> {
    let raw = crate::bindings::wasi::config::store::get(key)
        .map_err(|e| {
            AuthError::MissingConfig(format!("Config store error for '{}': {:?}", key, e))
        })?
        .ok_or_else(|| {
            AuthError::MissingConfig(format!("Key '{}' not found in config store", key))
        })?;
    serde_json::from_str(&raw)
        .map_err(|e| AuthError::MissingConfig(format!("Failed to parse {}: {}", key, e)))
}

fn load_auth_profiles() -> Result<HashMap<String, AuthProfileConfig>, AuthError> {
    load_config(CONFIG_KEY_AUTHENTICATION_PROFILES)
}

fn load_actions_config() -> Result<HashMap<String, ResourceAuthConfig>, AuthError> {
    load_config(CONFIG_KEY_ACTIONS)
}

fn load_mcps_config() -> Result<HashMap<String, ResourceAuthConfig>, AuthError> {
    load_config(CONFIG_KEY_MCPS)
}

fn extract_bearer_token(request: &IncomingRequest) -> Result<String, AuthError> {
    let headers = request.headers().entries();
    let auth_value = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
        .ok_or(AuthError::MalformedToken)?;
    let value = String::from_utf8_lossy(&auth_value.1);
    value
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != "null")
        .map(str::to_string)
        .ok_or(AuthError::MalformedToken)
}

fn peek_auth_profile_id(token: &str) -> Result<String, AuthError> {
    let payload_b64 = token.split('.').nth(1).ok_or(AuthError::MalformedToken)?;
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| AuthError::MalformedToken)?;
    let claims: JwtPayload =
        serde_json::from_slice(&payload_bytes).map_err(|_| AuthError::MalformedToken)?;
    Ok(claims.auth_profile_id)
}

fn validate_hs256(token: &str, secret: &str) -> Result<(), AuthError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.validate_nbf = true;
    validation.leeway = 30;
    validation.set_required_spec_claims(&["exp"]);
    let _claims = decode::<serde_json::Value>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| AuthError::ValidationFailed(format!("JWT validation failed: {}", e)))?;
    Ok(())
}

fn fetch_validated_profile(
    request: &IncomingRequest,
) -> Result<(String, AuthProfileConfig), AuthError> {
    let token = extract_bearer_token(request)?;
    let jwt_profile_id = peek_auth_profile_id(&token)?;
    let profiles = load_auth_profiles()?;
    let profile = profiles
        .into_iter()
        .find(|(id, _)| id == &jwt_profile_id)
        .ok_or_else(|| AuthError::ValidationFailed("Unknown auth profile in JWT".into()))?;
    validate_hs256(&token, &profile.1.value)?;
    Ok(profile)
}

fn authenticate_and_check_profile(
    request: &IncomingRequest,
    expected_profile_id: &str,
) -> Result<bool, AuthError> {
    let (jwt_profile_id, _) = fetch_validated_profile(request)?;
    Ok(jwt_profile_id == expected_profile_id)
}

impl Guest for Component {
    fn allowed_to_call(
        request: &IncomingRequest,
        action_id: String,
    ) -> Result<Configuration, AuthError> {
        let actions = load_actions_config()?;
        let action_cfg = actions
            .get(&action_id)
            .ok_or_else(|| AuthError::ValidationFailed("Action not found in auth config".into()))?;
        let (jwt_profile_id, profile) = fetch_validated_profile(request)?;
        if jwt_profile_id != action_cfg.authentication_profile_id {
            return Err(AuthError::ValidationFailed(
                "Forbidden: auth profile does not allow this action".into(),
            ));
        }
        Ok(Configuration {
            value: profile.value,
            is_encrypted: profile.is_encrypted,
        })
    }

    fn allowed_to_list(request: &IncomingRequest, mcp_id: String) -> Result<bool, AuthError> {
        let mcps = load_mcps_config()?;
        let mcp_cfg = mcps
            .get(&mcp_id)
            .ok_or_else(|| AuthError::ValidationFailed("MCP not found in auth config".into()))?;
        authenticate_and_check_profile(request, &mcp_cfg.authentication_profile_id)
    }
}

bindings::export!(Component with_types_in bindings);

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    use serde::Serialize;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn make_hs256_token(secret: &[u8], auth_profile: &str, exp_offset: i64) -> String {
        #[derive(Serialize)]
        struct Claims {
            auth_profile_id: String,
            exp: u64,
            nbf: u64,
            iat: u64,
        }
        let n = now();
        let claims = Claims {
            auth_profile_id: auth_profile.to_string(),
            exp: (n as i64 + exp_offset) as u64,
            nbf: n,
            iat: n,
        };
        let header = Header::new(Algorithm::HS256);
        encode(&header, &claims, &EncodingKey::from_secret(secret)).expect("encode failed")
    }

    #[test]
    fn test_peek_auth_profile_id_valid() {
        let token = make_hs256_token(b"secret", "profile-abc", 3600);
        let result = peek_auth_profile_id(&token);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "profile-abc");
    }

    #[test]
    fn test_peek_auth_profile_id_malformed_no_dots() {
        let result = peek_auth_profile_id("nodots");
        assert!(matches!(result, Err(AuthError::MalformedToken)));
    }

    #[test]
    fn test_peek_auth_profile_id_invalid_base64() {
        let result = peek_auth_profile_id("header.!!!invalid_base64!!!.sig");
        assert!(matches!(result, Err(AuthError::MalformedToken)));
    }

    #[test]
    fn test_validate_hs256_valid() {
        let secret = b"test_secret";
        let token = make_hs256_token(secret, "profile-xyz", 3600);
        let result = validate_hs256(&token, "test_secret");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_hs256_wrong_secret() {
        let token = make_hs256_token(b"correct_secret", "profile-xyz", 3600);
        let result = validate_hs256(&token, "wrong_secret");
        assert!(matches!(result, Err(AuthError::ValidationFailed(_))));
    }

    #[test]
    fn test_validate_hs256_expired() {
        let secret = b"test_secret";
        let token = make_hs256_token(secret, "profile-xyz", -3600);
        let result = validate_hs256(&token, "test_secret");
        assert!(matches!(result, Err(AuthError::ValidationFailed(_))));
    }
}
