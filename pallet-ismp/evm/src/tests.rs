use core::str::FromStr;
use std::collections::BTreeMap;
use crate::{handler::u64_to_u256, mocks::*};
use alloy_primitives::Address;
use alloy_sol_macro::sol;
use alloy_sol_types::{SolCall, SolType};
use fp_evm::{CreateInfo, FeeCalculator, GenesisAccount};
use frame_support::{traits::Get, weights::Weight};
use frame_support::traits::GenesisBuild;
use frame_system::EventRecord;
use hex_literal::hex;
use ismp_rs::host::StateMachine;
use pallet_evm::{runner::Runner, FixedGasWeightMapping, GasWeightMapping};
use pallet_ismp::Event;
use sp_core::{
    offchain::{testing::TestOffchainExt, OffchainDbExt, OffchainWorkerExt},
    H160, U256,
};

sol! {
    function transfer(
        address to,
        bytes memory dest,
        uint256 amount,
        uint256 timeout,
        uint64 gasLimit
    ) public;

    function dispatchGet(
        bytes memory dest,
        bytes[] memory keys,
        uint256 height,
        uint256 timeout,
        uint64 gasLimit
    ) public;

    function mintTo(address who, uint256 amount) public;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    let mut accounts = BTreeMap::new();
    accounts.insert(
        H160::from_low_u64_be(10),
        GenesisAccount {
            nonce: U256::from(1),
            balance: U256::from(1000000000000u64),
            storage: Default::default(),
            code: vec![
                0x00, // STOP
            ],
        },
    );
    accounts.insert(
        H160::default(), // root
        GenesisAccount {
            nonce: U256::from(1),
            balance: U256::max_value(),
            storage: Default::default(),
            code: vec![],
        },
    );

    GenesisBuild::<Test>::assimilate_storage(&pallet_evm::GenesisConfig { accounts }, &mut t).unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    register_offchain_ext(&mut ext);
    ext
}

fn register_offchain_ext(ext: &mut sp_io::TestExternalities) {
    let (offchain, _offchain_state) = TestOffchainExt::with_offchain_db(ext.offchain_db());
    ext.register_extension(OffchainDbExt::new(offchain.clone()));
    ext.register_extension(OffchainWorkerExt::new(offchain));
}

pub const EXAMPLE_CONTRACT: &str = include_str!("../solidity/IsmpDemo.bin");

const USER: Address = Address::new(hex!("d8da6bf26964af9d7eed9e03e53415d37aa96045"));

/// Verify the the last event emitted
fn assert_last_event<T: pallet_ismp::Config>(
    generic_event: <T as pallet_ismp::Config>::RuntimeEvent,
) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

fn deploy_contract(gas_limit: u64, weight_limit: Option<Weight>) -> CreateInfo {
    let info = <Test as pallet_evm::Config>::Runner::create(
        H160::zero(),
        hex::decode(EXAMPLE_CONTRACT.trim_end()).unwrap(),
        U256::zero(),
        gas_limit,
        Some(FixedGasPrice::min_gas_price().0),
        Some(FixedGasPrice::min_gas_price().0),
        None,
        Vec::new(),
        true, // non-transactional
        true, // must be validated
        weight_limit,
        None,
        &<Test as pallet_evm::Config>::config().clone(),
    )
    .expect("Deploy succeeds");

    let call_data = mintToCall { who: USER, amount: u64_to_u256(1_000_000_000).unwrap() }.encode();

    let contract_address = info.value;

    <Test as pallet_evm::Config>::Runner::call(
        H160::zero(),
        contract_address,
        call_data,
        U256::zero(),
        gas_limit,
        Some(FixedGasPrice::min_gas_price().0),
        Some(FixedGasPrice::min_gas_price().0),
        None,
        Vec::new(),
        true, // transactional
        true, // must be validated
        weight_limit,
        None,
        &<Test as pallet_evm::Config>::config().clone(),
    )
    .expect("call succeeds");

    info
}

#[test]
fn post_dispatch() {
    new_test_ext().execute_with(|| {
        let gas_limit: u64 = 1_000_000;
        let weight_limit = FixedGasWeightMapping::<Test>::gas_to_weight(gas_limit, true);
        let result = deploy_contract(gas_limit, Some(weight_limit));

        let contract_address = result.value;

        let call_data = transferCall {
            to: USER,
            dest: StateMachine::Polkadot(1000).to_string().as_bytes().to_vec(),
            amount: u64_to_u256(10_000).unwrap(),
            timeout: u64_to_u256(0).unwrap(),
            gasLimit: gas_limit,
        }
        .encode();

        <Test as pallet_evm::Config>::Runner::call(
            H160::zero(),
            contract_address,
            call_data,
            U256::zero(),
            gas_limit,
            Some(FixedGasPrice::min_gas_price().0),
            Some(FixedGasPrice::min_gas_price().0),
            None,
            Vec::new(),
            true, // transactional
            true, // must be validated
            Some(weight_limit),
            None,
            &<Test as pallet_evm::Config>::config().clone(),
        )
        .expect("call succeeds");

        // Check
        assert_last_event::<Test>(
            Event::Request {
                dest_chain: StateMachine::Polkadot(1000),
                source_chain: <Test as pallet_ismp::Config>::StateMachine::get(),
                request_nonce: 0,
            }
            .into(),
        );
    });
}
