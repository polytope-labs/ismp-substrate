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

//! The parachain consensus client module

use core::{marker::PhantomData, time::Duration};

use alloc::{collections::BTreeMap, format, vec, vec::Vec};
use codec::{Decode, Encode};
use core::fmt::Debug;
use ismp::{
    consensus::{
        ConsensusClient, ConsensusClientId, IntermediateState, StateCommitment, StateMachineHeight,
        StateMachineId,
    },
    error::Error,
    host::{IsmpHost, StateMachine},
    messaging::Proof,
    router::RequestResponse,
};
use ismp_primitives::mmr::{DataOrHash, Leaf, MmrHasher};
use merkle_mountain_range::MerkleProof;
use pallet_ismp::host::Host;
use parachain_system::{RelaychainDataProvider, RelaychainStateProvider};
use primitive_types::H256;
use sp_consensus_aura::{Slot, AURA_ENGINE_ID};
use sp_runtime::{
    app_crypto::sp_core::storage::StorageKey,
    generic::Header,
    traits::{BlakeTwo256, Header as _, Keccak256},
    DigestItem,
};
use sp_trie::{HashDBT, LayoutV0, StorageProof, Trie, TrieDBBuilder, EMPTY_PREFIX};

use crate::RelayChainOracle;

/// The parachain consensus client implementation for ISMP.
pub struct ParachainConsensusClient<T, R>(PhantomData<(T, R)>);

impl<T, R> Default for ParachainConsensusClient<T, R> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

/// Information necessary to prove the sibling parachain's finalization to this
/// parachain.
#[derive(Debug, Encode, Decode)]
pub struct ParachainConsensusProof {
    /// List of para ids contained in the proof
    pub para_ids: Vec<u32>,
    /// Height of the relay chain for the given proof
    pub relay_height: u32,
    /// Storage proof for the parachain headers
    pub storage_proof: Vec<Vec<u8>>,
}

/// Hashing algorithm for the state proof
#[derive(Debug, Encode, Decode)]
pub enum HashAlgorithm {
    Keccak,
    Blake2,
}

/// Holds the relevant data needed for state proof verification
#[derive(Debug, Encode, Decode)]
pub struct ParachainStateProof {
    /// Algorithm to use for state proof verification
    pub hasher: HashAlgorithm,
    /// Storage proof for the parachain headers
    pub storage_proof: Vec<Vec<u8>>,
}

/// Holds the relevant data needed for request/response proof verification
#[derive(Debug, Encode, Decode)]
pub struct MembershipProof {
    /// Size of the mmr at the time this proof was generated
    pub mmr_size: u64,
    /// Leaf indices for the proof
    pub leaf_indices: Vec<u64>,
    /// Mmr proof
    pub proof: Vec<H256>,
}

/// The `ConsensusEngineId` of ISMP digest in the parachain header.
pub const ISMP_ID: sp_runtime::ConsensusEngineId = *b"ISMP";

/// ConsensusClientId for [`ParachainConsensusClient`]
pub const PARACHAIN_CONSENSUS_ID: ConsensusClientId = *b"PARA";

/// Slot duration in milliseconds
const SLOT_DURATION: u64 = 12_000;

impl<T, R> ConsensusClient for ParachainConsensusClient<T, R>
where
    R: RelayChainOracle,
    T: pallet_ismp::Config + super::Config,
    T::BlockNumber: Into<u32>,
    T::Hash: From<H256>,
{
    fn verify_consensus(
        &self,
        host: &dyn IsmpHost,
        state: Vec<u8>,
        proof: Vec<u8>,
    ) -> Result<(Vec<u8>, Vec<IntermediateState>), Error> {
        let update: ParachainConsensusProof =
            codec::Decode::decode(&mut &proof[..]).map_err(|e| {
                Error::ImplementationSpecific(format!(
                    "Cannot decode parachain consensus proof: {e:?}"
                ))
            })?;

        // first check our oracle's registry
        let root = R::state_root(update.relay_height)
            // not in our registry? ask parachain_system.
            .or_else(|| {
                let state = RelaychainDataProvider::<T>::current_relay_chain_state();

                if state.number == update.relay_height {
                    Some(state.state_root)
                } else {
                    None
                }
            })
            // well, we couldn't find it
            .ok_or_else(|| {
                Error::ImplementationSpecific(format!(
                    "Cannot find relay chain height: {}",
                    update.relay_height
                ))
            })?;

        let storage_proof = StorageProof::new(update.storage_proof);
        let mut intermediates = vec![];

        let keys = update.para_ids.iter().map(|id| parachain_header_storage_key(*id).0);
        let headers =
            read_proof_check::<BlakeTwo256, _>(&root, storage_proof, keys).map_err(|e| {
                Error::ImplementationSpecific(format!("Error verifying parachain header {e:?}",))
            })?;

        for (key, header) in headers {
            let id = codec::Decode::decode(&mut &key[(key.len() - 4)..]).map_err(|e| {
                Error::ImplementationSpecific(format!("Error decoding parachain header: {e}"))
            })?;
            let header = header.ok_or_else(|| {
                Error::ImplementationSpecific(format!(
                    "Cannot find parachain header for ParaId({id})",
                ))
            })?;
            // ideally all parachain headers are the same
            let header = Header::<u32, BlakeTwo256>::decode(&mut &*header).map_err(|e| {
                Error::ImplementationSpecific(format!("Error decoding parachain header: {e}"))
            })?;

            let (mut timestamp, mut ismp_root) = (0, H256::default());
            for digest in header.digest().logs.iter() {
                match digest {
                    DigestItem::PreRuntime(consensus_engine_id, value)
                        if *consensus_engine_id == AURA_ENGINE_ID =>
                    {
                        let slot = Slot::decode(&mut &value[..]).map_err(|e| {
                            Error::ImplementationSpecific(format!("Cannot slot: {e:?}"))
                        })?;
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

                        ismp_root = H256::from_slice(&value);
                    }
                    // don't really care about the rest
                    _ => {}
                };
            }

            if timestamp == 0 {
                Err(Error::ImplementationSpecific("Timestamp or ismp root not found".into()))?
            }

            let height: u32 = (*header.number()).into();

            let state_id = match host.host_state_machine() {
                StateMachine::Kusama(_) => StateMachine::Kusama(id),
                StateMachine::Polkadot(_) => StateMachine::Polkadot(id),
                _ => Err(Error::ImplementationSpecific(
                    "Host state machine should be a parachain".into(),
                ))?,
            };

            let intermediate = IntermediateState {
                height: StateMachineHeight {
                    id: StateMachineId { state_id, consensus_client: PARACHAIN_CONSENSUS_ID },
                    height: height as u64,
                },
                commitment: StateCommitment {
                    timestamp,
                    ismp_root: Some(ismp_root),
                    state_root: H256::from_slice(header.state_root().as_ref()),
                },
            };

            intermediates.push(intermediate);
        }

        Ok((state, intermediates))
    }

    fn unbonding_period(&self) -> Duration {
        // there's no notion of client expiry, since there's shared security.
        Duration::from_secs(u64::MAX)
    }

    fn verify_membership(
        &self,
        _host: &dyn IsmpHost,
        item: RequestResponse,
        state: StateCommitment,
        proof: &Proof,
    ) -> Result<(), Error> {
        let membership = MembershipProof::decode(&mut &*proof.proof).map_err(|e| {
            Error::ImplementationSpecific(format!("Cannot decode membership proof: {e:?}"))
        })?;
        let nodes = membership.proof.into_iter().map(|h| DataOrHash::Hash(h.into())).collect();
        let proof =
            MerkleProof::<DataOrHash<T>, MmrHasher<T, Host<T>>>::new(membership.mmr_size, nodes);
        let leaves: Vec<(u64, DataOrHash<T>)> = match item {
            RequestResponse::Request(req) => membership
                .leaf_indices
                .into_iter()
                .zip(req.into_iter())
                .map(|(pos, req)| (pos, DataOrHash::Data(Leaf::Request(req))))
                .collect(),
            RequestResponse::Response(res) => membership
                .leaf_indices
                .into_iter()
                .zip(res.into_iter())
                .map(|(pos, res)| (pos, DataOrHash::Data(Leaf::Response(res))))
                .collect(),
        };
        let root = state
            .ismp_root
            .ok_or_else(|| Error::ImplementationSpecific("ISMP root should not be None".into()))?;

        let calc_root = proof
            .calculate_root(leaves.clone())
            .map_err(|e| Error::ImplementationSpecific(format!("Error verifying mmr: {e:?}")))?;
        let valid = calc_root.hash::<Host<T>>() == root.into();

        if !valid {
            Err(Error::ImplementationSpecific("Invalid membership proof".into()))?
        }

        Ok(())
    }

    fn state_trie_key(&self, _request: RequestResponse) -> Vec<Vec<u8>> {
        todo!()
    }

    fn verify_state_proof(
        &self,
        _host: &dyn IsmpHost,
        keys: Vec<Vec<u8>>,
        root: StateCommitment,
        proof: &Proof,
    ) -> Result<Vec<Option<Vec<u8>>>, Error> {
        let state_proof: ParachainStateProof = codec::Decode::decode(&mut &*proof.proof)
            .map_err(|e| Error::ImplementationSpecific(format!("failed to decode proof: {e:?}")))?;

        let data = match state_proof.hasher {
            HashAlgorithm::Keccak => {
                let db = StorageProof::new(state_proof.storage_proof).into_memory_db::<Keccak256>();
                let trie = TrieDBBuilder::<LayoutV0<Keccak256>>::new(&db, &root.state_root).build();
                keys.into_iter()
                    .map(|key| {
                        trie.get(&key).map_err(|e| {
                            Error::ImplementationSpecific(format!(
                                "Error reading state proof: {e:?}"
                            ))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
            HashAlgorithm::Blake2 => {
                let db =
                    StorageProof::new(state_proof.storage_proof).into_memory_db::<BlakeTwo256>();

                let trie =
                    TrieDBBuilder::<LayoutV0<BlakeTwo256>>::new(&db, &root.state_root).build();
                keys.into_iter()
                    .map(|key| {
                        trie.get(&key).map_err(|e| {
                            Error::ImplementationSpecific(format!(
                                "Error reading state proof: {e:?}"
                            ))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
        };

        Ok(data)
    }

    fn is_frozen(&self, _: &[u8]) -> Result<(), Error> {
        // parachain consensus client can never be frozen.
        Ok(())
    }
}

/// This returns the storage key for a parachain header on the relay chain.
pub fn parachain_header_storage_key(para_id: u32) -> StorageKey {
    let mut storage_key = frame_support::storage::storage_prefix(b"Paras", b"Heads").to_vec();
    let encoded_para_id = para_id.encode();
    storage_key.extend_from_slice(sp_io::hashing::twox_64(&encoded_para_id).as_slice());
    storage_key.extend_from_slice(&encoded_para_id);
    StorageKey(storage_key)
}

/// Lifted directly from [`sp_state_machine::read_proof_check`](https://github.com/paritytech/substrate/blob/b27c470eaff379f512d1dec052aff5d551ed3b03/primitives/state-machine/src/lib.rs#L1075-L1094)
pub fn read_proof_check<H, I>(
    root: &H::Out,
    proof: StorageProof,
    keys: I,
) -> Result<BTreeMap<Vec<u8>, Option<Vec<u8>>>, Error>
where
    H: hash_db::Hasher,
    H::Out: Debug,
    I: IntoIterator,
    I::Item: AsRef<[u8]>,
{
    let db = proof.into_memory_db();

    if !db.contains(root, EMPTY_PREFIX) {
        Err(Error::ImplementationSpecific("Invalid Proof".into()))?
    }

    let trie = TrieDBBuilder::<LayoutV0<H>>::new(&db, root).build();
    let mut result = BTreeMap::new();

    for key in keys.into_iter() {
        let value = trie
            .get(key.as_ref())
            .map_err(|e| Error::ImplementationSpecific(format!("Error reading from trie: {e:?}")))?
            .and_then(|val| Decode::decode(&mut &val[..]).ok());
        result.insert(key.as_ref().to_vec(), value);
    }

    Ok(result)
}
