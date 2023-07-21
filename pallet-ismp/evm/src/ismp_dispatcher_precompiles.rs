//! IsmpDispatcher precompiles for pallet-evm

use pallet_ismp::{dispatcher::Dispatcher, weight_info::WeightInfo, GasLimits, Pallet};

use crate::abi::{
    ContractData, DispatchGet as SolDispatchGet, DispatchPost as SolDispatchPost,
    PostResponse as SolPostResponse,
};
use alloc::{format, str::FromStr, string::String};
use alloy_sol_types::SolType;
use core::marker::PhantomData;
use fp_evm::{
    ExitError, ExitSucceed, Precompile, PrecompileFailure, PrecompileHandle, PrecompileOutput,
    PrecompileResult,
};
use frame_support::traits::Get;
use ismp_rs::{
    host::StateMachine,
    router::{DispatchGet, DispatchPost, DispatchRequest, IsmpDispatcher, Post, PostResponse},
};
use pallet_evm::GasWeightMapping;
use sp_core::{H256, U256};
use sp_std::prelude::*;

/// Ismp Request Dispatcher precompile for evm contracts
pub struct IsmpPostDispatcher<T> {
    _marker: PhantomData<T>,
}

impl<T> Precompile for IsmpPostDispatcher<T>
where
    T: pallet_ismp::Config + pallet_evm::Config,
    <T as frame_system::Config>::Hash: From<H256>,
{
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        let input = handle.input();
        let context = handle.context();
        let weight = <T as pallet_ismp::Config>::WeightInfo::dispatch_post_request();

        // The cost of a dispatch is the weight of calling the dispatcher plus an extra storage read
        // and write
        let cost = <T as pallet_evm::Config>::GasWeightMapping::weight_to_gas(weight);

        let dispatcher = Dispatcher::<T>::default();
        let post_dispatch =
            SolDispatchPost::decode(input, true).map_err(|e| PrecompileFailure::Error {
                exit_status: ExitError::Other(format!("Failed to decode input: {:?}", e).into()),
            })?;

        let post_dispatch = DispatchPost {
            dest: parse_state_machine(post_dispatch.dest)?,
            from: context.caller.0.to_vec(),
            to: post_dispatch.to,
            timeout_timestamp: u256_to_u64(post_dispatch.timeoutTimestamp),
            data: ContractData::encode(&post_dispatch.data),
        };

        handle.record_cost(cost)?;
        match dispatcher.dispatch_request(DispatchRequest::Post(post_dispatch)) {
            Ok(_) => Ok(PrecompileOutput { exit_status: ExitSucceed::Returned, output: vec![] }),
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(format!("dispatch execution failed: {:?}", e).into()),
            }),
        }
    }
}

/// Ismp Request Dispatcher precompile for evm contracts
pub struct IsmpGetDispatcher<T> {
    _marker: PhantomData<T>,
}

impl<T> Precompile for IsmpGetDispatcher<T>
where
    T: pallet_ismp::Config + pallet_evm::Config,
    <T as frame_system::Config>::Hash: From<H256>,
{
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        let input = handle.input();
        let context = handle.context();

        let weight = <T as pallet_ismp::Config>::WeightInfo::dispatch_get_request();

        // The cost of a dispatch is the weight of calling the dispatcher plus an extra storage read
        // and write
        let cost = <T as pallet_evm::Config>::GasWeightMapping::weight_to_gas(
            weight.saturating_add(<T as frame_system::Config>::DbWeight::get().reads_writes(1, 1)),
        );

        let dispatcher = Dispatcher::<T>::default();

        let get_dispatch = SolDispatchGet::decode(input, true).map_err(|e| {
            println!("\nError decoding DispatchGet Decode failure {:?}\n", e);
            PrecompileFailure::Error {
                exit_status: ExitError::Other(format!("Failed to decode input: {:?}", e).into()),
            }
        })?;
        let gas_limit = u256_to_u64(get_dispatch.gasLimit);
        let get_dispatch = DispatchGet {
            dest: parse_state_machine(get_dispatch.dest)?,
            from: context.caller.0.to_vec(),
            keys: get_dispatch.keys,
            height: u256_to_u64(get_dispatch.height),
            timeout_timestamp: u256_to_u64(get_dispatch.timeoutTimestamp),
        };

        handle.record_cost(cost)?;
        match dispatcher.dispatch_request(DispatchRequest::Get(get_dispatch)) {
            Ok(_) => {
                let nonce = Pallet::<T>::previous_nonce();
                GasLimits::<T>::insert(nonce, gas_limit);
                Ok(PrecompileOutput { exit_status: ExitSucceed::Returned, output: vec![] })
            }
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(format!("dispatch execution failed: {:?}", e).into()),
            }),
        }
    }
}

/// Ismp Response Dispatcher precompile for evm contracts
pub struct IsmpResponseDispatcher<T> {
    _marker: PhantomData<T>,
}

impl<T> Precompile for IsmpResponseDispatcher<T>
where
    T: pallet_ismp::Config + pallet_evm::Config,
    <T as frame_system::Config>::Hash: From<H256>,
{
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        let input = handle.input();

        let weight = <T as pallet_ismp::Config>::WeightInfo::dispatch_response();

        let cost = <T as pallet_evm::Config>::GasWeightMapping::weight_to_gas(weight);

        let dispatcher = Dispatcher::<T>::default();
        let post_response =
            SolPostResponse::decode(input, true).map_err(|e| PrecompileFailure::Error {
                exit_status: ExitError::Other(format!("Failed to decode input: {:?}", e).into()),
            })?;
        let post_response = PostResponse {
            post: Post {
                source: parse_state_machine(post_response.request.source)?,
                dest: parse_state_machine(post_response.request.dest)?,
                nonce: u256_to_u64(post_response.request.nonce),
                from: post_response.request.from,
                to: post_response.request.to,
                timeout_timestamp: u256_to_u64(post_response.request.timeoutTimestamp),
                data: ContractData::encode(&post_response.request.data),
            },
            response: post_response.response,
        };
        handle.record_cost(cost)?;

        match dispatcher.dispatch_response(post_response) {
            Ok(_) => Ok(PrecompileOutput { exit_status: ExitSucceed::Returned, output: vec![] }),
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(format!("dispatch execution failed: {:?}", e).into()),
            }),
        }
    }
}

/// Convert u256 to u64 without overflow check
pub fn u256_to_u64(value: alloy_primitives::U256) -> u64 {
    U256::from_big_endian(value.to_be_bytes::<32>().as_slice()).low_u64()
}

/// Parse state machine from utf8 bytes
fn parse_state_machine(bytes: Vec<u8>) -> Result<StateMachine, PrecompileFailure> {
    StateMachine::from_str(&String::from_utf8(bytes).unwrap_or_default()).map_err(|e| {
        PrecompileFailure::Error {
            exit_status: ExitError::Other(format!("Failed to destination chain: {:?}", e).into()),
        }
    })
}
