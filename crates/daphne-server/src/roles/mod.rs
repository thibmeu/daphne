// Copyright (c) 2024 Cloudflare, Inc. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use daphne::ReplayProtection;

use crate::storage_proxy_connection::kv::{self, Kv, KvGetOptions};

mod aggregator;
mod helper;
mod leader;

pub async fn fetch_replay_protection_override(kv: Kv<'_>) -> ReplayProtection {
    let skip_replay_protection = kv
        .get_cloned::<kv::prefix::GlobalConfigOverride<bool>>(
            &kv::prefix::GlobalOverrides::SkipReplayProtection,
            &KvGetOptions {
                cache_not_found: true,
            },
        )
        .await
        .inspect_err(
            |e| tracing::error!(error = ?e, "failed to fetch skip_replay_protection from kv"),
        )
        .ok() // treat error as false
        .flatten()
        .unwrap_or_default(); // treat missing as false
    if skip_replay_protection {
        tracing::debug!("replay protection is disabled");
        ReplayProtection::InsecureDisabled
    } else {
        ReplayProtection::Enabled
    }
}

#[cfg(feature = "test-utils")]
mod test_utils {
    use daphne::{
        auth::BearerToken,
        fatal_error,
        hpke::{HpkeConfig, HpkeReceiverConfig},
        messages::decode_base64url_vec,
        roles::DapAggregator,
        vdaf::{Prio3Config, VdafConfig},
        DapError, DapQueryConfig, DapTaskConfig, DapVersion,
    };
    use daphne_service_utils::{
        test_route_types::{InternalTestAddTask, InternalTestEndpointForTask},
        DapRole,
    };
    use prio::codec::Decode;
    use std::num::NonZeroUsize;

    use crate::storage_proxy_connection::kv;

    impl crate::App {
        pub(crate) async fn internal_delete_all(&self) -> Result<(), DapError> {
            self.test_leader_state.lock().await.delete_all();

            use daphne_service_utils::durable_requests::PURGE_STORAGE;
            *self.cache.write().await = Default::default();

            self.http
                .delete(self.storage_proxy_config.url.join(PURGE_STORAGE).unwrap())
                .bearer_auth(&self.storage_proxy_config.auth_token)
                .send()
                .await
                .map_err(
                    |e| fatal_error!(err = ?e, "failed to send delete request to storage proxy"),
                )?
                .error_for_status()
                .map_err(|e| fatal_error!(err = ?e, "failed to clear storage proxy"))?;

            Ok(())
        }

        pub(crate) async fn storage_ready_check(&self) -> Result<(), DapError> {
            use daphne_service_utils::durable_requests::STORAGE_READY;
            self.http
                .get(self.storage_proxy_config.url.join(STORAGE_READY).unwrap())
                .bearer_auth(&self.storage_proxy_config.auth_token)
                .send()
                .await
                .map_err(|e| fatal_error!(err = ?e, "failed to send ready check request to storage proxy"))?
                .error_for_status()
                .map_err(|e| fatal_error!(err = ?e, "storage proxy is not ready"))?;
            Ok(())
        }

        pub(crate) fn internal_endpoint_for_task(
            &self,
            version: DapVersion,
            cmd: InternalTestEndpointForTask,
        ) -> Result<String, DapError> {
            if self.service_config.role != cmd.role {
                return Err(fatal_error!(err = "role mismatch"));
            }
            let path = self
                .service_config
                .base_url
                .as_ref()
                .ok_or_else(|| fatal_error!(err = "base_url not configured"))?
                .path();
            Ok(format!("{path}{}/", version.as_ref()))
        }

        pub(crate) async fn internal_add_task(
            &self,
            version: DapVersion,
            cmd: InternalTestAddTask,
        ) -> Result<(), DapError> {
            // VDAF config.
            let vdaf = match (
                cmd.vdaf.typ.as_ref(),
                cmd.vdaf.bits,
                cmd.vdaf.length,
                cmd.vdaf.chunk_length,
            ) {
                ("Prio3Count", None, None, None) => VdafConfig::Prio3(Prio3Config::Count),
                ("Prio3Sum", Some(bits), None, None) => VdafConfig::Prio3(Prio3Config::Sum {
                    bits: bits.parse().map_err(|e| fatal_error!(err = ?e, "failed to parse bits for Prio3Config::Sum"))?,
                }),
                ("Prio3SumVec", Some(bits), Some(length), Some(chunk_length)) => {
                    VdafConfig::Prio3(Prio3Config::SumVec {
                        bits: bits.parse().map_err(|e| fatal_error!(err = ?e, "failed to parse bits fro Prio3Config::SumVec"))?,
                        length: length.parse().map_err(|e| fatal_error!(err = ?e, "failed to parse length fro Prio3Config::SumVec"))?,
                        chunk_length: chunk_length.parse().map_err(|e| fatal_error!(err = ?e, "failed to parse chunk_length fro Prio3Config::SumVec"))?,
                    })
                }
                ("Prio3Histogram", None, Some(length), Some(chunk_length)) => {
                    VdafConfig::Prio3(Prio3Config::Histogram {
                        length: length.parse().map_err(|e| fatal_error!(err = ?e, "failed to parse length fro Prio3Config::Histogram"))?,
                        chunk_length: chunk_length.parse().map_err(|e| fatal_error!(err = ?e, "failed to parse chunk_length fro Prio3Config::Histogram"))?,
                    })
                }
                _ => return Err(fatal_error!(err = "command failed: unrecognized VDAF")),
            };

            // VDAF verification key.
            let vdaf_verify_key_data = decode_base64url_vec(cmd.vdaf_verify_key.as_bytes())
                .ok_or_else(|| {
                    fatal_error!(err = "VDAF verify key is not valid URL-safe base64")
                })?;
            let vdaf_verify_key = vdaf
                .get_decoded_verify_key(&vdaf_verify_key_data)
                .map_err(|e| fatal_error!(err = ?e, "failed to decode verify key"))?;

            // Collector HPKE config.
            let collector_hpke_config_data =
                decode_base64url_vec(cmd.collector_hpke_config.as_bytes()).ok_or_else(|| {
                    fatal_error!(err = "HPKE collector config is not valid URL-safe base64")
                })?;
            let collector_hpke_config = HpkeConfig::get_decoded(&collector_hpke_config_data)
                .map_err(|e| fatal_error!(err = ?e, "failed to decode hpke config"))?;

            // Leader authentication token.
            let token = BearerToken::from(cmd.leader_authentication_token);
            if self
                .kv()
                .put_if_not_exists::<kv::prefix::LeaderBearerToken>(&cmd.task_id, token)
                .await
                .map_err(|e| fatal_error!(err = ?e, "failed to fetch leader bearer token"))?
                .is_some()
            {
                return Err(fatal_error!(
                    err = "command failed: token already exists for the given task and bearer role (leader)",
                    task_id = %cmd.task_id,
                ));
            }

            // Collector authentication token.
            match (cmd.role, cmd.collector_authentication_token) {
                (DapRole::Leader, Some(token_string)) => {
                    let token = BearerToken::from(token_string);
                    if self
                        .kv()
                        .put_if_not_exists::<kv::prefix::CollectorBearerToken>(&cmd.task_id, token)
                        .await
                        .map_err(
                            |e| fatal_error!(err = ?e, "failed to put collector bearer token"),
                        )?
                        .is_some()
                    {
                        return Err(fatal_error!(err = format!(
                            "command failed: token already exists for the given task ({}) and bearer role (collector)",
                            cmd.task_id
                        )));
                    }
                }
                (DapRole::Leader, None) => {
                    return Err(fatal_error!(
                        err = "command failed: missing collector authentication token",
                    ))
                }
                (DapRole::Helper, None) => (),
                (DapRole::Helper, Some(..)) => {
                    return Err(fatal_error!(
                        err = "command failed: unexpected collector authentication token",
                    ));
                }
            };

            // Query configuraiton.
            let query = match (cmd.query_type, cmd.max_batch_size) {
                (1, None) => DapQueryConfig::TimeInterval,
                (1, Some(..)) => {
                    return Err(fatal_error!(
                        err = "command failed: unexpected max batch size"
                    ))
                }
                (2, max_batch_size) => DapQueryConfig::FixedSize { max_batch_size },
                _ => {
                    return Err(fatal_error!(
                        err = "command failed: unrecognized query type"
                    ))
                }
            };

            if self
                .kv()
                .put_if_not_exists_with_expiration::<kv::prefix::TaskConfig>(
                    &cmd.task_id,
                    DapTaskConfig {
                        version,
                        leader_url: cmd.leader,
                        helper_url: cmd.helper,
                        time_precision: cmd.time_precision,
                        not_before: self.get_current_time(),
                        not_after: cmd.task_expiration,
                        min_batch_size: cmd.min_batch_size,
                        query,
                        vdaf,
                        vdaf_verify_key,
                        collector_hpke_config,
                        method: Default::default(),
                        num_agg_span_shards: NonZeroUsize::new(4).unwrap(),
                    },
                    cmd.task_expiration,
                )
                .await
                .map_err(|e| fatal_error!(err = ?e, "failed to put task config in kv"))?
                .is_some()
            {
                Err(fatal_error!(
                    err = format!(
                        "command failed: config already exists for the given task ({})",
                        cmd.task_id
                    )
                ))
            } else {
                Ok(())
            }
        }

        pub(crate) async fn internal_add_hpke_config(
            &self,
            version: DapVersion,
            new_receiver: HpkeReceiverConfig,
        ) -> Result<(), DapError> {
            let mut config_list = self
                .kv()
                .get_cloned::<kv::prefix::HpkeReceiverConfigSet>(&version, &Default::default())
                .await
                .map_err(|e| fatal_error!(err = ?e, "failed to get hpke config"))?
                .unwrap_or_default();

            if config_list
                .iter()
                .any(|receiver| new_receiver.config.id == receiver.config.id)
            {
                return Err(fatal_error!(
                    err = format!(
                        "receiver config with id {} already exists",
                        new_receiver.config.id
                    )
                ));
            }

            config_list.push(new_receiver);

            self.kv()
                .put::<kv::prefix::HpkeReceiverConfigSet>(&version, config_list)
                .await
                .map_err(|e| fatal_error!(err = ?e, "failed to put hpke config"))?;
            Ok(())
        }
    }
}
