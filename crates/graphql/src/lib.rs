// Copyright 2022-2023 The Matrix.org Foundation C.I.C.
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

#![forbid(unsafe_code)]
#![deny(
    clippy::all,
    clippy::str_to_string,
    rustdoc::broken_intra_doc_links,
    clippy::future_not_send
)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::unused_async
)]

use anyhow::Context;
use async_graphql::EmptySubscription;
use mas_data_model::{BrowserSession, Session, User};
use ulid::Ulid;

mod model;
mod mutations;
mod query;
mod state;

pub use self::{
    model::{CreationEvent, Node},
    mutations::Mutation,
    query::Query,
    state::{BoxState, State},
};

pub type Schema = async_graphql::Schema<Query, Mutation, EmptySubscription>;
pub type SchemaBuilder = async_graphql::SchemaBuilder<Query, Mutation, EmptySubscription>;

#[must_use]
pub fn schema_builder() -> SchemaBuilder {
    async_graphql::Schema::build(Query::new(), Mutation::new(), EmptySubscription)
        .register_output_type::<Node>()
        .register_output_type::<CreationEvent>()
}

/// The identity of the requester.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Requester {
    /// The requester presented no authentication information.
    #[default]
    Anonymous,

    /// The requester is a browser session, stored in a cookie.
    BrowserSession(BrowserSession),

    /// The requester is a OAuth2 session, with an access token.
    OAuth2Session(Session, User),
}

impl Requester {
    fn browser_session(&self) -> Option<&BrowserSession> {
        match self {
            Self::BrowserSession(session) => Some(session),
            Self::OAuth2Session(_, _) | Self::Anonymous => None,
        }
    }

    fn user(&self) -> Option<&User> {
        match self {
            Self::BrowserSession(session) => Some(&session.user),
            Self::OAuth2Session(_session, user) => Some(user),
            Self::Anonymous => None,
        }
    }

    fn ensure_owner_or_admin(&self, user_id: Ulid) -> Result<(), async_graphql::Error> {
        // If the requester is an admin, they can do anything.
        if self.is_admin() {
            return Ok(());
        }

        // Else check that they are the owner.
        let user = self.user().context("Unauthorized")?;
        if user.id == user_id {
            Ok(())
        } else {
            Err(async_graphql::Error::new("Unauthorized"))
        }
    }

    fn is_admin(&self) -> bool {
        match self {
            Self::OAuth2Session(session, _user) => {
                // TODO: is this the right scope?
                // This has to be in sync with the policy
                session.scope.contains("urn:mas:admin")
            }
            Self::BrowserSession(_) | Self::Anonymous => false,
        }
    }
}

impl From<BrowserSession> for Requester {
    fn from(session: BrowserSession) -> Self {
        Self::BrowserSession(session)
    }
}

impl<T> From<Option<T>> for Requester
where
    T: Into<Requester>,
{
    fn from(session: Option<T>) -> Self {
        session.map(Into::into).unwrap_or_default()
    }
}
