use crate::{Config, Event as PalletEvent};
use alloc::collections::BTreeSet;
use ismp_rs::{
    consensus_client::{ConsensusClientId, StateMachineHeight, StateMachineId},
    host::StateMachine,
};

#[derive(codec::Encode, codec::Decode)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub enum Event {
    /// Event to be emitted when the challenge period for a state machine update has elapsed
    StateMachineUpdated {
        state_machine_id: StateMachineId,
        latest_height: u64,
        previous_height: u64,
    },
    ChallengePeriodStarted {
        consensus_client_id: ConsensusClientId,
        /// Tuple of previous height and latest height
        state_machines: BTreeSet<(StateMachineHeight, StateMachineHeight)>,
    },

    Response {
        /// Chain that this response will be routed to
        dest_chain: StateMachine,
        /// Source Chain for this response
        source_chain: StateMachine,
        /// Nonce for the request which this response is for
        request_nonce: u64,
    },
    Request {
        /// Chain that this request will be routed to
        dest_chain: StateMachine,
        /// Source Chain for request
        source_chain: StateMachine,
        /// Request nonce
        request_nonce: u64,
    },
}

pub fn to_core_protocol_events<T: Config>(event: PalletEvent<T>) -> Option<Event> {
    match event {
        PalletEvent::StateMachineUpdated { state_machine_id, latest_height, previous_height } => {
            Some(Event::StateMachineUpdated { state_machine_id, latest_height, previous_height })
        }
        PalletEvent::Response { dest_chain, source_chain, request_nonce } => {
            Some(Event::Response { dest_chain, source_chain, request_nonce })
        }
        PalletEvent::Request { dest_chain, source_chain, request_nonce } => {
            Some(Event::Request { dest_chain, source_chain, request_nonce })
        }
        _ => None,
    }
}