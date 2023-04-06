use codec::{Decode, Encode};
use ismp_rust::consensus_client::{
    ConsensusClient, ConsensusClientId, IntermediateState, StateCommitment, StateMachineHeight,
    StateMachineId, ETHEREUM_CONSENSUS_CLIENT_ID,
};
use ismp_rust::error::Error;
use ismp_rust::host::ISMPHost;
use ismp_rust::messaging::Proof;
use std::time::Duration;
use sync_committee_primitives::derived_types::{LightClientState, LightClientUpdate};

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

// TODO:  Unbonding period for ethereum
const UNBONDING_PERIOD: u64 = 14;
// number of seconds in a day
const DAY: u64 = 24 * 60 * 60;
const EXECUTION_PAYLOAD_STATE_ID: u64 = 1;

impl ConsensusClient for ConsensusState {
    fn verify(
        &self,
        host: &dyn ISMPHost,
        trusted_consensus_state: Vec<u8>,
        proof: Vec<u8>,
    ) -> Result<(Vec<u8>, Vec<IntermediateState>), Error> {
        let beacon_message = BeaconMessage::decode(&mut &proof[..]).map_err(|_| {
            Error::ImplementationSpecific(format!("Cannot decode beacon message {:?}", proof))
        })?;

        let light_client_update = match beacon_message {
            BeaconMessage::ConsensusUpdate(update) => update.clone(),
            _ => return Err(Error::CannotHandleConsensusMessage),
        };

        let light_client_state = LightClientState::decode(&mut &trusted_consensus_state[..])
            .map_err(|_| {
                Error::ImplementationSpecific(format!(
                    "Cannot decode trusted consensus state {:?}",
                    trusted_consensus_state
                ))
            })?;

        let height = light_client_update.finalized_header.slot;
        // Ensure consensus client is not frozen
        let is_frozen = if let Some(frozen_height) = self.frozen_height {
            light_client_update.finalized_header.slot >= frozen_height
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
        if host.host_timestamp() - host.consensus_update_time(self.consensus_id())?
            >= self.unbonding_period()
        {
            return Err(Error::ImplementationSpecific(format!(
                "Unbonding period elapsed for host {:?} and consensus id {:?}",
                host.host(),
                self.consensus_id()
            )));
        }

        let no_codec_light_client_state = light_client_state.clone().try_into().map_err(|_| {
            Error::ImplementationSpecific(format!(
                "Cannot convert light client state {:?} to no codec type",
                light_client_state
            ))
        })?;
        let no_codec_light_client_update =
            light_client_update.clone().try_into().map_err(|_| {
                Error::ImplementationSpecific(format!(
                    "Cannot convert light client update {:?} to no codec type",
                    light_client_update
                ))
            })?;

        let _new_light_client_state = sync_committee_verifier::verify_sync_committee_attestation(
            no_codec_light_client_state,
            no_codec_light_client_update,
        )
        .map_err(|_| Error::ConsensusProofVerificationFailed {
            id: self.consensus_id(),
        })?;

        let mut intermediate_states = vec![];

        let commitment_root = light_client_update.execution_payload.state_root.clone();
        let intermediate_state = construct_intermediate_state(
            EXECUTION_PAYLOAD_STATE_ID,
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
        _host: &dyn ISMPHost,
        _key: Vec<u8>,
        _commitment: Vec<u8>,
        _proof: &Proof,
    ) -> Result<(), Error> {
        todo!()
    }

    fn verify_non_membership(
        &self,
        _host: &dyn ISMPHost,
        _key: Vec<u8>,
        _commitment: Vec<u8>,
        _proof: &Proof,
    ) -> Result<(), Error> {
        todo!()
    }

    fn is_frozen(&self, _host: &dyn ISMPHost, _id: ConsensusClientId) -> Result<bool, Error> {
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
