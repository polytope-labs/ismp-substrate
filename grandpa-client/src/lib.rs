
// scale encode the struct and enum definition
// Define the message, make it an enum with 2 variants, first variant for standalone chain(finality_proof(extract the state root and ismp root
// and store as the latest commitments(same as that of the parachain consensus)), second
// variant for relay chain(finality proof, and parachain headers(extract the state root and ismp from the parachain headers) and also contains the consensus client id, state machine id
// for standalone chain, we need the state root for the chain,
// will use grandpa client to monitor parachains, vice versa
// grandpa client should extract state for both standalone and parachains
// parachain_header will be an option when defining the header struct, height is not needed

pub mod client_message;

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
use primitives::{FinalityProof, ParachainHeaderProofs};
use crate::client_message::{ClientMessage};

pub struct GrandpaConsensusClient<T, R>(PhantomData<(T, R)>);

impl<T, R> Default for ParachainConsensusClient<T, R> {
    fn default() -> Self {
        Self(PhantomData)
    }
}


/// Interface that exposes the grandpa state roots.
pub trait RelayChainOracle {
    /// Returns the state root for a given height if it exists.
    fn state_root(height: relay_chain::BlockNumber) -> Option<relay_chain::Hash>;
}

impl<T: Config> RelayChainOracle for Pallet<T> {
    fn state_root(height: relay_chain::BlockNumber) -> Option<relay_chain::Hash> {
        RelayChainState::<T>::get(height)
    }
}

impl<T, R> ConsensusClient for GrandpaConsensusClient<T, R>
    where
        R: RelayChainOracle,
        T: pallet_ismp::Config + super::Config,
        T::BlockNumber: Into<u32>,
        T::Hash: From<H256>,
{
    fn verify_consensus(&self, host: &dyn IsmpHost, trusted_consensus_state: Vec<u8>, proof: Vec<u8>) -> Result<(Vec<u8>, BTreeMap<StateMachine, StateCommitmentHeight>), Error> {
        let update: FinalityProof<T> =
            codec::Decode::decode(&mut &proof[..]).map_err(|e| {
                Error::ImplementationSpecific(format!(
                    "Cannot decode finality consensus proof: {e:?}"
                ))
            })?;

        // first check our oracle's registry
        let root = R::state_root(update.relay_height)
            // not in our registry? ask parachain_system.
            .or_else(|| {
                let state = RelaychainDataProvider::<T>::current_relay_chain_state();

                if state.number == update.relay_height {
                    Some(state.state_root)
                } else {
                    None
                }
            })
            // well, we couldn't find it
            .ok_or_else(|| {
                Error::ImplementationSpecific(format!(
                    "Cannot find relay chain height: {}",
                    update.relay_height
                ))
            })?;
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
