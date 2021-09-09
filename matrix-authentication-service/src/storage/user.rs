// Copyright 2021 The Matrix.org Foundation C.I.C.
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

use std::borrow::BorrowMut;

use anyhow::Context;
use argon2::Argon2;
use chrono::{DateTime, Utc};
use password_hash::{PasswordHash, PasswordHasher, SaltString};
use rand::rngs::OsRng;
use serde::Serialize;
use sqlx::{Acquire, Executor, FromRow, Postgres, Transaction};
use thiserror::Error;
use tokio::task;
use tracing::{info_span, Instrument};

use crate::errors::HtmlError;

#[derive(Serialize, Debug, Clone, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
}

#[derive(Serialize, Debug, Clone, FromRow)]
pub struct SessionInfo {
    id: i64,
    user_id: i64,
    username: String,
    pub active: bool,
    created_at: DateTime<Utc>,
    pub last_authd_at: Option<DateTime<Utc>>,
}

impl SessionInfo {
    pub fn key(&self) -> i64 {
        self.id
    }

    pub async fn reauth(
        mut self,
        conn: impl Acquire<'_, Database = Postgres>,
        password: String,
    ) -> anyhow::Result<Self> {
        let mut txn = conn.begin().await?;
        self.last_authd_at = Some(authenticate_session(&mut txn, self.id, password).await?);
        txn.commit().await?;
        Ok(self)
    }

    pub async fn end(
        mut self,
        executor: impl Executor<'_, Database = Postgres>,
    ) -> anyhow::Result<Self> {
        end_session(executor, self.id).await?;
        self.active = false;
        Ok(self)
    }
}

#[derive(Debug, Error)]
pub enum LoginError {
    #[error("could not find user {username:?}")]
    NotFound {
        username: String,
        #[source]
        source: sqlx::Error,
    },

    #[error("authentication failed for {username:?}")]
    Authentication {
        username: String,
        #[source]
        source: AuthenticationError,
    },

    #[error("failed to login")]
    Other(#[from] anyhow::Error),
}

impl HtmlError for LoginError {
    fn html_display(&self) -> String {
        match self {
            LoginError::NotFound { .. } => "Could not find user".to_string(),
            LoginError::Authentication { .. } => "Failed to authenticate user".to_string(),
            LoginError::Other(e) => format!("Internal error: <pre>{}</pre>", e),
        }
    }
}

pub async fn login(
    conn: impl Acquire<'_, Database = Postgres>,
    username: &str,
    password: String,
) -> Result<SessionInfo, LoginError> {
    let mut txn = conn.begin().await.context("could not start transaction")?;
    let user = lookup_user_by_username(&mut txn, username)
        .await
        .map_err(|source| {
            if matches!(source, sqlx::Error::RowNotFound) {
                LoginError::NotFound {
                    username: username.to_string(),
                    source,
                }
            } else {
                LoginError::Other(source.into())
            }
        })?;

    let mut session = start_session(&mut txn, user).await?;
    session.last_authd_at = Some(
        authenticate_session(&mut txn, session.id, password)
            .await
            .map_err(|source| {
                if matches!(source, AuthenticationError::Password { .. }) {
                    LoginError::Authentication {
                        username: username.to_string(),
                        source,
                    }
                } else {
                    LoginError::Other(source.into())
                }
            })?,
    );
    txn.commit().await.context("could not commit transaction")?;
    Ok(session)
}

pub async fn lookup_active_session(
    executor: impl Executor<'_, Database = Postgres>,
    id: i64,
) -> anyhow::Result<SessionInfo> {
    sqlx::query_as!(
        SessionInfo,
        r#"
            SELECT
                s.id,
                u.id as user_id,
                u.username,
                s.active,
                s.created_at,
                a.created_at as "last_authd_at?"
            FROM user_sessions s
            INNER JOIN users u 
                ON s.user_id = u.id
            LEFT JOIN user_session_authentications a
                ON a.session_id = s.id
            WHERE s.id = $1 AND s.active
            ORDER BY a.created_at DESC
            LIMIT 1
        "#,
        id,
    )
    .fetch_one(executor)
    .await
    .context("could not fetch session")
}

pub async fn lookup_session(
    executor: impl Executor<'_, Database = Postgres>,
    id: i64,
) -> anyhow::Result<SessionInfo> {
    sqlx::query_as!(
        SessionInfo,
        r#"
            SELECT
                s.id,
                u.id as user_id,
                u.username,
                s.active,
                s.created_at,
                a.created_at as "last_authd_at?"
            FROM user_sessions s
            INNER JOIN users u 
                ON s.user_id = u.id
            LEFT JOIN user_session_authentications a
                ON a.session_id = s.id
            WHERE s.id = $1
            ORDER BY a.created_at DESC
            LIMIT 1
        "#,
        id,
    )
    .fetch_one(executor)
    .await
    .context("could not fetch session")
}

pub async fn start_session(
    executor: impl Executor<'_, Database = Postgres>,
    user: User,
) -> anyhow::Result<SessionInfo> {
    let (id, created_at): (i64, DateTime<Utc>) = sqlx::query_as(
        r#"
            INSERT INTO user_sessions (user_id)
            VALUES ($1)
            RETURNING id, created_at
        "#,
    )
    .bind(user.id)
    .fetch_one(executor)
    .await
    .context("could not create session")?;

    Ok(SessionInfo {
        id,
        user_id: user.id,
        username: user.username,
        active: true,
        created_at,
        last_authd_at: None,
    })
}

#[derive(Debug, Error)]
pub enum AuthenticationError {
    #[error("could not verify password")]
    Password(#[from] password_hash::Error),

    #[error("could not fetch user password hash")]
    Fetch(sqlx::Error),

    #[error("could not save session auth")]
    Save(sqlx::Error),

    #[error("runtime error")]
    Internal(#[from] tokio::task::JoinError),
}

pub async fn authenticate_session(
    txn: &mut Transaction<'_, Postgres>,
    session_id: i64,
    password: String,
) -> Result<DateTime<Utc>, AuthenticationError> {
    // First, fetch the hashed password from the user associated with that session
    let hashed_password: String = sqlx::query_scalar!(
        r#"
            SELECT u.hashed_password
            FROM user_sessions s
            INNER JOIN users u
               ON u.id = s.user_id 
            WHERE s.id = $1
        "#,
        session_id,
    )
    .fetch_one(txn.borrow_mut())
    .await
    .map_err(AuthenticationError::Fetch)?;

    // TODO: pass verifiers list as parameter
    // Verify the password in a blocking thread to avoid blocking the async executor
    task::spawn_blocking(move || {
        let context = Argon2::default();
        let hasher = PasswordHash::new(&hashed_password).map_err(AuthenticationError::Password)?;
        hasher
            .verify_password(&[&context], &password)
            .map_err(AuthenticationError::Password)
    })
    .await??;

    // That went well, let's insert the auth info
    let created_at: DateTime<Utc> = sqlx::query_scalar!(
        r#"
            INSERT INTO user_session_authentications (session_id)
            VALUES ($1)
            RETURNING created_at
        "#,
        session_id,
    )
    .fetch_one(txn.borrow_mut())
    .await
    .map_err(AuthenticationError::Save)?;

    Ok(created_at)
}

pub async fn register_user(
    executor: impl Executor<'_, Database = Postgres>,
    phf: impl PasswordHasher,
    username: &str,
    password: &str,
) -> anyhow::Result<User> {
    let salt = SaltString::generate(&mut OsRng);
    let hashed_password = PasswordHash::generate(phf, password, salt.as_str())?;

    let id: i64 = sqlx::query_scalar!(
        r#"
            INSERT INTO users (username, hashed_password)
            VALUES ($1, $2)
            RETURNING id
        "#,
        username,
        hashed_password.to_string(),
    )
    .fetch_one(executor)
    .instrument(info_span!("Register user"))
    .await
    .context("could not insert user")?;

    Ok(User {
        id,
        username: username.to_string(),
    })
}

pub async fn end_session(
    executor: impl Executor<'_, Database = Postgres>,
    id: i64,
) -> anyhow::Result<()> {
    let res = sqlx::query!("UPDATE user_sessions SET active = FALSE WHERE id = $1", id)
        .execute(executor)
        .instrument(info_span!("End session"))
        .await
        .context("could not end session")?;

    match res.rows_affected() {
        1 => Ok(()),
        0 => Err(anyhow::anyhow!("no row affected")),
        _ => Err(anyhow::anyhow!("too many row affected")),
    }
}

#[allow(dead_code)]
pub async fn lookup_user_by_id(
    executor: impl Executor<'_, Database = Postgres>,
    id: i64,
) -> anyhow::Result<User> {
    sqlx::query_as!(
        User,
        r#"
            SELECT id, username
            FROM users
            WHERE id = $1
        "#,
        id
    )
    .fetch_one(executor)
    .instrument(info_span!("Fetch user"))
    .await
    .context("could not fetch user")
}

pub async fn lookup_user_by_username(
    executor: impl Executor<'_, Database = Postgres>,
    username: &str,
) -> Result<User, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"
            SELECT id, username
            FROM users
            WHERE username = $1
        "#,
        username,
    )
    .fetch_one(executor)
    .instrument(info_span!("Fetch user"))
    .await
}
