// Copyright (C) 2023 Polytope Labs.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Primitive types and traits used by the GRANDPA prover & verifier.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::all)]
#![deny(missing_docs)]

extern crate alloc;

use alloc::collections::BTreeMap;
use codec::{Decode, Encode};
use core::{fmt::Debug, time::Duration};
use frame_support::sp_runtime::Digest;
use ismp::{error::Error, host::StateMachine};
use sp_consensus_aura::{Slot, AURA_ENGINE_ID};
use sp_core::{sp_std, H256};
use sp_finality_grandpa::{AuthorityId, AuthorityList, AuthoritySignature};
use sp_runtime::{traits::Header, DigestItem};
use sp_std::prelude::*;
use sp_storage::StorageKey;

/// The `ConsensusEngineId` of ISMP digest in the parachain header.
pub const ISMP_ID: sp_runtime::ConsensusEngineId = *b"ISMP";

const SLOT_DURATION: u64 = 12_000;

/// GRANPA errors
pub mod error;
/// GRANDPA justification utilities
pub mod justification;

/// Represents a Hash in this library
pub type Hash = H256;
/// A commit message for this chain's block type.
pub type Commit<H> = finality_grandpa::Commit<
    <H as Header>::Hash,
    <H as Header>::Number,
    AuthoritySignature,
    AuthorityId,
>;

/// Finality for block B is proved by providing:
/// 1) the justification for the descendant block F;
/// 2) headers sub-chain (B; F] if B != F;
#[derive(Debug, PartialEq, Encode, Decode, Clone)]
pub struct FinalityProof<H: codec::Codec> {
    /// The hash of block F for which justification is provided.
    pub block: Hash,
    /// Justification of the block F.
    pub justification: Vec<u8>,
    /// The set of headers in the range (B; F] that we believe are unknown to the caller. Ordered.
    pub unknown_headers: Vec<H>,
}

/// Previous light client state.
#[derive(Debug, PartialEq, Encode, Decode, Clone)]
pub struct ConsensusState {
    /// Current authority set
    pub current_authorities: AuthorityList,
    /// Id of the current authority set.
    pub current_set_id: u64,
    /// latest finalized height on relay chain or standalone chain
    pub latest_height: u32,
    /// State machine id StateMachine::Polkadot(0) or StateMachine::Kusama(0) or
    ///StateMachine::Grandpa(ConsensusStateId)
    pub state_machine: StateMachine,
    /// latest finalized height on the parachains, this map will be empty for Standalone chains
    /// Map of para_ids
    pub para_ids: BTreeMap<u32, bool>,
    /// latest finalized hash on relay chain or standalone chain.
    pub latest_hash: Hash,
}

/// Holds relavant parachain proofs for both header and timestamp extrinsic.
#[derive(Clone, Debug, Encode, Decode)]
pub struct ParachainHeaderProofs {
    /// State proofs that prove a parachain header exists at a given relay chain height
    pub state_proof: Vec<Vec<u8>>,
    /// Timestamp extrinsic
    pub extrinsic: Vec<u8>,
    /// Timestamp extrinsic proof for previously proven parachain header.
    pub extrinsic_proof: Vec<Vec<u8>>,
    /// The parachain id
    pub para_id: u32,
}

/// Parachain headers with a Grandpa finality proof.
#[derive(Clone, Encode, Decode)]
pub struct ParachainHeadersWithFinalityProof<H: codec::Codec> {
    /// The grandpa finality proof: contains relay chain headers from the
    /// last known finalized grandpa block.
    pub finality_proof: FinalityProof<H>,
    /// Contains a map of relay chain header hashes to parachain headers
    /// finalzed at the relay chain height. We check for this parachain header finalization
    /// via state proofs. Also contains extrinsic proof for timestamp.
    pub parachain_headers: BTreeMap<Hash, Vec<ParachainHeaderProofs>>,
}

/// Hashing algorithm for the state proof
#[derive(Debug, Encode, Decode, Clone)]
#[cfg_attr(feature = "std", derive(serde::Deserialize, serde::Serialize))]
pub enum HashAlgorithm {
    /// For chains that use keccak as their hashing algo
    Keccak,
    /// For chains that use blake2 as their hashing algo
    Blake2,
}

/// Holds the relevant data needed for state proof verification
#[derive(Debug, Encode, Decode, Clone)]
pub struct SubstrateStateProof {
    /// Algorithm to use for state proof verification
    pub hasher: HashAlgorithm,
    /// Storage proof for the parachain headers
    pub storage_proof: Vec<Vec<u8>>,
}

/// Holds the relevant data needed for request/response proof verification
#[derive(Debug, Encode, Decode, Clone)]
pub struct MembershipProof {
    /// Size of the mmr at the time this proof was generated
    pub mmr_size: u64,
    /// Leaf indices for the proof
    pub leaf_indices: Vec<u64>,
    /// Mmr proof
    pub proof: Vec<H256>,
}

/// This returns the storage key for a parachain header on the relay chain.
pub fn parachain_header_storage_key(para_id: u32) -> StorageKey {
    let mut storage_key = frame_support::storage::storage_prefix(b"Paras", b"Heads").to_vec();
    let encoded_para_id = para_id.encode();
    storage_key.extend_from_slice(sp_io::hashing::twox_64(&encoded_para_id).as_slice());
    storage_key.extend_from_slice(&encoded_para_id);
    StorageKey(storage_key)
}

/// Fetches the overlay(ismp) root and timestamp from the header digest
pub fn fetch_overlay_root_and_timestamp(digest: &Digest) -> Result<(u64, H256), Error> {
    let (mut timestamp, mut overlay_root) = (0, H256::default());

    for digest in digest.logs.iter() {
        match digest {
            DigestItem::PreRuntime(consensus_engine_id, value)
                if *consensus_engine_id == AURA_ENGINE_ID =>
            {
                let slot = Slot::decode(&mut &value[..])
                    .map_err(|e| Error::ImplementationSpecific(format!("Cannot slot: {e:?}")))?;
                timestamp = Duration::from_millis(*slot * SLOT_DURATION).as_secs();
            }
            DigestItem::Consensus(consensus_engine_id, value)
                if *consensus_engine_id == ISMP_ID =>
            {
                if value.len() != 32 {
                    Err(Error::ImplementationSpecific(
                        "Header contains an invalid ismp root".into(),
                    ))?
                }

                overlay_root = H256::from_slice(&value);
            }
            // don't really care about the rest
            _ => {}
        };
    }

    Ok((timestamp, overlay_root))
}

/// Fetches the overlay(ismp) root from the header digest item
pub fn fetch_overlay_root(digest: &Digest) -> Result<H256, Error> {
    let mut overlay_root = H256::default();
    for digest in digest.logs.iter() {
        match digest {
            DigestItem::Consensus(consensus_engine_id, value)
                if *consensus_engine_id == ISMP_ID =>
            {
                if value.len() != 32 {
                    Err(Error::ImplementationSpecific(format!(
                        "Header contains an invalid ismp root"
                    )))?
                }

                overlay_root = H256::from_slice(&value);
            }
            // don't really care about the rest
            _ => {}
        };
    }

    Ok(overlay_root)
}
