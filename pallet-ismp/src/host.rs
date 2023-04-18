use crate::{
    consensus_clients::{
        beacon_consensus_client::beacon_client::BeaconConsensusClient,
        consensus_client_ids::ETHEREUM_CONSENSUS_CLIENT_ID,
    },
    router::Router,
    Config, ConsensusClientUpdateTime, ConsensusStates, FrozenHeights, LatestStateMachineHeight,
    RequestAcks, StateCommitments,
};
use alloc::{format, string::ToString};
use core::time::Duration;
use ethabi::ethereum_types::H256;
use frame_support::traits::UnixTime;
use ismp_rs::{
    consensus_client::{
        ConsensusClient, ConsensusClientId, StateCommitment, StateMachineHeight, StateMachineId,
    },
    error::Error,
    host::{ChainID, ISMPHost},
    router::{ISMPRouter, Request},
    util::hash_request,
};
use sp_runtime::SaturatedConversion;
use sp_std::prelude::*;

#[derive(Clone)]
pub struct Host<T: Config>(core::marker::PhantomData<T>);

impl<T: Config> Default for Host<T> {
    fn default() -> Self {
        Self(core::marker::PhantomData)
    }
}

impl<T: Config> ISMPHost for Host<T>
where
    <T as frame_system::Config>::Hash: From<H256>,
{
    fn host(&self) -> ChainID {
        <T as Config>::CHAIN_ID
    }

    fn latest_commitment_height(&self, id: StateMachineId) -> Result<StateMachineHeight, Error> {
        LatestStateMachineHeight::<T>::get(id)
            .map(|height| StateMachineHeight { id, height })
            .ok_or_else(|| {
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

    fn is_frozen(&self, height: StateMachineHeight) -> Result<bool, Error> {
        if let Some(frozen_height) = FrozenHeights::<T>::get(height.id) {
            Ok(height.height >= frozen_height)
        } else {
            Ok(false)
        }
    }

    fn request_commitment(&self, req: &Request) -> Result<H256, Error> {
        let commitment = hash_request::<Self>(req);

        let _ = RequestAcks::<T>::get(commitment.0.to_vec()).ok_or_else(|| {
            Error::RequestCommitmentNotFound {
                nonce: req.nonce(),
                source: req.source_chain(),
                dest: req.dest_chain(),
            }
        })?;

        Ok(commitment)
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

    fn consensus_client(&self, id: ConsensusClientId) -> Result<Box<dyn ConsensusClient>, Error> {
        match id {
            ETHEREUM_CONSENSUS_CLIENT_ID => Ok(Box::new(BeaconConsensusClient::<Self>::default())),
            _ => Err(Error::ImplementationSpecific(format!(
                "No consensus client found for consensus id {:?}",
                id
            ))),
        }
    }

    fn challenge_period(&self, id: ConsensusClientId) -> Duration {
        match id {
            id if id == ETHEREUM_CONSENSUS_CLIENT_ID => Duration::from_secs(30 * 60),
            _ => Duration::from_secs(15 * 60),
        }
    }

    fn ismp_router(&self) -> Box<dyn ISMPRouter> {
        Box::new(Router::<T>::default())
    }

    fn store_latest_commitment_height(&self, height: StateMachineHeight) -> Result<(), Error> {
        LatestStateMachineHeight::<T>::insert(height.id, height.height);
        Ok(())
    }

    fn keccak256(bytes: &[u8]) -> H256
    where
        Self: Sized,
    {
        sp_io::hashing::keccak_256(bytes).into()
    }
}
