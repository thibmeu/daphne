use std::time::SystemTime;

use prio::codec::Encode;
use rand::prelude::*;
use serde_json::json;
use daphne::{hpke::{HpkeKemId, HpkeReceiverConfig}, messages::{encode_base64url, Base64Encode, TaskId}, vdaf::{Prio3Config, VdafConfig}};

fn main() {
    let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

    let leader_url = "http://localhost:8787/v09";
    let helper_url = "http://localhost:8788/v09";
    let leader_authentication_token = "I-am-the-leader";
    let collector_authentication_token = "I-am-the-collector";
    let min_batch_size = 1;
    let max_batch_size = 12;
    let time_precision = 3600;
    let task_expiration = now + 604_800;

    let mut rng = thread_rng();
    let task_id = TaskId(rng.gen()).to_base64url();
    let collector_hpke_config = HpkeReceiverConfig::gen(rng.gen(), HpkeKemId::X25519HkdfSha256).unwrap();

    let vdaf_config = &VdafConfig::Prio3(Prio3Config::Sum { bits: 8 });
    let vdaf_verify_key = vdaf_config.gen_verify_key();
    let vdaf = json!({
        "type": "Prio3Sum",
        "bits": "8",
    });

    let leader = json!({
        "task_id": task_id,
        "leader": leader_url,
        "helper": helper_url,
        "vdaf": vdaf.clone(),
        "leader_authentication_token": leader_authentication_token,
        "collector_authentication_token": collector_authentication_token,
        "role": "leader",
        "vdaf_verify_key": encode_base64url(&vdaf_verify_key),
        "query_type": 2,
        "min_batch_size": min_batch_size,
        "max_batch_size": max_batch_size,
        "time_precision": time_precision,
        "collector_hpke_config": encode_base64url(collector_hpke_config.config.get_encoded().unwrap()),
        "task_expiration": task_expiration,
    });
    println!("leader\n{}", serde_json::to_string_pretty(&leader).unwrap());

    let helper = json!({
        "task_id": task_id,
        "leader": leader_url,
        "helper": helper_url,
        "vdaf": vdaf.clone(),
        "leader_authentication_token": leader_authentication_token,
        "role": "helper",
        "vdaf_verify_key": encode_base64url(&vdaf_verify_key),
        "query_type": 2,
        "min_batch_size": min_batch_size,
        "max_batch_size": max_batch_size,
        "time_precision": time_precision,
        "collector_hpke_config": encode_base64url(collector_hpke_config.config.get_encoded().unwrap()),
        "task_expiration": task_expiration,
    });
    println!("helper\n{}", serde_json::to_string_pretty(&helper).unwrap());
}
