
// scale encode the struct and enum definition
// Define the message, make it an enum with 2 variants, first variant for standalone chain(finality_proof(extract the state root and ismp root
// and store as the latest commitments(same as that of the parachain consensus)), second
// variant for relay chain(finality proof, and parachain headers(extract the state root and ismp from the parachain headers) and also contains the consensus client id, state machine id
// for standalone chain, we need the state root for the chain,
// will use grandpa client to monitor parachains, vice versa
// grandpa client should extract state for both standalone and parachains
// parachain_header will be an option when defining the header struct, height is not needed

use core::marker::PhantomData;
use std::collections::BTreeMap;
use std::time::Duration;
use sp_core::H256;
use ismp::{
    consensus::{ConsensusClient, ConsensusClientId, StateCommitment, StateMachineClient},
    error::Error,
    host::{IsmpHost, StateMachine},
    messaging::{Proof, StateCommitmentHeight},
    router::{Request, RequestResponse},
    util::hash_request,
};
use primitives::{ConsensusState, FinalityProof, ParachainHeaderProofs, ParachainHeadersWithFinalityProof};
use verifier::{verify_grandpa_finality_proof, verify_parachain_headers_with_grandpa_finality_proof};
use crate::consensus_message::{ConsensusMessage};

pub const POLKADOT_CONSENSUS_STATE_ID: [u8; 8] = *b"polkadot";
pub const KUSAMA_CONSENSUS_STATE_ID: [u8; 8] = *b"__kusama";

// map of consensus state id(bytes) to b tree set of state machine
// expose an extrinsic to update the map, takes consensus state id and a vector of state machine
// map for a relay chain... consensus state id, b tree set of para ids
// map for standalone chain... consensus state to 1 state machine
// extrinsic(adding or removing) of para ids to a relay chain
// extrinsic of state machine to a consensus state

pub struct GrandpaConsensusClient<T>(PhantomData<(T)>);

impl<T> Default for ParachainConsensusClient<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T> ConsensusClient for GrandpaConsensusClient<T>
    where
        T::BlockNumber: Into<u32>,
        T::Hash: From<H256>,
{
    fn verify_consensus(&self, host: &dyn IsmpHost, trusted_consensus_state: Vec<u8>, proof: Vec<u8>) -> Result<(Vec<u8>, BTreeMap<StateMachine, StateCommitmentHeight>), Error> {
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

        // match over the message
         match consensus_message {
            ConsensusMessage::StandaloneChainMessage(standalone_chain_message) => {
                verify_grandpa_finality_proof(consensus_state.clone(), standalone_chain_message.finality_proof)?;
            }

            ConsensusMessage::RelayChainMessage(relay_chain_message) => {
                let headers_with_finality_proof = ParachainHeadersWithFinalityProof {
                    finality_proof: relay_chain_message.finality_proof,
                    parachain_headers: relay_chain_message.parachain_headers,
                };

                verify_parachain_headers_with_grandpa_finality_proof(consensus_state.clone(), headers_with_finality_proof)?;
            }
        };

        Ok(())

        // check if there's a state machine set for that consensus state id(PENDING)

        // decode the proof into consensus message
        // match over the message

        // for standalone, just verify finality proof
        // take the highest  unknown headers(with the highest block number)
        // extract the ismp root and state root from header

        // create a pallet to map consensus state id to state machine

        // for the relay chain, it's the same with the standalone but no extraction is to be done
    }

    fn verify_fraud_proof(&self, host: &dyn IsmpHost, trusted_consensus_state: Vec<u8>, proof_1: Vec<u8>, proof_2: Vec<u8>) -> Result<(), Error> {
        todo!()
    }

    fn unbonding_period(&self) -> Duration {
        todo!()
    }

    fn state_machine(&self, id: StateMachine) -> Result<Box<dyn StateMachineClient>, Error> {
        todo!()
    }
}
