use codec::{Decode, Encode};
use frame_support::traits::UnixTime;
use ismp_rust::consensus_client::{
    ConsensusClient, ConsensusClientId, IntermediateState, StateCommitment, StateMachineHeight,
    StateMachineId,
};
use ismp_rust::error::Error;
use ismp_rust::host::ISMPHost;
use std::time::Duration;
use sync_committee_primitives::derived_types::{LightClientState, LightClientUpdate};

const ETHEREUM_CONSENSUS_CLIENT_ID: u64 = 0;

#[derive(Debug, Encode, Decode, Clone)]
pub struct ConsensusState {
    pub frozen_height: Option<u64>,
    pub light_client_state: LightClientState,
}

#[derive(Encode, Decode)]
pub struct Misbehaviour {
    pub update_1: LightClientUpdate,
    pub update_2: LightClientUpdate,
}

#[derive(Encode, Decode)]
pub enum BeaconMessage {
    ConsensusUpdate(LightClientUpdate),
    Misbehaviour(Misbehaviour),
}

// Unbonding period for relay chains in days
const UNBONDING_PERIOD: u64 = 14;
// number of seconds in a day
const DAY: u64 = 24 * 60 * 60;

pub struct EthConsensusClient;

impl ConsensusClient for ConsensusState {
    fn verify(
        &self,
        host: &dyn ISMPHost,
        trusted_consensus_state: Vec<u8>,
        proof: Vec<u8>, // The light client update is the proof, use sync committee rs to verify
    ) -> Result<(Vec<u8>, Vec<IntermediateState>), Error> {
        //TODO: create proper error type for it in ISMP Rust

        let beacon_message = BeaconMessage::decode(&mut &proof[..])
            .map_err(|_| Error::CannotHandleConsensusMessage)?;

        let light_client_update;
        if let BeaconMessage::ConsensusUpdate(update) = beacon_message {
            light_client_update = update.clone();
        } else {
            //TODO: we still need to handle misbehaviour
            return Err(Error::CannotHandleConsensusMessage);
        }

        let light_client_state = LightClientState::decode(&mut &trusted_consensus_state[..])
            .map_err(|_| Error::CannotHandleConsensusMessage)?;

        let height = light_client_update.execution_payload.block_number;
        // Ensure consensus client is not frozen
        let is_frozen = if let Some(frozen_height) = self.frozen_height {
            height >= frozen_height
        } else {
            false
        };

        if is_frozen {
            return Err(Error::FrozenConsensusClient {
                id: self.consensus_id(),
            });
        }

        // check that the client hasn't elapsed unbonding period
        let timestamp = light_client_update.execution_payload.timestamp;
        if self.unbonding_period() > Duration::from_secs(timestamp) {
            // return the right error, need to update ismp_rust
        }

        let no_codec_light_client_state = light_client_state
            .try_into()
            .map_err(|_| Error::CannotHandleConsensusMessage)?;
        let no_codec_light_client_update = light_client_update
            .clone()
            .try_into()
            .map_err(|_| Error::CannotHandleConsensusMessage)?;

        let new_light_client_state = sync_committee_verifier::verify_sync_committee_attestation(
            no_codec_light_client_state,
            no_codec_light_client_update,
        )
        .map_err(|_| Error::ConsensusProofVerificationFailed {
            id: self.consensus_id(),
        })?;

        let mut intermediate_states = vec![];

        let commitment_root = light_client_update.execution_payload.state_root.clone();
        let intermediate_state = construct_intermediate_state(
            1,
            self.consensus_id(),
            height,
            timestamp,
            commitment_root,
        );

        intermediate_states.push(intermediate_state);

        Ok((proof.clone(), intermediate_states))
    }

    fn consensus_id(&self) -> ConsensusClientId {
        ETHEREUM_CONSENSUS_CLIENT_ID
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
        todo!()
    }
}

fn construct_intermediate_state(
    state_id: u64,
    consensus_client_id: u64,
    height: u64,
    timestamp: u64,
    commitment_root: Vec<u8>,
) -> IntermediateState {
    let state_machine_id = StateMachineId {
        state_id,
        consensus_client: consensus_client_id,
    };

    let state_machine_height = StateMachineHeight {
        id: state_machine_id,
        height,
    };

    let state_commitment = StateCommitment {
        timestamp,
        commitment_root,
    };

    let intermediate_state = IntermediateState {
        height: state_machine_height,
        commitment: state_commitment,
    };

    intermediate_state
}
