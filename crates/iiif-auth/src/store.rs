use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Instant;

use rand::Rng;

/// In-memory store for sessions and access tokens.
///
/// Sessions are created when a user logs in via the access service.
/// Tokens are issued by the token service and reference a session.
pub struct AuthStore {
    sessions: RwLock<HashMap<String, SessionInfo>>,
    tokens: RwLock<HashMap<String, TokenInfo>>,
    token_ttl_secs: u64,
}

#[allow(dead_code)]
struct SessionInfo {
    username: String,
    created: Instant,
}

struct TokenInfo {
    session_id: String,
    created: Instant,
}

impl AuthStore {
    pub fn new(token_ttl_secs: u64) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            tokens: RwLock::new(HashMap::new()),
            token_ttl_secs,
        }
    }

    /// Create a new session for an authenticated user. Returns the session ID.
    pub fn create_session(&self, username: &str) -> String {
        let session_id = generate_random_id();
        let info = SessionInfo {
            username: username.to_string(),
            created: Instant::now(),
        };
        self.sessions
            .write()
            .expect("session lock")
            .insert(session_id.clone(), info);
        session_id
    }

    /// Check if a session ID is valid and return the username.
    pub fn validate_session(&self, session_id: &str) -> Option<String> {
        let sessions = self.sessions.read().expect("session lock");
        sessions.get(session_id).map(|s| s.username.clone())
    }

    /// Remove a session (logout).
    pub fn remove_session(&self, session_id: &str) {
        self.sessions
            .write()
            .expect("session lock")
            .remove(session_id);
    }

    /// Issue an access token for a valid session. Returns `(token, expires_in)`.
    pub fn issue_token(&self, session_id: &str) -> Option<(String, u64)> {
        // Verify session exists
        self.validate_session(session_id)?;

        let token = generate_random_id();
        let info = TokenInfo {
            session_id: session_id.to_string(),
            created: Instant::now(),
        };
        self.tokens
            .write()
            .expect("token lock")
            .insert(token.clone(), info);

        Some((token, self.token_ttl_secs))
    }

    /// Validate an access token. Returns `true` if the token is valid and not expired.
    pub fn validate_token(&self, token: &str) -> bool {
        let tokens = self.tokens.read().expect("token lock");
        match tokens.get(token) {
            Some(info) => {
                let elapsed = info.created.elapsed().as_secs();
                if elapsed > self.token_ttl_secs {
                    return false;
                }
                // Also check that the underlying session still exists
                let sessions = self.sessions.read().expect("session lock");
                sessions.contains_key(&info.session_id)
            }
            None => false,
        }
    }

    /// Purge expired tokens and orphaned sessions.
    pub fn cleanup(&self) {
        let mut tokens = self.tokens.write().expect("token lock");
        tokens.retain(|_, info| info.created.elapsed().as_secs() <= self.token_ttl_secs);
    }
}

fn generate_random_id() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 16] = rng.random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_lifecycle() {
        let store = AuthStore::new(3600);
        let sid = store.create_session("alice");

        assert_eq!(store.validate_session(&sid), Some("alice".to_string()));
        assert_eq!(store.validate_session("invalid"), None);

        store.remove_session(&sid);
        assert_eq!(store.validate_session(&sid), None);
    }

    #[test]
    fn token_lifecycle() {
        let store = AuthStore::new(3600);
        let sid = store.create_session("bob");

        let (token, ttl) = store.issue_token(&sid).unwrap();
        assert_eq!(ttl, 3600);
        assert!(store.validate_token(&token));
        assert!(!store.validate_token("bogus"));

        // Remove session invalidates token
        store.remove_session(&sid);
        assert!(!store.validate_token(&token));
    }

    #[test]
    fn no_token_without_session() {
        let store = AuthStore::new(3600);
        assert!(store.issue_token("nonexistent").is_none());
    }
}
