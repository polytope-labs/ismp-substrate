use crate::{
    Config, ConsensusClientUpdateTime, ConsensusStates, FrozenConsensusHeights, FrozenHeights,
    LatestStateMachineHeight, RequestAcks, StateCommitments, StateMachineUpdateTime,
};
use codec::{Decode, Encode};
use frame_support::traits::UnixTime;
use ismp_rust::consensus_client::{
    ConsensusClient, ConsensusClientId, IntermediateState, StateCommitment, StateMachineHeight,
    StateMachineId,
};
use ismp_rust::error::Error;
use ismp_rust::host::ISMPHost;
use std::time::Duration;
use sync_committee_primitives::derived_types::LightClientState;

#[derive(Debug, Encode, Decode, Clone)]
pub struct ConsensusState<T: Config> {
    pub frozen_height: u64,
    pub consensus_client_id: ConsensusClientId,
    pub light_client_state: LightClientState,
    pub state_machine_height: StateMachineHeight,
    pub state_commitment: StateCommitment,
    pub state_machine_id: StateMachineId,
    pub phantom_data: core::marker::PhantomData<T>,
}

// Unbonding period for relay chains in days
const UNBONDING_PERIOD: u64 = 14;
// number of seconds in a day
const DAY: u64 = 24 * 60 * 60;

impl<T: Config> ConsensusClient for ConsensusState<T> {
    fn verify(
        &self,
        host: &dyn ISMPHost,
        trusted_consensus_state: Vec<u8>,
        proof: Vec<u8>,
    ) -> Result<(Vec<u8>, Vec<IntermediateState>), Error> {
        // Ensure consensus client is not frozen
        if self.is_frozen(host, self.consensus_client_id)? {
            return Err(Error::FrozenConsensusClient {
                id: self.consensus_client_id,
            });
        }

        // check that the client hasn't elapsed unbonding period
        let timestamp = <T::TimeProvider as UnixTime>::now();
        if self.unbonding_period() > timestamp {
            // return the right error, need to update ismp_rust
        }

        // verify the encoding of the light client state
        let light_client_state = self.light_client_state.encode();
        if light_client_state != trusted_consensus_state {
            return Err(Error::ConsensusProofVerificationFailed {
                id: self.consensus_client_id,
            });
        }

        self.verify_membership(host, trusted_consensus_state, proof.clone())?;

        let mut intermediate_states = vec![];

        let intermediate_state = IntermediateState {
            height: self.state_machine_height.clone(),
            commitment: self.state_commitment.clone(),
        };

        intermediate_states.push(intermediate_state);

        Ok((proof.clone(), intermediate_states))
    }

    fn consensus_id(&self) -> ConsensusClientId {
        self.consensus_client_id
    }

    fn unbonding_period(&self) -> Duration {
        Duration::from_secs(UNBONDING_PERIOD * DAY)
    }

    fn verify_membership(
        &self,
        host: &dyn ISMPHost,
        key: Vec<u8>,
        commitment: Vec<u8>,
    ) -> Result<(), Error> {
        todo!()
    }

    fn verify_non_membership(
        &self,
        host: &dyn ISMPHost,
        key: Vec<u8>,
        commitment: Vec<u8>,
    ) -> Result<(), Error> {
        todo!()
    }

    fn is_frozen(&self, host: &dyn ISMPHost, id: ConsensusClientId) -> Result<bool, Error> {
        if let Some(frozen_height) = FrozenConsensusHeights::<T>::get(id) {
            Ok(self.frozen_height >= frozen_height)
        } else {
            Ok(false)
        }
    }
}
