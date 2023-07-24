pub mod consensus_message;
pub mod consensus;

use alloc::{vec, vec::Vec};
pub use pallet::*;
use pallet_ismp::host::Host;

#[frame_support::pallet]
pub mod pallet {
    use codec::alloc::collections::BTreeSet;
    use super::*;
    use cumulus_primitives_core::{ParaId, relay_chain};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use ismp::host::{IsmpHost, StateMachine};
    use ismp::messaging::{ConsensusMessage, Message};
    use primitive_types::H256;
    use primitives::ConsensusState;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The config trait
    #[pallet::config]
    pub trait Config:
    frame_system::Config
    {
        /// The overarching event type
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    /// Mapping of standalone chain consensus state id to 1 state machine.
    #[pallet::storage]
    #[pallet::getter(fn relay_chain_state)]
    pub type StandaloneChainConsensusState<T: Config> =
    StorageMap<_, Blake2_128Concat, Vec<u8>, StateMachine>;

    /// Mapping of relay chain consensus state id to set of para ids.
    #[pallet::storage]
    #[pallet::getter(fn relay_chain_state)]
    pub type RelayChainConsensusState<T: Config> =
    StorageMap<_, Blake2_128Concat, Vec<u8>, BTreeSet<ParaId>>;

    /// Events emitted by this pallet
    #[pallet::event]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {
        /// Standalone Consensus State Already Exists
        StandaloneConsensusStateAlreadyExists,
        /// Standalone Consensus Does not Exist
        StandaloneConsensusStateDontExists,
        /// Error fetching consensus state
        ErrorFetchingConsensusState,
        /// Error decoding consensus state
        ErrorDecodingConsensusState,
        /// Incorrect consensus state id length
        IncorrectConsensusStateIdLength
    }

    #[pallet::call]
    impl<T: Config> Pallet<T>
        where
            <T as frame_system::Config>::Hash: From<H256>,
    {
        /// Add some new parachains to the list of parachains in the relay chain consensus state
        #[pallet::call_index(0)]
        #[pallet::weight(0)]
        pub fn add_parachains(origin: OriginFor<T>, consensus_state_id_vec: Vec<u8>, para_ids: Vec<u32>) -> DispatchResult {
            ensure_root(origin)?;

            let ismp_host = Host::<T>::default();
            let consensus_state_id = consensus_state_id_vec.as_slice().try_into().map_err(|_| Error::IncorrectConsensusStateIdLength)?;

            let encoded_consensus_state = ismp_host.consensus_state(consensus_state_id).map_err(|_| Error::ErrorFetchingConsensusState)?;
            let mut consensus_state: ConsensusState =
                codec::Decode::decode(&mut &encoded_consensus_state[..]).map_err(|_| Error::ErrorDecodingConsensusState)?;

            let mut stored_para_ids = consensus_state.latest_para_heights;
            para_ids.iter().for_each(|para_id| {
                stored_para_ids.entry(para_id).or_insert(true);
            });
            consensus_state.latest_para_heights = stored_para_ids;

            let encoded_consensus_state = consensus_state.encode();
            ismp_host.store_consensus_state(consensus_state_id, encoded_consensus_state)?;
            Ok(())
        }
    }
}
