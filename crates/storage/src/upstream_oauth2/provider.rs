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

use chrono::{DateTime, Utc};
use mas_data_model::UpstreamOAuthProvider;
use mas_iana::{jose::JsonWebSignatureAlg, oauth::OAuthClientAuthenticationMethod};
use oauth2_types::scope::Scope;
use rand::Rng;
use sqlx::PgExecutor;
use thiserror::Error;
use ulid::Ulid;
use uuid::Uuid;

use crate::{Clock, DatabaseInconsistencyError, LookupError};

#[derive(Debug, Error)]
#[error("Failed to lookup upstream OAuth 2.0 provider")]
pub enum ProviderLookupError {
    Driver(#[from] sqlx::Error),
    Inconcistency(#[from] DatabaseInconsistencyError),
}

impl LookupError for ProviderLookupError {
    fn not_found(&self) -> bool {
        matches!(self, Self::Driver(sqlx::Error::RowNotFound))
    }
}

struct ProviderLookup {
    upstream_oauth_provider_id: Uuid,
    issuer: String,
    scope: String,
    client_id: String,
    encrypted_client_secret: Option<String>,
    token_endpoint_signing_alg: Option<String>,
    token_endpoint_auth_method: String,
    created_at: DateTime<Utc>,
}

#[tracing::instrument(
    skip_all,
    fields(upstream_oauth_provider.id = %id),
    err,
)]
pub async fn lookup_provider(
    executor: impl PgExecutor<'_>,
    id: Ulid,
) -> Result<UpstreamOAuthProvider, ProviderLookupError> {
    let res = sqlx::query_as!(
        ProviderLookup,
        r#"
            SELECT
                upstream_oauth_provider_id,
                issuer,
                scope,
                client_id,
                encrypted_client_secret,
                token_endpoint_signing_alg,
                token_endpoint_auth_method,
                created_at
            FROM upstream_oauth_providers
            WHERE upstream_oauth_provider_id = $1
        "#,
        Uuid::from(id),
    )
    .fetch_one(executor)
    .await?;

    Ok(UpstreamOAuthProvider {
        id: res.upstream_oauth_provider_id.into(),
        issuer: res.issuer,
        scope: res.scope.parse().map_err(|_| DatabaseInconsistencyError)?,
        client_id: res.client_id,
        encrypted_client_secret: res.encrypted_client_secret,
        token_endpoint_auth_method: res
            .token_endpoint_auth_method
            .parse()
            .map_err(|_| DatabaseInconsistencyError)?,
        token_endpoint_signing_alg: res
            .token_endpoint_signing_alg
            .map(|x| x.parse())
            .transpose()
            .map_err(|_| DatabaseInconsistencyError)?,
        created_at: res.created_at,
    })
}

#[tracing::instrument(
    skip_all,
    fields(
        upstream_oauth_provider.id,
        upstream_oauth_provider.issuer = %issuer,
        upstream_oauth_provider.client_id = %client_id,
    ),
    err,
)]
#[allow(clippy::too_many_arguments)]
pub async fn add_provider(
    executor: impl PgExecutor<'_>,
    mut rng: impl Rng + Send,
    clock: &Clock,
    issuer: String,
    scope: Scope,
    token_endpoint_auth_method: OAuthClientAuthenticationMethod,
    token_endpoint_signing_alg: Option<JsonWebSignatureAlg>,
    client_id: String,
    encrypted_client_secret: Option<String>,
) -> Result<UpstreamOAuthProvider, sqlx::Error> {
    let created_at = clock.now();
    let id = Ulid::from_datetime_with_source(created_at.into(), &mut rng);
    tracing::Span::current().record("upstream_oauth_provider.id", tracing::field::display(id));

    sqlx::query!(
        r#"
            INSERT INTO upstream_oauth_providers (
                upstream_oauth_provider_id,
                issuer,
                scope,
                token_endpoint_auth_method,
                token_endpoint_signing_alg,
                client_id,
                encrypted_client_secret,
                created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
        Uuid::from(id),
        &issuer,
        scope.to_string(),
        token_endpoint_auth_method.to_string(),
        token_endpoint_signing_alg.as_ref().map(ToString::to_string),
        &client_id,
        encrypted_client_secret.as_deref(),
        created_at,
    )
    .execute(executor)
    .await?;

    Ok(UpstreamOAuthProvider {
        id,
        issuer,
        scope,
        client_id,
        encrypted_client_secret,
        token_endpoint_signing_alg,
        token_endpoint_auth_method,
        created_at,
    })
}