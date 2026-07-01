//! Staff Access Token parsing and Admin API JWT minting.
//!
//! Ghost's Admin API authenticates with a short-lived JWT (HS256) whose signing
//! key is the **hex-decoded** secret half of the `{id}:{secret}` token. The
//! token's `id` goes into the JWT header `kid`, and the payload fixes
//! `aud = "/admin/"` with a 5-minute expiry.

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::Serialize;

use crate::constants::{ADMIN_AUDIENCE, JWT_TTL_SECS};
use crate::error::{CoreError, Result};

/// A parsed Staff Access Token, ready to mint Admin API JWTs.
#[derive(Clone)]
pub(crate) struct StaffToken {
    id: String,
    secret: Vec<u8>,
}

impl std::fmt::Debug for StaffToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the secret.
        f.debug_struct("StaffToken")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Admin API JWT claims.
#[derive(Debug, Serialize)]
struct AdminClaims {
    iat: u64,
    exp: u64,
    aud: String,
}

impl StaffToken {
    /// Parse a `{id}:{secret}` token, validating the hex secret.
    ///
    /// # Errors
    /// Returns [`CoreError::InvalidToken`] when the `:` separator is missing,
    /// either half is empty, or the secret is not valid hex.
    pub(crate) fn parse(raw: &str) -> Result<Self> {
        let (id, secret_hex) = raw
            .trim()
            .split_once(':')
            .ok_or_else(|| CoreError::InvalidToken("expected `{id}:{secret}` form".to_owned()))?;
        if id.is_empty() || secret_hex.is_empty() {
            return Err(CoreError::InvalidToken(
                "id and secret must be non-empty".to_owned(),
            ));
        }
        let secret = hex::decode(secret_hex)
            .map_err(|e| CoreError::InvalidToken(format!("secret is not valid hex: {e}")))?;
        Ok(Self {
            id: id.to_owned(),
            secret,
        })
    }

    /// Mint a signed Admin API JWT valid for [`JWT_TTL_SECS`] seconds from
    /// `now_unix` (seconds since the Unix epoch).
    ///
    /// `now_unix` is taken as a parameter so callers — and tests — control time.
    ///
    /// # Errors
    /// Returns [`CoreError::Jwt`] if signing fails.
    pub(crate) fn sign_jwt(&self, now_unix: u64) -> Result<String> {
        let claims = AdminClaims {
            iat: now_unix,
            exp: now_unix.saturating_add(JWT_TTL_SECS),
            aud: ADMIN_AUDIENCE.to_owned(),
        };
        let mut header = Header::new(Algorithm::HS256);
        header.kid = Some(self.id.clone());
        let key = EncodingKey::from_secret(&self.secret);
        Ok(jsonwebtoken::encode(&header, &claims, &key)?)
    }
}

/// Current wall-clock time in seconds since the Unix epoch.
///
/// Returns `0` if the system clock is set before 1970 (it never is in practice).
#[must_use]
pub(crate) fn current_unix_time() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use jsonwebtoken::{DecodingKey, Validation};

    use super::{Algorithm, StaffToken};
    use crate::constants::ADMIN_AUDIENCE;

    // `e3b0c44298fc...` is a well-formed hex string (32 bytes).
    const SECRET_HEX: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn parse_rejects_missing_colon() {
        assert!(StaffToken::parse("no-colon-here").is_err());
    }

    #[test]
    fn parse_rejects_empty_halves() {
        assert!(StaffToken::parse(":abc").is_err());
        assert!(StaffToken::parse("abc:").is_err());
    }

    #[test]
    fn parse_rejects_non_hex_secret() {
        assert!(StaffToken::parse("id:zzzz").is_err());
    }

    #[test]
    fn sign_produces_decodable_admin_jwt() {
        let token = StaffToken::parse(&format!("64abc:{SECRET_HEX}")).unwrap();
        let now = 1_700_000_000u64;
        let jwt = token.sign_jwt(now).unwrap();

        // Header carries the kid.
        let header = jsonwebtoken::decode_header(&jwt).unwrap();
        assert_eq!(header.alg, Algorithm::HS256);
        assert_eq!(header.kid.as_deref(), Some("64abc"));

        // Payload validates against the audience and the 5-minute expiry.
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&[ADMIN_AUDIENCE]);
        validation.set_required_spec_claims(&["exp", "aud"]);
        // The test signs at a fixed past instant, so don't reject on expiry —
        // the claim values themselves are asserted below.
        validation.validate_exp = false;
        let key = DecodingKey::from_secret(&hex::decode(SECRET_HEX).unwrap());
        let data = jsonwebtoken::decode::<serde_json::Value>(&jwt, &key, &validation).unwrap();
        assert_eq!(data.claims["iat"].as_u64(), Some(now));
        assert_eq!(data.claims["exp"].as_u64(), Some(now + 300));
        assert_eq!(data.claims["aud"].as_str(), Some(ADMIN_AUDIENCE));
    }
}
