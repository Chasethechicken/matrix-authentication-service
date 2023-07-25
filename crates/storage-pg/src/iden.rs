// Copyright 2023 The Matrix.org Foundation C.I.C.
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

//! Table and column identifiers used by [`sea_query`]

#[derive(sea_query::Iden)]
pub enum UserSessions {
    Table,
    UserSessionId,
    CreatedAt,
    FinishedAt,
    UserId,
}

#[derive(sea_query::Iden)]
pub enum Users {
    Table,
    UserId,
    Username,
    PrimaryUserEmailId,
}

#[derive(sea_query::Iden)]
pub enum UserEmails {
    Table,
    UserEmailId,
    UserId,
    Email,
    CreatedAt,
    ConfirmedAt,
}

#[derive(sea_query::Iden)]
pub enum CompatSessions {
    Table,
    CompatSessionId,
    UserId,
    DeviceId,
    CreatedAt,
    FinishedAt,
    IsSynapseAdmin,
}

#[derive(sea_query::Iden)]
pub enum CompatSsoLogins {
    Table,
    CompatSsoLoginId,
    RedirectUri,
    LoginToken,
    CompatSessionId,
    CreatedAt,
    FulfilledAt,
    ExchangedAt,
}

#[derive(sea_query::Iden)]
#[iden = "oauth2_sessions"]
pub enum OAuth2Sessions {
    Table,
    #[iden = "oauth2_session_id"]
    OAuth2SessionId,
    UserSessionId,
    #[iden = "oauth2_client_id"]
    OAuth2ClientId,
    Scope,
    CreatedAt,
    FinishedAt,
}

#[derive(sea_query::Iden)]
#[iden = "upstream_oauth_providers"]
pub enum UpstreamOAuthProviders {
    Table,
    #[iden = "upstream_oauth_provider_id"]
    UpstreamOAuthProviderId,
    Issuer,
    Scope,
    ClientId,
    EncryptedClientSecret,
    TokenEndpointSigningAlg,
    TokenEndpointAuthMethod,
    CreatedAt,
    ClaimsImports,
}

#[derive(sea_query::Iden)]
#[iden = "upstream_oauth_links"]
pub enum UpstreamOAuthLinks {
    Table,
    #[iden = "upstream_oauth_link_id"]
    UpstreamOAuthLinkId,
    #[iden = "upstream_oauth_provider_id"]
    UpstreamOAuthProviderId,
    UserId,
    Subject,
    CreatedAt,
}