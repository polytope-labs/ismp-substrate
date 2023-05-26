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

//! Host implementation for ISMP
use crate::{
    dispatcher::Receipt, primitives::ConsensusClientProvider, Config, ConsensusClientUpdateTime,
    ConsensusStates, FrozenConsensusClients, FrozenHeights, IncomingRequestAcks,
    IncomingResponseAcks, LatestStateMachineHeight, Nonce, OutgoingRequestAcks, StateCommitments,
};
use alloc::{format, string::ToString};
use core::time::Duration;
use frame_support::traits::{Get, UnixTime};
use ismp_rs::{
    consensus::{
        ConsensusClient, ConsensusClientId, StateCommitment, StateMachineHeight, StateMachineId,
    },
    error::Error,
    host::{IsmpHost, StateMachine},
    router::{IsmpRouter, Request, Response},
    util::{hash_request, hash_response},
};
use sp_core::H256;
use sp_runtime::SaturatedConversion;
use sp_std::prelude::*;

/// An implementation for the IsmpHost
#[derive(Clone)]
pub struct Host<T: Config>(core::marker::PhantomData<T>);

impl<T: Config> Default for Host<T> {
    fn default() -> Self {
        Self(core::marker::PhantomData)
    }
}

impl<T: Config> IsmpHost for Host<T>
where
    <T as frame_system::Config>::Hash: From<H256>,
{
    fn host_state_machine(&self) -> StateMachine {
        T::StateMachine::get()
    }

    fn latest_commitment_height(&self, id: StateMachineId) -> Result<u64, Error> {
        LatestStateMachineHeight::<T>::get(id).map(|height| height).ok_or_else(|| {
            Error::ImplementationSpecific("Missing latest state machine height".to_string())
        })
    }

    fn state_machine_commitment(
        &self,
        height: StateMachineHeight,
    ) -> Result<StateCommitment, Error> {
        StateCommitments::<T>::get(height).ok_or_else(|| Error::StateCommitmentNotFound { height })
    }

    fn consensus_update_time(&self, id: ConsensusClientId) -> Result<Duration, Error> {
        ConsensusClientUpdateTime::<T>::get(id)
            .map(|timestamp| Duration::from_secs(timestamp))
            .ok_or_else(|| {
                Error::ImplementationSpecific(format!("Update time not found for {:?}", id))
            })
    }

    fn consensus_state(&self, id: ConsensusClientId) -> Result<Vec<u8>, Error> {
        ConsensusStates::<T>::get(id).ok_or_else(|| Error::ConsensusStateNotFound { id })
    }

    fn timestamp(&self) -> Duration {
        <T::TimeProvider as UnixTime>::now()
    }

    fn request_commitment(&self, req: &Request) -> Result<H256, Error> {
        let commitment = hash_request::<Self>(req);

        let _ = OutgoingRequestAcks::<T>::get(commitment.0.to_vec()).ok_or_else(|| {
            Error::RequestCommitmentNotFound {
                nonce: req.nonce(),
                source: req.source_chain(),
                dest: req.dest_chain(),
            }
        })?;

        Ok(commitment)
    }

    fn request_receipt(&self, req: &Request) -> Option<()> {
        let commitment = hash_request::<Self>(req);

        let _ = IncomingRequestAcks::<T>::get(commitment.0.to_vec())
            .ok_or_else(|| Error::RequestCommitmentNotFound {
                nonce: req.nonce(),
                source: req.source_chain(),
                dest: req.dest_chain(),
            })
            .ok()?;

        Some(())
    }

    fn store_consensus_state(&self, id: ConsensusClientId, state: Vec<u8>) -> Result<(), Error> {
        ConsensusStates::<T>::insert(id, state);
        Ok(())
    }

    fn store_consensus_update_time(
        &self,
        id: ConsensusClientId,
        timestamp: Duration,
    ) -> Result<(), Error> {
        ConsensusClientUpdateTime::<T>::insert(id, timestamp.as_secs().saturated_into::<u64>());
        Ok(())
    }

    fn store_state_machine_commitment(
        &self,
        height: StateMachineHeight,
        state: StateCommitment,
    ) -> Result<(), Error> {
        StateCommitments::<T>::insert(height, state);
        Ok(())
    }

    fn freeze_state_machine(&self, height: StateMachineHeight) -> Result<(), Error> {
        FrozenHeights::<T>::insert(height.id, height.height);
        Ok(())
    }

    fn store_latest_commitment_height(&self, height: StateMachineHeight) -> Result<(), Error> {
        LatestStateMachineHeight::<T>::insert(height.id, height.height);
        Ok(())
    }

    fn delete_request_commitment(&self, req: &Request) -> Result<(), Error> {
        let hash = hash_request::<Self>(req);
        // We can't delete actual leaves in the mmr so this serves as a replacement for that
        OutgoingRequestAcks::<T>::remove(hash.0.to_vec());
        Ok(())
    }

    fn store_request_receipt(&self, req: &Request) -> Result<(), Error> {
        let hash = hash_request::<Self>(req);
        IncomingRequestAcks::<T>::insert(hash.0.to_vec(), Receipt::Ok);
        Ok(())
    }

    fn consensus_client(&self, id: ConsensusClientId) -> Result<Box<dyn ConsensusClient>, Error> {
        <T as Config>::ConsensusClientProvider::consensus_client(id)
    }

    fn keccak256(bytes: &[u8]) -> H256
    where
        Self: Sized,
    {
        sp_io::hashing::keccak_256(bytes).into()
    }

    fn challenge_period(&self, id: ConsensusClientId) -> Duration {
        <T as Config>::ConsensusClientProvider::challenge_period(id)
    }

    fn ismp_router(&self) -> Box<dyn IsmpRouter> {
        Box::new(T::IsmpRouter::default())
    }

    fn is_state_machine_frozen(&self, machine: StateMachineHeight) -> Result<(), Error> {
        if let Some(frozen_height) = FrozenHeights::<T>::get(machine.id) {
            if machine.height >= frozen_height {
                Err(Error::FrozenStateMachine { height: machine })?
            }
        }
        Ok(())
    }

    fn is_consensus_client_frozen(&self, client: ConsensusClientId) -> Result<(), Error> {
        FrozenConsensusClients::<T>::get(client)
            .then(|| ())
            .ok_or_else(|| Error::FrozenConsensusClient { id: client })
    }

    fn next_nonce(&self) -> u64 {
        let nonce = Nonce::<T>::get();
        Nonce::<T>::put(nonce + 1);
        nonce
    }

    fn response_receipt(&self, res: &Response) -> Option<()> {
        let commitment = hash_response::<Self>(res);

        let _ = IncomingResponseAcks::<T>::get(commitment.0.to_vec())
            .ok_or_else(|| Error::ImplementationSpecific("Response receipt not found".to_string()))
            .ok()?;

        Some(())
    }

    fn freeze_consensus_client(&self, client: ConsensusClientId) -> Result<(), Error> {
        FrozenConsensusClients::<T>::insert(client, true);
        Ok(())
    }

    fn store_response_receipt(&self, res: &Response) -> Result<(), Error> {
        let hash = hash_response::<Self>(res);
        IncomingResponseAcks::<T>::insert(hash.0.to_vec(), Receipt::Ok);
        Ok(())
    }
}
