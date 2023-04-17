#![cfg_attr(not(feature = "std"), no_std)]

pub mod consensus_client;

use cumulus_primitives_core::relay_chain;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use cumulus_primitives_core::relay_chain;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use parachain_system::{RelaychainDataProvider, RelaychainStateProvider};

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + parachain_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::storage]
    #[pallet::getter(fn state_commitments)]
    pub type RelayChainState<T: Config> =
        StorageMap<_, Blake2_128Concat, relay_chain::BlockNumber, relay_chain::Hash, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        NewRelayChainState { height: relay_chain::BlockNumber },
    }

    // Pallet implements [`Hooks`] trait to define some logic to execute in some context.
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_n: T::BlockNumber) -> Weight {
            let state = RelaychainDataProvider::<T>::current_relay_chain_state();
            if !RelayChainState::<T>::contains_key(state.number) {
                RelayChainState::<T>::insert(state.number, state.state_root);
            }
            Weight::zero()
        }
    }
}

/// Interface that exposes the relay chain state roots.
pub trait RelayChainOracle {
    /// Returns the state root for a given height if it exists.
    fn storage_root(height: relay_chain::BlockNumber) -> Option<relay_chain::Hash>;
}

impl<T: Config> RelayChainOracle for Pallet<T> {
    fn storage_root(height: relay_chain::BlockNumber) -> Option<relay_chain::Hash> {
        RelayChainState::get(height)
    }
}
