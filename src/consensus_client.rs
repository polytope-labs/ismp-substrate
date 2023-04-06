use crate::{
    Config, ConsensusClientUpdateTime, ConsensusStates, FrozenHeights, LatestStateMachineHeight,
    RequestAcks, StateCommitments, StateMachineUpdateTime,
};
use std::time::Duration;
use ismp_rust::consensus_client::{ConsensusClient, ConsensusClientId, IntermediateState};
use ismp_rust::error::Error;
use ismp_rust::host::ISMPHost;
use sync_committee_primitives::derived_types::LightClientState;

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, Default)]
pub struct ConsensusState<T: Config> {
    pub frozen_height: u64,
    pub consensus_client_id: ConsensusClientId,
    pub light_client_state: LightClientState
}

impl<T: Config> ConsensusClient for ConsensusState<T>  {
    fn verify(&self, host: &dyn ISMPHost, trusted_consensus_state: Vec<u8>, proof: Vec<u8>) -> Result<(Vec<u8>, Vec<IntermediateState>), Error> {
       todo!()
    }

    fn consensus_id(&self) -> ConsensusClientId {
        todo!()
    }

    fn unbonding_period(&self) -> Duration {
        todo!()
    }

    fn verify_membership(&self, host: &dyn ISMPHost, key: Vec<u8>, commitment: Vec<u8>) -> Result<(), Error> {
        todo!()
    }

    fn verify_non_membership(&self, host: &dyn ISMPHost, key: Vec<u8>, commitment: Vec<u8>) -> Result<(), Error> {
        todo!()
    }

    fn is_frozen(&self, host: &dyn ISMPHost, id: ConsensusClientId) -> Result<bool, Error> {
        todo!()
    }
}
