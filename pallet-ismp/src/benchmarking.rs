#![cfg(feature = "runtime-benchmarks")]

#[allow(unused)]
use super::*;
use frame_benchmarking::v2::*;
use frame_system::{EventRecord, Pallet as System, RawOrigin};
use ismp_rs::{
    consensus::{IntermediateState, StateCommitment, StateMachineHeight, StateMachineId},
    host::StateMachine,
    messaging::{
        ConsensusMessage, CreateConsensusClient, Message, Proof, RequestMessage, ResponseMessage,
        TimeoutMessage,
    },
};
use sp_core::H256;

#[benchmarks(
    where
    <T as frame_system::Config>::Hash: From<H256>
)]
mod benchmarks {
    #[allow(unused)]
    use super::*;

    fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
        let events = System::<T>::events();
        let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
        let EventRecord { event, .. } = &events[events.len() - 1];
        assert_eq!(event, &system_event);
    }

    #[benchmark]
    fn create_consensus_client() {
        let consensus_client_id = [0u8; 4];
        let intermediate_state = IntermediateState {
            height: StateMachineHeight {
                id: StateMachineId { state_id: StateMachine::Ethereum, consensus_client: *b"PARA" },
                height: 1,
            },

            commitment: StateCommitment {
                timestamp: 1651280681,
                ismp_root: None,
                state_root: sp_core::H256::repeat_byte(24),
            },
        };

        let message = CreateConsensusClient {
            consensus_state: vec![1; 4],
            consensus_client_id,
            state_machine_commitments: vec![intermediate_state],
        };

        #[extrinsic_call]
        pallet::<T>::create_consensus_client(RawOrigin::Root, message);

        assert_last_event::<T>(Event::ConsensusClientCreated { consensus_client_id }.into());
    }

    #[benchmark]
    fn handle() {
        let caller: T::AccountId = whitelisted_caller();

        let state_id = StateMachine::Ethereum;
        let consensus_client = *b"PARA";
        let consensus_client_id = [0u8; 4];
        let consensus_proof = [1u8; 32].to_vec();

        let mut messages = Vec::new();

        let consensus_message =
            Message::Consensus(ConsensusMessage { consensus_client_id, consensus_proof });
        let request_message = Message::Request(RequestMessage {
            requests: vec![],
            proof: Proof {
                height: StateMachineHeight {
                    id: StateMachineId { state_id, consensus_client },
                    height: 0,
                },
                proof: vec![],
            },
        });
        let response_message = Message::Response(ResponseMessage {
            responses: vec![],
            proof: Proof {
                height: StateMachineHeight {
                    id: StateMachineId { state_id, consensus_client },
                    height: 0,
                },
                proof: vec![],
            },
        });
        let timeout_message = Message::Timeout(TimeoutMessage {
            requests: vec![],
            timeout_proof: Proof {
                height: StateMachineHeight {
                    id: StateMachineId { state_id, consensus_client },
                    height: 0,
                },
                proof: vec![],
            },
        });

        messages.push(consensus_message);
        messages.push(request_message);
        messages.push(response_message);
        messages.push(timeout_message);

        #[extrinsic_call]
        pallet::<T>::handle(RawOrigin::Signed(caller.clone()), messages);
    }

    impl_benchmark_test_suite!(pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
