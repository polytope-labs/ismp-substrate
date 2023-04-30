#![cfg(feature = "runtime-benchmarks")]

#[allow(unused)]
use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use ismp_rs::{
    consensus::{IntermediateState, StateCommitment, StateMachineHeight, StateMachineId},
    host::StateMachine,
    messaging::CreateConsensusClient,
};
use sp_core::H256;

#[benchmarks(
    where
    <T as frame_system::Config>::Hash: From<H256>
)]
mod benchmarks {
    #[allow(unused)]
    use super::*;

    #[benchmark]
    fn create_consensus_client() {
        let intermediate_state = IntermediateState {
            height: StateMachineHeight {
                id: StateMachineId { state_id: StateMachine::Ethereum, consensus_client: *b"PARA" },
                height: 1 as u64,
            },

            commitment: StateCommitment {
                timestamp: 12,
                ismp_root: None,
                state_root: sp_core::H256::repeat_byte(24),
            },
        };

        let message = CreateConsensusClient {
            consensus_state: vec![1, 2, 3],
            consensus_client_id: [1, 2, 3, 4],
            state_machine_commitments: vec![intermediate_state],
        };

        #[extrinsic_call]
        pallet::<T>::create_consensus_client(RawOrigin::Root, message);
    }

    impl_benchmark_test_suite!(pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
