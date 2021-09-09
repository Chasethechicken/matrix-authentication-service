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

use async_trait::async_trait;
use chrono::Duration;
use schemars::{gen::SchemaGenerator, schema::Schema, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use super::ConfigurationSection;

fn default_ttl() -> Duration {
    Duration::hours(1)
}

fn ttl_schema(gen: &mut SchemaGenerator) -> Schema {
    u64::json_schema(gen)
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CsrfConfig {
    #[schemars(schema_with = "ttl_schema")]
    #[serde(default = "default_ttl")]
    #[serde_as(as = "serde_with::DurationSeconds<i64>")]
    pub ttl: Duration,
}

impl Default for CsrfConfig {
    fn default() -> Self {
        Self { ttl: default_ttl() }
    }
}

#[async_trait]
impl ConfigurationSection<'_> for CsrfConfig {
    fn path() -> &'static str {
        "csrf"
    }

    async fn generate() -> anyhow::Result<Self> {
        Ok(Self::default())
    }
}

#[cfg(test)]
mod tests {
    use figment::Jail;

    use super::*;

    #[test]
    fn load_config() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "config.yaml",
                r#"
                    csrf:
                      ttl: 1800
                "#,
            )?;

            let config = CsrfConfig::load_from_file("config.yaml")?;

            assert_eq!(config.ttl, Duration::minutes(30));

            Ok(())
        });
    }
}
