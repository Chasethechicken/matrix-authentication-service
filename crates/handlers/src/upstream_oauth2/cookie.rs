// Copyright 2022 The Matrix.org Foundation C.I.C.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// TODO: move that to a standalone cookie manager

use axum_extra::extract::{cookie::Cookie, PrivateCookieJar};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use mas_axum_utils::CookieExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;
use ulid::Ulid;

/// Name of the cookie
static COOKIE_NAME: &str = "upstream-oauth2-sessions";

/// Sessions expire after 10 minutes
static SESSION_MAX_TIME_SECS: i64 = 60 * 10;

#[derive(Serialize, Deserialize)]
pub struct Payload {
    session: Ulid,
    provider: Ulid,
    state: String,
    link: Option<Ulid>,
}

impl Payload {
    fn expired(&self, now: DateTime<Utc>) -> bool {
        let Ok(ts) = self.session.timestamp_ms().try_into() else { return true };
        let Some(when) = NaiveDateTime::from_timestamp_millis(ts) else { return true };
        let when = DateTime::from_utc(when, Utc);
        let max_age = Duration::seconds(SESSION_MAX_TIME_SECS);
        now - when > max_age
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct UpstreamSessions(Vec<Payload>);

#[derive(Debug, Error, PartialEq, Eq)]
#[error("upstream session not found")]
pub struct UpstreamSessionNotFound;

impl UpstreamSessions {
    /// Load the upstreams sessions cookie
    pub fn load<K>(cookie_jar: &PrivateCookieJar<K>) -> Self {
        cookie_jar
            .get(COOKIE_NAME)
            .and_then(|c| c.decode().ok())
            .unwrap_or_default()
    }

    /// Save the upstreams sessions to the cookie jar
    pub fn save<K>(
        self,
        cookie_jar: PrivateCookieJar<K>,
        now: DateTime<Utc>,
    ) -> PrivateCookieJar<K> {
        let this = self.expire(now);
        let mut cookie = Cookie::named(COOKIE_NAME).encode(&this);
        cookie.set_path("/");
        cookie.set_http_only(true);

        let expiration = now + Duration::seconds(SESSION_MAX_TIME_SECS);
        let expiration = OffsetDateTime::from_unix_timestamp(expiration.timestamp())
            .expect("invalid unix timestamp");
        cookie.set_expires(expiration);

        cookie_jar.add(cookie)
    }

    fn expire(mut self, now: DateTime<Utc>) -> Self {
        self.0.retain(|p| !p.expired(now));
        self
    }

    /// Add a new session, for a provider and a random state
    pub fn add(mut self, session: Ulid, provider: Ulid, state: String) -> Self {
        self.0.push(Payload {
            session,
            provider,
            state,
            link: None,
        });
        self
    }

    // Find a session ID from the provider and the state
    pub fn find_session(
        &self,
        provider: Ulid,
        state: &str,
    ) -> Result<Ulid, UpstreamSessionNotFound> {
        self.0
            .iter()
            .find(|p| p.provider == provider && p.state == state && p.link.is_none())
            .map(|p| p.session)
            .ok_or(UpstreamSessionNotFound)
    }

    /// Save the link generated by a session
    pub fn add_link_to_session(
        mut self,
        session: Ulid,
        link: Ulid,
    ) -> Result<Self, UpstreamSessionNotFound> {
        let payload = self
            .0
            .iter_mut()
            .find(|p| p.session == session && p.link.is_none())
            .ok_or(UpstreamSessionNotFound)?;

        payload.link = Some(link);
        Ok(self)
    }

    /// Find a session from its link
    pub fn lookup_link(&self, link_id: Ulid) -> Result<Ulid, UpstreamSessionNotFound> {
        self.0
            .iter()
            .find(|p| p.link == Some(link_id))
            .map(|p| p.session)
            .ok_or(UpstreamSessionNotFound)
    }

    /// Mark a link as consumed to avoid replay
    pub fn consume_link(mut self, link_id: Ulid) -> Result<Self, UpstreamSessionNotFound> {
        let pos = self
            .0
            .iter()
            .position(|p| p.link == Some(link_id))
            .ok_or(UpstreamSessionNotFound)?;

        self.0.remove(pos);

        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use rand::SeedableRng;
    use rand_chacha::ChaChaRng;

    use super::*;

    #[test]
    fn test_session_cookie() {
        let now = chrono::Utc
            .with_ymd_and_hms(2018, 1, 18, 1, 30, 22)
            .unwrap();
        let mut rng = ChaChaRng::seed_from_u64(42);

        let sessions = UpstreamSessions::default();

        let provider_a = Ulid::from_datetime_with_source(now.into(), &mut rng);
        let provider_b = Ulid::from_datetime_with_source(now.into(), &mut rng);

        let first_session = Ulid::from_datetime_with_source(now.into(), &mut rng);
        let first_state = "first-state";
        let sessions = sessions.add(first_session, provider_a, first_state.into());

        let now = now + Duration::minutes(5);

        let second_session = Ulid::from_datetime_with_source(now.into(), &mut rng);
        let second_state = "second-state";
        let sessions = sessions.add(second_session, provider_b, second_state.into());

        let sessions = sessions.expire(now);
        assert_eq!(
            sessions.find_session(provider_a, first_state),
            Ok(first_session)
        );
        assert_eq!(
            sessions.find_session(provider_b, second_state),
            Ok(second_session)
        );
        assert!(sessions.find_session(provider_b, first_state).is_err());
        assert!(sessions.find_session(provider_a, second_state).is_err());

        // Make the first session expire
        let now = now + Duration::minutes(6);
        let sessions = sessions.expire(now);
        assert!(sessions.find_session(provider_a, first_state).is_err());
        assert_eq!(
            sessions.find_session(provider_b, second_state),
            Ok(second_session)
        );

        // Associate a link with the second
        let second_link = Ulid::from_datetime_with_source(now.into(), &mut rng);
        let sessions = sessions
            .add_link_to_session(second_session, second_link)
            .unwrap();

        // Now the session can't be found with its state
        assert!(sessions.find_session(provider_b, second_state).is_err());

        // But it can be looked up by its link
        assert_eq!(sessions.lookup_link(second_link), Ok(second_session));
        // And it can be consumed
        let sessions = sessions.consume_link(second_link).unwrap();
        // But only once
        assert!(sessions.consume_link(second_link).is_err());
    }
}