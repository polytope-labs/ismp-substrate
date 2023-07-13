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
// See the License for the specific lang

use crate::consensus_message::ConsensusMessage;
use alloc::{boxed::Box, collections::BTreeMap, format, vec, vec::Vec};
use codec::{Decode, Encode};
use core::marker::PhantomData;
use ismp::{
    consensus::{ConsensusClient, ConsensusStateId, StateCommitment, StateMachineClient},
    error::Error,
    host::{IsmpHost, StateMachine},
    messaging::{Proof, StateCommitmentHeight},
    router::{Request, RequestResponse},
    util::hash_request,
};
use pallet_ismp::host::Host;
use primitive_types::H256;
use primitives::{
    fetch_overlay_root, fetch_overlay_root_and_timestamp, ConsensusState, HashAlgorithm,
    MembershipProof, ParachainHeadersWithFinalityProof, SubstrateStateProof,
};
use sp_runtime::traits::{BlakeTwo256, Header, Keccak256};
use sp_trie::{LayoutV0, StorageProof, Trie, TrieDBBuilder};
use verifier::{
    verify_grandpa_finality_proof, verify_parachain_headers_with_grandpa_finality_proof,
};

use ismp_primitives::mmr::{DataOrHash, Leaf, MmrHasher};
use merkle_mountain_range::MerkleProof;

pub const POLKADOT_CONSENSUS_STATE_ID: [u8; 8] = *b"polkadot";
pub const KUSAMA_CONSENSUS_STATE_ID: [u8; 8] = *b"_kusama_";

/// The `ConsensusEngineId` of ISMP digest in the parachain header.
pub const ISMP_ID: sp_runtime::ConsensusEngineId = *b"ISMP";

pub struct GrandpaConsensusClient<T>(PhantomData<T>);

/// The grandpa state machine implementation for ISMP.
pub struct GrandpaStateMachine<T>(PhantomData<T>);

impl<T> Default for GrandpaStateMachine<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T> Default for GrandpaConsensusClient<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T> ConsensusClient for GrandpaConsensusClient<T>
where
    T: pallet_ismp::Config + super::Config,
    T::BlockNumber: Into<u32>,
    T::Hash: From<H256>,
{
    fn verify_consensus(
        &self,
        _host: &dyn IsmpHost,
        _consensus_state_id: ConsensusStateId,
        trusted_consensus_state: Vec<u8>,
        proof: Vec<u8>,
    ) -> Result<(Vec<u8>, BTreeMap<StateMachine, StateCommitmentHeight>), Error> {
        // decode the proof into consensus message
        let consensus_message: ConsensusMessage =
            codec::Decode::decode(&mut &proof[..]).map_err(|e| {
                Error::ImplementationSpecific(format!(
                    "Cannot decode consensus message from proof: {e:?}",
                ))
            })?;

        // decode the consensus state
        let consensus_state: ConsensusState =
            codec::Decode::decode(&mut &trusted_consensus_state[..]).map_err(|e| {
                Error::ImplementationSpecific(format!(
                    "Cannot decode consensus state from trusted consensus state bytes: {e:?}",
                ))
            })?;

        let mut intermediates = BTreeMap::new();

        // match over the message
        match consensus_message {
            ConsensusMessage::RelayChainMessage(relay_chain_message) => {
                let headers_with_finality_proof = ParachainHeadersWithFinalityProof {
                    finality_proof: relay_chain_message.finality_proof,
                    parachain_headers: relay_chain_message.parachain_headers,
                };

                let (derived_consensus_state, parachain_headers) =
                    verify_parachain_headers_with_grandpa_finality_proof(
                        consensus_state.clone(),
                        headers_with_finality_proof,
                    )
                    .map_err(|_| {
                        Error::ImplementationSpecific(format!("Error verifying parachain headers"))
                    })?;

                for (_para_id, header_vec) in parachain_headers {
                    //let (header, timestamp) = header_vec.get(0).unwrap();
                    for (header, timestamp) in header_vec {
                        let overlay_root = fetch_overlay_root(header.digest())?;

                        if timestamp == 0 {
                            Err(Error::ImplementationSpecific(
                                "Timestamp or ismp root not found".into(),
                            ))?
                        }

                        let height: u32 = (*header.number()).into();

                        let state_id: StateMachine = match consensus_state.state_machine {
                            StateMachine::Grandpa(_) => {
                                StateMachine::Grandpa(header.number().clone().to_le_bytes())
                            }
                            _ => Err(Error::ImplementationSpecific(
                                "Host state machine should be a parachain".into(),
                            ))?,
                        };

                        let intermediate = StateCommitmentHeight {
                            commitment: StateCommitment {
                                timestamp,
                                overlay_root: Some(overlay_root),
                                state_root: header.state_root,
                            },
                            height: height.into(),
                        };

                        intermediates.insert(state_id, intermediate);
                    }
                }

                Ok((derived_consensus_state.encode(), intermediates))
            }

            ConsensusMessage::StandaloneChainMessage(standalone_chain_message) => {
                let (derived_consensus_state, header, _, _) = verify_grandpa_finality_proof(
                    consensus_state.clone(),
                    standalone_chain_message.finality_proof,
                )
                .map_err(|_| {
                    Error::ImplementationSpecific(
                        "Error verifying parachain headers".parse().unwrap(),
                    )
                })?;

                let (timestamp, overlay_root) = fetch_overlay_root_and_timestamp(header.digest())?;

                if timestamp == 0 {
                    Err(Error::ImplementationSpecific("Timestamp or ismp root not found".into()))?
                }

                let height: u32 = (*header.number()).into();

                let state_id = consensus_state.state_machine;

                let intermediate = StateCommitmentHeight {
                    commitment: StateCommitment {
                        timestamp,
                        overlay_root: Some(overlay_root),
                        state_root: header.state_root,
                    },
                    height: height.into(),
                };

                intermediates.insert(state_id, intermediate);

                Ok((derived_consensus_state.encode(), intermediates))
            }
        }
    }

    fn verify_fraud_proof(
        &self,
        _host: &dyn IsmpHost,
        _trusted_consensus_state: Vec<u8>,
        _proof_1: Vec<u8>,
        _proof_2: Vec<u8>,
    ) -> Result<(), Error> {
        todo!()
    }

    fn state_machine(&self, _id: StateMachine) -> Result<Box<dyn StateMachineClient>, Error> {
        todo!()
    }
}

impl<T> StateMachineClient for GrandpaStateMachine<T>
where
    T: pallet_ismp::Config + super::Config,
    T::BlockNumber: Into<u32>,
    T::Hash: From<H256>,
{
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
            .overlay_root
            .ok_or_else(|| Error::ImplementationSpecific("ISMP root should not be None".into()))?;

        let calc_root = proof
            .calculate_root(leaves.clone())
            .map_err(|e| Error::ImplementationSpecific(format!("Error verifying mmr: {e:?}")))?;
        let valid = calc_root.hash::<Host<T>>() == root.clone().into();

        if !valid {
            Err(Error::ImplementationSpecific("Invalid membership proof".into()))?
        }

        Ok(())
    }

    fn state_trie_key(&self, requests: Vec<Request>) -> Vec<Vec<u8>> {
        let mut keys = vec![];

        for req in requests {
            match req {
                Request::Post(post) => {
                    let request = Request::Post(post);
                    let commitment = hash_request::<Host<T>>(&request).0.to_vec();
                    keys.push(pallet_ismp::RequestReceipts::<T>::hashed_key_for(commitment));
                }
                Request::Get(_) => continue,
            }
        }

        keys
    }

    fn verify_state_proof(
        &self,
        _host: &dyn IsmpHost,
        keys: Vec<Vec<u8>>,
        root: StateCommitment,
        proof: &Proof,
    ) -> Result<BTreeMap<Vec<u8>, Option<Vec<u8>>>, Error> {
        let state_proof: SubstrateStateProof = codec::Decode::decode(&mut &*proof.proof)
            .map_err(|e| Error::ImplementationSpecific(format!("failed to decode proof: {e:?}")))?;

        let data = match state_proof.hasher {
            HashAlgorithm::Keccak => {
                let db = StorageProof::new(state_proof.storage_proof).into_memory_db::<Keccak256>();
                let trie = TrieDBBuilder::<LayoutV0<Keccak256>>::new(&db, &root.state_root).build();
                keys.into_iter()
                    .map(|key| {
                        let value = trie.get(&key).map_err(|e| {
                            Error::ImplementationSpecific(format!(
                                "Error reading state proof: {e:?}"
                            ))
                        })?;
                        Ok((key, value))
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?
            }
            HashAlgorithm::Blake2 => {
                let db =
                    StorageProof::new(state_proof.storage_proof).into_memory_db::<BlakeTwo256>();

                let trie =
                    TrieDBBuilder::<LayoutV0<BlakeTwo256>>::new(&db, &root.state_root).build();
                keys.into_iter()
                    .map(|key| {
                        let value = trie.get(&key).map_err(|e| {
                            Error::ImplementationSpecific(format!(
                                "Error reading state proof: {e:?}"
                            ))
                        })?;
                        Ok((key, value))
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?
            }
        };

        Ok(data)
    }
}
