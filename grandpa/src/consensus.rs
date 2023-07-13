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
use alloc::collections::BTreeMap;
use codec::Encode;
use core::marker::PhantomData;
use ismp::{
    consensus::{ConsensusClient, ConsensusStateId, StateCommitment, StateMachineClient},
    error::Error,
    host::{IsmpHost, StateMachine},
    messaging::StateCommitmentHeight,
};
use ismp_primitives::fetch_overlay_root_and_timestamp;
use primitive_types::H256;
use primitives::{ConsensusState, ParachainHeadersWithFinalityProof};
use sp_runtime::traits::Header;
use state_machine_primitives::SubstrateStateMachine;
use verifier::{
    verify_grandpa_finality_proof, verify_parachain_headers_with_grandpa_finality_proof,
};

pub const POLKADOT_CONSENSUS_STATE_ID: [u8; 8] = *b"polkadot";
pub const KUSAMA_CONSENSUS_STATE_ID: [u8; 8] = *b"_kusama_";

/// The `ConsensusEngineId` of ISMP digest in the parachain header.
pub const ISMP_ID: sp_runtime::ConsensusEngineId = *b"ISMP";

pub struct GrandpaConsensusClient<T>(PhantomData<T>);

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

                let (consensus_state, parachain_headers) =
                    verify_parachain_headers_with_grandpa_finality_proof(
                        consensus_state,
                        headers_with_finality_proof,
                    )
                    .map_err(|_| {
                        Error::ImplementationSpecific(format!("Error verifying parachain headers"))
                    })?;

                for (para_id, header_vec) in parachain_headers {
                    for header in header_vec {
                        let (timestamp, overlay_root) = fetch_overlay_root_and_timestamp(
                            header.digest(),
                            consensus_state.slot_duration,
                        )?;

                        if timestamp == 0 {
                            Err(Error::ImplementationSpecific(
                                "Timestamp or ismp root not found".into(),
                            ))?
                        }

                        let height: u32 = (*header.number()).into();

                        let state_id: StateMachine = match consensus_state.state_machine {
                            StateMachine::Polkadot(_) => StateMachine::Polkadot(para_id),
                            StateMachine::Kusama(_) => StateMachine::Kusama(para_id),
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

                Ok((consensus_state.encode(), intermediates))
            }

            ConsensusMessage::StandaloneChainMessage(standalone_chain_message) => {
                let (consensus_state, header, _, _) = verify_grandpa_finality_proof(
                    consensus_state,
                    standalone_chain_message.finality_proof,
                )
                .map_err(|_| {
                    Error::ImplementationSpecific(
                        "Error verifying parachain headers".parse().unwrap(),
                    )
                })?;

                let (timestamp, overlay_root) = fetch_overlay_root_and_timestamp(
                    header.digest(),
                    consensus_state.slot_duration,
                )?;

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

                Ok((consensus_state.encode(), intermediates))
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
        Ok(Box::new(SubstrateStateMachine::<T>::default()))
    }
}
