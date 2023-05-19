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
    use crate::router::Receipt;
    use frame_support::PalletId;
    use frame_system::EventRecord;
    use ismp_rs::{
        consensus::{ConsensusClient, IntermediateState, StateCommitment, StateMachineHeight},
        error::Error as IsmpError,
        messaging::{Message, Proof, RequestMessage, ResponseMessage},
        module::ISMPModule,
        router::{Post, RequestResponse},
        util::hash_request,
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

    /// This module should be added to the module router in runtime-benchmarks
    pub struct BenchmarkIsmpModule;
    pub const MODULE_ID: PalletId = PalletId(*b"benchmar");
    impl ISMPModule for BenchmarkIsmpModule {
        fn on_accept(_request: Request) -> Result<(), IsmpError> {
            Ok(())
        }

        fn on_response(_response: Response) -> Result<(), IsmpError> {
            Ok(())
        }

        fn on_timeout(_request: Request) -> Result<(), IsmpError> {
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

    fn setup_mock_client<H: ISMPHost>(host: &H) -> IntermediateState {
        let intermediate_state = IntermediateState {
            height: StateMachineHeight {
                id: StateMachineId {
                    state_id: StateMachine::Ethereum,
                    consensus_client: BENCHMARK_CONSENSUS_CLIENT_ID,
                },
                height: 1,
            },
            commitment: StateCommitment {
                timestamp: 1000,
                ismp_root: None,
                state_root: Default::default(),
            },
        };

        host.store_consensus_state(BENCHMARK_CONSENSUS_CLIENT_ID, vec![]).unwrap();
        host.store_state_machine_commitment(
            intermediate_state.height,
            intermediate_state.commitment,
        )
        .unwrap();

        intermediate_state
    }

    // The Benchmark consensus client should be added to the runtime for these benchmarks to work
    #[benchmark]
    fn handle_request_message() {
        let host = Host::<T>::default();
        let intermediate_state = setup_mock_client(&host);
        let post = Post {
            source_chain: StateMachine::Ethereum,
            dest_chain: <T as Config>::StateMachine::get(),
            nonce: 0,
            from: MODULE_ID.0.to_vec(),
            to: MODULE_ID.0.to_vec(),
            timeout_timestamp: 5000,
            data: vec![],
        };

        let msg = RequestMessage {
            requests: vec![Request::Post(post)],
            proof: Proof { height: intermediate_state.height, proof: vec![] },
        };
        let caller = whitelisted_caller();

        #[extrinsic_call]
        pallet::<T>::handle(RawOrigin::Signed(caller), vec![Message::Request(msg)]);
    }

    #[benchmark]
    fn handle_response_message() {
        let host = Host::<T>::default();
        let intermediate_state = setup_mock_client(&host);
        let post = Post {
            source_chain: <T as Config>::StateMachine::get(),
            dest_chain: StateMachine::Ethereum,
            nonce: 0,
            from: MODULE_ID.0.to_vec(),
            to: MODULE_ID.0.to_vec(),
            timeout_timestamp: 5000,
            data: vec![],
        };
        let request = Request::Post(post.clone());

        let commitment = hash_request::<Host<T>>(&request);
        RequestAcks::<T>::insert(commitment.0.to_vec(), Receipt::Ok);

        let response = Response::Post { post, response: vec![] };

        let msg = ResponseMessage::Post {
            responses: vec![response],
            proof: Proof { height: intermediate_state.height, proof: vec![] },
        };

        let caller = whitelisted_caller();

        #[extrinsic_call]
        pallet::<T>::handle(RawOrigin::Signed(caller), vec![Message::Response(msg)]);
    }

    // #[benchmark]
    // fn handle_timeout_message() {}

    impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
