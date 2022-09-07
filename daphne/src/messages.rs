// Copyright (c) 2022 Cloudflare, Inc. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Messages in the DAP protocol.

use crate::DapTaskConfig;
use prio::codec::{
    decode_u16_items, decode_u32_items, encode_u16_items, encode_u32_items, CodecError, Decode,
    Encode,
};
use serde::{Deserialize, Serialize};
use std::{
    convert::{TryFrom, TryInto},
    fmt::Debug,
    io::{Cursor, Read},
};

const KEM_ID_X25519_HKDF_SHA256: u16 = 0x0020;
const KEM_ID_P256_HKDF_SHA256: u16 = 0x0010;
const KDF_ID_HKDF_SHA256: u16 = 0x0001;
const AEAD_ID_AES128GCM: u16 = 0x0001;
const QUERY_TYPE_TIME_INTERVAL: u16 = 0x0001;
const QUERY_TYPE_FIXED_SIZE: u16 = 0x0002;

/// The identifier for a DAP task.
#[derive(Clone, Debug, Default, Deserialize, Hash, PartialEq, Eq, Serialize)]
pub struct Id(#[serde(with = "hex")] pub [u8; 32]);

impl Id {
    /// Return the URL-safe, base64 encoding of the task ID.
    pub fn to_base64url(&self) -> String {
        base64::encode_config(&self.0, base64::URL_SAFE_NO_PAD)
    }

    /// Return the ID encoded as a hex string.
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }
}

impl Encode for Id {
    fn encode(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.0);
    }
}

impl Decode for Id {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        let mut data = [0; 32];
        bytes.read_exact(&mut data[..])?;
        Ok(Id(data))
    }
}

impl AsRef<[u8]> for Id {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// A duration.
pub type Duration = u64;

/// The timestamp sent in a [`Report`].
pub type Time = u64;

/// The nonce sent in a [`Report`].
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[allow(missing_docs)]
pub struct Nonce(pub [u8; 16]);

impl Encode for Nonce {
    fn encode(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.0);
    }
}

impl Decode for Nonce {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        let mut nonce = [0; 16];
        bytes.read_exact(&mut nonce)?;
        Ok(Nonce(nonce))
    }
}

impl AsRef<[u8]> for Nonce {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Report extensions.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Extension {
    Unhandled { typ: u16, payload: Vec<u8> },
}

impl Encode for Extension {
    fn encode(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::Unhandled { typ, payload } => {
                typ.encode(bytes);
                encode_u16_bytes(bytes, payload);
            }
        }
    }
}

impl Decode for Extension {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self::Unhandled {
            typ: u16::decode(bytes)?,
            payload: decode_u16_bytes(bytes)?,
        })
    }
}

/// Report metadata.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[allow(missing_docs)]
pub struct ReportMetadata {
    pub time: Time,
    pub nonce: Nonce,
    pub extensions: Vec<Extension>,
}

impl Encode for ReportMetadata {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.time.encode(bytes);
        self.nonce.encode(bytes);
        encode_u16_items(bytes, &(), &self.extensions);
    }
}

impl Decode for ReportMetadata {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self {
            time: Time::decode(bytes)?,
            nonce: Nonce::decode(bytes)?,
            extensions: decode_u16_items(&(), bytes)?,
        })
    }
}

/// A report generated by a client.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[allow(missing_docs)]
pub struct Report {
    pub task_id: Id,
    pub metadata: ReportMetadata,
    pub public_share: Vec<u8>,
    pub encrypted_input_shares: Vec<HpkeCiphertext>,
}

impl Encode for Report {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.task_id.encode(bytes);
        self.metadata.encode(bytes);
        encode_u32_bytes(bytes, &self.public_share);
        encode_u32_items(bytes, &(), &self.encrypted_input_shares);
    }
}

impl Decode for Report {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self {
            task_id: Id::decode(bytes)?,
            metadata: ReportMetadata::decode(bytes)?,
            public_share: decode_u32_bytes(bytes)?,
            encrypted_input_shares: decode_u32_items(&(), bytes)?,
        })
    }
}

/// An initial aggregate sub-request sent in an [`AggregateInitializeReq`]. The contents of this
/// structure pertain to a single report.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[allow(missing_docs)]
pub struct ReportShare {
    pub metadata: ReportMetadata,
    pub public_share: Vec<u8>,
    pub encrypted_input_share: HpkeCiphertext,
}

impl Encode for ReportShare {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.metadata.encode(bytes);
        encode_u32_bytes(bytes, &self.public_share);
        self.encrypted_input_share.encode(bytes);
    }
}

impl Decode for ReportShare {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self {
            metadata: ReportMetadata::decode(bytes)?,
            public_share: decode_u32_bytes(bytes)?,
            encrypted_input_share: HpkeCiphertext::decode(bytes)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BatchParameter {
    TimeInterval,
    FixedSize { batch_id: Id },
}

impl Encode for BatchParameter {
    fn encode(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::TimeInterval => QUERY_TYPE_TIME_INTERVAL.encode(bytes),
            Self::FixedSize { batch_id } => {
                QUERY_TYPE_FIXED_SIZE.encode(bytes);
                batch_id.encode(bytes);
            }
        }
    }
}

impl Decode for BatchParameter {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        match u16::decode(bytes)? {
            QUERY_TYPE_TIME_INTERVAL => Ok(Self::TimeInterval),
            QUERY_TYPE_FIXED_SIZE => Ok(Self::FixedSize {
                batch_id: Id::decode(bytes)?,
            }),
            _ => Err(CodecError::UnexpectedValue),
        }
    }
}

/// Aggregate initialization request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateInitializeReq {
    pub task_id: Id,
    pub agg_job_id: Id,
    pub agg_param: Vec<u8>,
    pub batch_param: BatchParameter,
    pub report_shares: Vec<ReportShare>,
}

impl Encode for AggregateInitializeReq {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.task_id.encode(bytes);
        self.agg_job_id.encode(bytes);
        encode_u16_bytes(bytes, &self.agg_param);
        self.batch_param.encode(bytes);
        encode_u32_items(bytes, &(), &self.report_shares);
    }
}

impl Decode for AggregateInitializeReq {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self {
            task_id: Id::decode(bytes)?,
            agg_job_id: Id::decode(bytes)?,
            agg_param: decode_u16_bytes(bytes)?,
            batch_param: BatchParameter::decode(bytes)?,
            report_shares: decode_u32_items(&(), bytes)?,
        })
    }
}

/// Aggregate continuation request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateContinueReq {
    pub task_id: Id,
    pub agg_job_id: Id,
    pub transitions: Vec<Transition>,
}

impl Encode for AggregateContinueReq {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.task_id.encode(bytes);
        self.agg_job_id.encode(bytes);
        encode_u32_items(bytes, &(), &self.transitions);
    }
}

impl Decode for AggregateContinueReq {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self {
            task_id: Id::decode(bytes)?,
            agg_job_id: Id::decode(bytes)?,
            transitions: decode_u32_items(&(), bytes)?,
        })
    }
}

/// Transition message. This conveyes a message sent from one Aggregator to another during the
/// preparation phase of VDAF evaluation.
//
// TODO spec: This is called `PrepareStep` in draft-ietf-ppm-dap-01. This is confusing because it
// overloads a term used in draft-irtf-cfrg-draft-01.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transition {
    pub nonce: Nonce,
    pub var: TransitionVar,
}

impl Encode for Transition {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.nonce.encode(bytes);
        self.var.encode(bytes);
    }
}

impl Decode for Transition {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self {
            nonce: Nonce::decode(bytes)?,
            var: TransitionVar::decode(bytes)?,
        })
    }
}

/// Transition message variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransitionVar {
    Continued(Vec<u8>),
    Finished,
    Failed(TransitionFailure),
}

impl Encode for TransitionVar {
    fn encode(&self, bytes: &mut Vec<u8>) {
        match self {
            TransitionVar::Continued(vdaf_message) => {
                0_u8.encode(bytes);
                encode_u32_bytes(bytes, vdaf_message);
            }
            TransitionVar::Finished => {
                1_u8.encode(bytes);
            }
            TransitionVar::Failed(err) => {
                2_u8.encode(bytes);
                err.encode(bytes);
            }
        }
    }
}

impl Decode for TransitionVar {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        match u8::decode(bytes)? {
            0 => Ok(Self::Continued(decode_u32_bytes(bytes)?)),
            1 => Ok(Self::Finished),
            2 => Ok(Self::Failed(TransitionFailure::decode(bytes)?)),
            _ => Err(CodecError::UnexpectedValue),
        }
    }
}

/// Transition error.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum TransitionFailure {
    BatchCollected = 0,
    ReportReplayed = 1,
    ReportDropped = 2,
    HpkeUnknownConfigId = 3,
    HpkeDecryptError = 4,
    VdafPrepError = 5,
    BatchSaturated = 6,
}

impl TryFrom<u8> for TransitionFailure {
    type Error = CodecError;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            b if b == Self::BatchCollected as u8 => Ok(Self::BatchCollected),
            b if b == Self::ReportReplayed as u8 => Ok(Self::ReportReplayed),
            b if b == Self::ReportDropped as u8 => Ok(Self::ReportDropped),
            b if b == Self::HpkeUnknownConfigId as u8 => Ok(Self::HpkeUnknownConfigId),
            b if b == Self::HpkeDecryptError as u8 => Ok(Self::HpkeDecryptError),
            b if b == Self::VdafPrepError as u8 => Ok(Self::VdafPrepError),
            b if b == Self::BatchSaturated as u8 => Ok(Self::BatchSaturated),
            _ => Err(CodecError::UnexpectedValue),
        }
    }
}

impl Encode for TransitionFailure {
    fn encode(&self, bytes: &mut Vec<u8>) {
        (*self as u8).encode(bytes);
    }
}

impl Decode for TransitionFailure {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        u8::decode(bytes)?.try_into()
    }
}

impl std::fmt::Display for TransitionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::BatchCollected => write!(f, "batch-collected({})", *self as u8),
            Self::ReportReplayed => write!(f, "report-replayed({})", *self as u8),
            Self::ReportDropped => write!(f, "report-dropped({})", *self as u8),
            Self::HpkeUnknownConfigId => write!(f, "hpke-unknown-config-id({})", *self as u8),
            Self::HpkeDecryptError => write!(f, "hpke-decrypt-error({})", *self as u8),
            Self::VdafPrepError => write!(f, "vdaf-prep-error({})", *self as u8),
            Self::BatchSaturated => write!(f, "batch-saturated({})", *self as u8),
        }
    }
}

/// An aggregate response sent from the Helper to the Leader.
#[derive(Debug, PartialEq, Eq, Default)]
#[allow(missing_docs)]
pub struct AggregateResp {
    pub transitions: Vec<Transition>,
}

impl Encode for AggregateResp {
    fn encode(&self, bytes: &mut Vec<u8>) {
        encode_u32_items(bytes, &(), &self.transitions);
    }
}

impl Decode for AggregateResp {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self {
            transitions: decode_u32_items(&(), bytes)?,
        })
    }
}

/// A batch interval.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[allow(missing_docs)]
pub struct Interval {
    pub start: Time,
    pub duration: Duration,
}

impl Interval {
    /// Return the end of the interval, i.e., `self.start + self.duration`.
    pub fn end(&self) -> Time {
        self.start + self.duration
    }

    /// Check that the batch interval is valid for the given task configuration.
    pub fn is_valid_for(&self, task_config: &DapTaskConfig) -> bool {
        if self.start % task_config.min_batch_duration != 0
            || self.duration % task_config.min_batch_duration != 0
            || self.duration < task_config.min_batch_duration
        {
            return false;
        }
        true
    }
}

impl Encode for Interval {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.start.encode(bytes);
        self.duration.encode(bytes);
    }
}

impl Decode for Interval {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self {
            start: Time::decode(bytes)?,
            duration: Duration::decode(bytes)?,
        })
    }
}

/// A query issued by the Collector in a collect request.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Query {
    TimeInterval { batch_interval: Interval },
    FixedSize { batch_id: Id },
}

impl Query {
    /// Return a reference to the query batch interval. Panics if the query type is not
    /// "time_interval".
    ///
    /// TODO(issue #100) Deprecate this once "fixed_size" support is added.
    pub fn unwrap_interval(&self) -> &Interval {
        if let BatchSelector::TimeInterval { ref batch_interval } = self {
            batch_interval
        } else {
            panic!("TODO(issue #100)");
        }
    }
}

impl Encode for Query {
    fn encode(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::TimeInterval { batch_interval } => {
                QUERY_TYPE_TIME_INTERVAL.encode(bytes);
                batch_interval.encode(bytes);
            }
            Self::FixedSize { batch_id } => {
                QUERY_TYPE_FIXED_SIZE.encode(bytes);
                batch_id.encode(bytes);
            }
        }
    }
}

impl Decode for Query {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        match u16::decode(bytes)? {
            QUERY_TYPE_TIME_INTERVAL => Ok(Self::TimeInterval {
                batch_interval: Interval::decode(bytes)?,
            }),
            QUERY_TYPE_FIXED_SIZE => Ok(Self::FixedSize {
                batch_id: Id::decode(bytes)?,
            }),
            _ => Err(CodecError::UnexpectedValue),
        }
    }
}

impl Default for Query {
    fn default() -> Self {
        Self::TimeInterval {
            batch_interval: Interval::default(),
        }
    }
}

/// A collect request.
//
// TODO Add serialization tests.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct CollectReq {
    pub task_id: Id,
    pub query: Query,
    pub agg_param: Vec<u8>,
}

impl Encode for CollectReq {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.task_id.encode(bytes);
        self.query.encode(bytes);
        encode_u16_bytes(bytes, &self.agg_param);
    }
}

impl Decode for CollectReq {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self {
            task_id: Id::decode(bytes)?,
            query: Query::decode(bytes)?,
            agg_param: decode_u16_bytes(bytes)?,
        })
    }
}

/// A collect response.
//
// TODO Add serialization tests.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct CollectResp {
    pub report_count: u64,
    pub encrypted_agg_shares: Vec<HpkeCiphertext>,
}

impl Encode for CollectResp {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.report_count.encode(bytes);
        encode_u32_items(bytes, &(), &self.encrypted_agg_shares);
    }
}

impl Decode for CollectResp {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self {
            report_count: u64::decode(bytes)?,
            encrypted_agg_shares: decode_u32_items(&(), bytes)?,
        })
    }
}

/// A batch selector issued by the Leader in an aggregate-share request.
//
// NOTE(cjpatton) This structure is identical to Query, hence the typedef. Eventually the
// strudcture might change (https://github.com/ietf-wg-ppm/draft-ietf-ppm-dap/issues/342), at which
// point we will need to make BatchSelector its own struct.
pub type BatchSelector = Query;

/// An aggregate-share request.
//
// TODO Add serialization tests.
#[derive(Debug, Default)]
pub struct AggregateShareReq {
    pub task_id: Id,
    pub batch_selector: BatchSelector,
    pub agg_param: Vec<u8>,
    pub report_count: u64,
    pub checksum: [u8; 32],
}

impl Encode for AggregateShareReq {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.task_id.encode(bytes);
        self.batch_selector.encode(bytes);
        encode_u16_bytes(bytes, &self.agg_param);
        self.report_count.encode(bytes);
        bytes.extend_from_slice(&self.checksum);
    }
}

impl Decode for AggregateShareReq {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self {
            task_id: Id::decode(bytes)?,
            batch_selector: BatchSelector::decode(bytes)?,
            agg_param: decode_u16_bytes(bytes)?,
            report_count: u64::decode(bytes)?,
            checksum: {
                let mut checksum = [0u8; 32];
                bytes.read_exact(&mut checksum[..])?;
                checksum
            },
        })
    }
}

/// An aggregate-share response.
//
// TODO Add serialization tests.
#[derive(Debug)]
pub struct AggregateShareResp {
    pub encrypted_agg_share: HpkeCiphertext,
}

impl Encode for AggregateShareResp {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.encrypted_agg_share.encode(bytes);
    }
}

impl Decode for AggregateShareResp {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self {
            encrypted_agg_share: HpkeCiphertext::decode(bytes)?,
        })
    }
}

/// Codepoint for KEM schemes compatible with HPKE.
#[derive(Clone, Copy, Deserialize, Serialize, Debug, PartialEq, Eq)]
pub enum HpkeKemId {
    P256HkdfSha256,
    X25519HkdfSha256,
    NotImplemented(u16),
}

impl From<HpkeKemId> for u16 {
    fn from(kem_id: HpkeKemId) -> Self {
        match kem_id {
            HpkeKemId::P256HkdfSha256 => KEM_ID_P256_HKDF_SHA256,
            HpkeKemId::X25519HkdfSha256 => KEM_ID_X25519_HKDF_SHA256,
            HpkeKemId::NotImplemented(x) => x,
        }
    }
}

impl Encode for HpkeKemId {
    fn encode(&self, bytes: &mut Vec<u8>) {
        u16::from(*self).encode(bytes);
    }
}

impl Decode for HpkeKemId {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        match u16::decode(bytes)? {
            x if x == KEM_ID_P256_HKDF_SHA256 => Ok(Self::P256HkdfSha256),
            x if x == KEM_ID_X25519_HKDF_SHA256 => Ok(Self::X25519HkdfSha256),
            x => Ok(Self::NotImplemented(x)),
        }
    }
}

/// Codepoint for KDF schemes compatible with HPKE.
#[derive(Clone, Copy, Deserialize, Serialize, Debug, PartialEq, Eq)]
pub enum HpkeKdfId {
    HkdfSha256,
    NotImplemented(u16),
}

impl From<HpkeKdfId> for u16 {
    fn from(kdf_id: HpkeKdfId) -> Self {
        match kdf_id {
            HpkeKdfId::HkdfSha256 => KDF_ID_HKDF_SHA256,
            HpkeKdfId::NotImplemented(x) => x,
        }
    }
}

impl Encode for HpkeKdfId {
    fn encode(&self, bytes: &mut Vec<u8>) {
        u16::from(*self).encode(bytes);
    }
}

impl Decode for HpkeKdfId {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        match u16::decode(bytes)? {
            x if x == KDF_ID_HKDF_SHA256 => Ok(Self::HkdfSha256),
            x => Ok(Self::NotImplemented(x)),
        }
    }
}

/// Codepoint for AEAD schemes compatible with HPKE.
#[derive(Clone, Copy, Deserialize, Serialize, Debug, PartialEq, Eq)]
pub enum HpkeAeadId {
    Aes128Gcm,
    NotImplemented(u16),
}

impl From<HpkeAeadId> for u16 {
    fn from(aead_id: HpkeAeadId) -> Self {
        match aead_id {
            HpkeAeadId::Aes128Gcm => AEAD_ID_AES128GCM,
            HpkeAeadId::NotImplemented(x) => x,
        }
    }
}

impl Encode for HpkeAeadId {
    fn encode(&self, bytes: &mut Vec<u8>) {
        u16::from(*self).encode(bytes);
    }
}

impl Decode for HpkeAeadId {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        match u16::decode(bytes)? {
            x if x == AEAD_ID_AES128GCM => Ok(Self::Aes128Gcm),
            x => Ok(Self::NotImplemented(x)),
        }
    }
}

/// The HPKE public key configuration of a Server.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct HpkeConfig {
    pub id: u8,
    pub kem_id: HpkeKemId,
    pub kdf_id: HpkeKdfId,
    pub aead_id: HpkeAeadId,
    // TODO Change this type to be the deserialized public key in order to avoid copying the
    // serialized key. We can't do this with rust-hpke because <X25519HkdfSha256 as Kem>::PublicKey
    // doesn't implement Debug. Eventually we'll replace rust-hpke with a more ergonomic
    // implementation that does. For now we'll eat the copy.
    #[serde(with = "hex")]
    pub(crate) public_key: Vec<u8>,
}

impl AsRef<HpkeConfig> for HpkeConfig {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl Encode for HpkeConfig {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.id.encode(bytes);
        self.kem_id.encode(bytes);
        self.kdf_id.encode(bytes);
        self.aead_id.encode(bytes);
        encode_u16_bytes(bytes, &self.public_key);
    }
}

impl Decode for HpkeConfig {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self {
            id: u8::decode(bytes)?,
            kem_id: HpkeKemId::decode(bytes)?,
            kdf_id: HpkeKdfId::decode(bytes)?,
            aead_id: HpkeAeadId::decode(bytes)?,
            public_key: decode_u16_bytes(bytes)?,
        })
    }
}

/// An HPKE ciphertext. In the DAP protocol, input shares and aggregate shares are encrypted to the
/// intended recipient.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct HpkeCiphertext {
    pub config_id: u8,
    #[serde(with = "hex")]
    pub enc: Vec<u8>,
    #[serde(with = "hex")]
    pub payload: Vec<u8>,
}

impl Encode for HpkeCiphertext {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.config_id.encode(bytes);
        encode_u16_bytes(bytes, &self.enc);
        encode_u32_bytes(bytes, &self.payload);
    }
}

impl Decode for HpkeCiphertext {
    fn decode(bytes: &mut Cursor<&[u8]>) -> Result<Self, CodecError> {
        Ok(Self {
            config_id: u8::decode(bytes)?,
            enc: decode_u16_bytes(bytes)?,
            payload: decode_u32_bytes(bytes)?,
        })
    }
}

// NOTE ring provides a similar function, but as of version 0.16.20, it doesn't compile to
// wasm32-unknown-unknown.
pub(crate) fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut r = 0;
    for (x, y) in left.iter().zip(right) {
        r |= x ^ y;
    }
    r == 0
}

pub(crate) fn encode_u16_bytes(bytes: &mut Vec<u8>, input: &[u8]) {
    u16::try_from(input.len())
        .expect("length too large for u16")
        .encode(bytes);
    bytes.extend_from_slice(input);
}

pub(crate) fn decode_u16_bytes(bytes: &mut Cursor<&[u8]>) -> Result<Vec<u8>, CodecError> {
    let len = u16::decode(bytes)? as usize;
    let mut out = vec![0; len];
    bytes.read_exact(&mut out)?;
    Ok(out)
}

pub(crate) fn encode_u32_bytes(bytes: &mut Vec<u8>, input: &[u8]) {
    u32::try_from(input.len())
        .expect("length too large for u32")
        .encode(bytes);
    bytes.extend_from_slice(input);
}

pub(crate) fn decode_u32_bytes(bytes: &mut Cursor<&[u8]>) -> Result<Vec<u8>, CodecError> {
    let len = u32::decode(bytes)? as usize;
    let mut out = vec![0; len];
    bytes.read_exact(&mut out)?;
    Ok(out)
}
