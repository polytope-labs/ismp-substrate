// Only enable this module for benchmarking.
#![cfg(feature = "runtime-benchmarks")]

use crate::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

// Details on using the benchmarks macro can be seen at:
//   https://paritytech.github.io/substrate/master/frame_benchmarking/trait.Benchmarking.html#tymethod.benchmarks
#[benchmarks(
    where
        <T as frame_system::Config>::Hash: From<H256>
)]
mod benchmarks {
    use super::*;
    use frame_system::EventRecord;
    use ismp_rs::{
        consensus::{ConsensusClient, IntermediateState, StateCommitment, StateMachineHeight},
        error::Error as IsmpError,
        messaging::Proof,
        router::RequestResponse,
    };

    fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
        let events = frame_system::Pallet::<T>::events();
        let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
        let EventRecord { event, .. } = &events[events.len() - 1];
        assert_eq!(event, &system_event);
    }

    #[derive(Default)]
    pub struct BenchmarkClient;

    pub const BENCHMARK_CONSENSUS_CLIENT_ID: [u8; 4] = [1u8; 4];

    impl ConsensusClient for BenchmarkClient {
        fn verify_consensus(
            &self,
            _host: &dyn ISMPHost,
            _trusted_consensus_state: Vec<u8>,
            _proof: Vec<u8>,
        ) -> Result<(Vec<u8>, Vec<IntermediateState>), IsmpError> {
            Ok(Default::default())
        }

        fn unbonding_period(&self) -> Duration {
            Duration::from_secs(60 * 60 * 60)
        }

        fn verify_membership(
            &self,
            _host: &dyn ISMPHost,
            _item: RequestResponse,
            _root: StateCommitment,
            _proof: &Proof,
        ) -> Result<(), IsmpError> {
            Ok(())
        }

        fn state_trie_key(&self, _request: RequestResponse) -> Vec<Vec<u8>> {
            Default::default()
        }

        fn verify_state_proof(
            &self,
            _host: &dyn ISMPHost,
            _keys: Vec<Vec<u8>>,
            _root: StateCommitment,
            _proof: &Proof,
        ) -> Result<Vec<Option<Vec<u8>>>, IsmpError> {
            Ok(Default::default())
        }

        fn is_frozen(&self, _trusted_consensus_state: &[u8]) -> Result<(), IsmpError> {
            Ok(())
        }
    }

    #[benchmark]
    fn create_consensus_client() {
        let intermediate_state = IntermediateState {
            height: StateMachineHeight {
                id: StateMachineId {
                    state_id: StateMachine::Polkadot(1000),
                    consensus_client: BENCHMARK_CONSENSUS_CLIENT_ID,
                },
                height: 1,
            },

            commitment: StateCommitment {
                timestamp: 1651280681,
                ismp_root: None,
                state_root: Default::default(),
            },
        };

        let message = CreateConsensusClient {
            consensus_state: Default::default(),
            consensus_client_id: BENCHMARK_CONSENSUS_CLIENT_ID,
            state_machine_commitments: vec![intermediate_state],
        };

        #[extrinsic_call]
        pallet::<T>::create_consensus_client(RawOrigin::Root, message);

        assert_last_event::<T>(
            Event::ConsensusClientCreated { consensus_client_id: BENCHMARK_CONSENSUS_CLIENT_ID }
                .into(),
        );
    }

    // The Benchmark mock client should be added to the runtime for these benchmarks to work
    #[benchmark]
    fn handle_request_message() {}

    #[benchmark]
    fn handle_response_message() {}

    #[benchmark]
    fn handle_timeout_message() {}

    impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
