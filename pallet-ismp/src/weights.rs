#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_ismp.
pub trait WeightInfo {
    fn create_consensus_client() -> Weight;
    fn handle() -> Weight;
}

/// Weights for pallet_ismp.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Storage: Timestamp Now (r:1 w:0)
    /// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
    /// Storage: Ismp LatestStateMachineHeight (r:0 w:1)
    /// Proof Skipped: Ismp LatestStateMachineHeight (max_values: None, max_size: None, mode: Measured)
    /// Storage: Ismp ConsensusStates (r:0 w:1)
    /// Proof Skipped: Ismp ConsensusStates (max_values: None, max_size: None, mode: Measured)
    /// Storage: Ismp ConsensusClientUpdateTime (r:0 w:1)
    /// Proof Skipped: Ismp ConsensusClientUpdateTime (max_values: None, max_size: None, mode: Measured)
    /// Storage: Ismp StateCommitments (r:0 w:1)
    /// Proof Skipped: Ismp StateCommitments (max_values: None, max_size: None, mode: Measured)
    fn create_consensus_client() -> Weight {
        // Proof Size summary in bytes:
        //  Measured:  `6`
        //  Estimated: `1517`
        // Minimum execution time: 18_000_000 picoseconds.
        Weight::from_parts(19_000_000, 0)
            .saturating_add(Weight::from_parts(0, 1517))
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(4))
    }

    /// Storage: Ismp ConsensusStates (r:1 w:0)
    /// Proof Skipped: Ismp ConsensusStates (max_values: None, max_size: None, mode: Measured)
    /// Storage: Ismp FrozenHeights (r:1 w:0)
    /// Proof Skipped: Ismp FrozenHeights (max_values: None, max_size: None, mode: Measured)
    /// Storage: Ismp ConsensusClientUpdateTime (r:1 w:0)
    /// Proof Skipped: Ismp ConsensusClientUpdateTime (max_values: None, max_size: None, mode: Measured)
    /// Storage: Timestamp Now (r:1 w:0)
    /// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
    fn handle() -> Weight {
        // Proof Size summary in bytes:
        //  Measured:  `159`
        //  Estimated: `12365`
        // Minimum execution time: 40_000_000 picoseconds.
        Weight::from_parts(42_000_000, 0)
            .saturating_add(Weight::from_parts(0, 12365))
            .saturating_add(T::DbWeight::get().reads(4))
    }
}

// For backwards compatibility and tests
impl WeightInfo for () {
    /// Storage: Timestamp Now (r:1 w:0)
    /// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
    /// Storage: Ismp LatestStateMachineHeight (r:0 w:1)
    /// Proof Skipped: Ismp LatestStateMachineHeight (max_values: None, max_size: None, mode: Measured)
    /// Storage: Ismp ConsensusStates (r:0 w:1)
    /// Proof Skipped: Ismp ConsensusStates (max_values: None, max_size: None, mode: Measured)
    /// Storage: Ismp ConsensusClientUpdateTime (r:0 w:1)
    /// Proof Skipped: Ismp ConsensusClientUpdateTime (max_values: None, max_size: None, mode: Measured)
    /// Storage: Ismp StateCommitments (r:0 w:1)
    /// Proof Skipped: Ismp StateCommitments (max_values: None, max_size: None, mode: Measured)
    fn create_consensus_client() -> Weight {
        // Proof Size summary in bytes:
        //  Measured:  `6`
        //  Estimated: `1517`
        // Minimum execution time: 18_000_000 picoseconds.
        Weight::from_parts(19_000_000, 0)
            .saturating_add(Weight::from_parts(0, 1517))
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    /// Storage: Ismp ConsensusStates (r:1 w:0)
    /// Proof Skipped: Ismp ConsensusStates (max_values: None, max_size: None, mode: Measured)
    /// Storage: Ismp FrozenHeights (r:1 w:0)
    /// Proof Skipped: Ismp FrozenHeights (max_values: None, max_size: None, mode: Measured)
    /// Storage: Ismp ConsensusClientUpdateTime (r:1 w:0)
    /// Proof Skipped: Ismp ConsensusClientUpdateTime (max_values: None, max_size: None, mode: Measured)
    /// Storage: Timestamp Now (r:1 w:0)
    /// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
    fn handle() -> Weight {
        // Proof Size summary in bytes:
        //  Measured:  `159`
        //  Estimated: `12365`
        // Minimum execution time: 40_000_000 picoseconds.
        Weight::from_parts(42_000_000, 0)
            .saturating_add(Weight::from_parts(0, 12365))
            .saturating_add(T::DbWeight::get().reads(4))
    }
}
