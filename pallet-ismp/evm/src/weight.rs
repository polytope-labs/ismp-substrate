//! Weight info utilities for evm contracts
use crate::{abi::ContractData as SolContractData, precompiles::u256_to_u64};
use alloy_sol_types::SolType;
use core::marker::PhantomData;
use frame_support::{dispatch::Weight, traits::Get};
use ismp_rs::router::{Post, Request, Response};
use pallet_evm::GasWeightMapping;
use pallet_ismp::{weight_info::IsmpModuleWeight, Config, GasLimits};

/// An implementation of IsmpModuleWeight for evm contract callbacks
pub struct EvmWeightCalculator<T: Config + pallet_evm::Config>(PhantomData<T>);

impl<T: Config + pallet_evm::Config> Default for EvmWeightCalculator<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Config + pallet_evm::Config> IsmpModuleWeight for EvmWeightCalculator<T> {
    fn on_accept(&self, request: &Post) -> Weight {
        if let Ok(contract_data) = SolContractData::decode(&request.data, true) {
            let gas_limit = u256_to_u64(contract_data.gasLimit);
            <T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(gas_limit, true)
        } else {
            <T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
                <T as pallet_evm::Config>::BlockGasLimit::get().low_u64(),
                true,
            )
        }
    }

    fn on_timeout(&self, request: &Request) -> Weight {
        match request {
            Request::Post(post) => {
                if let Ok(contract_data) = SolContractData::decode(&post.data, true) {
                    let gas_limit = u256_to_u64(contract_data.gasLimit);
                    <T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(gas_limit, true)
                } else {
                    <T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
                        <T as pallet_evm::Config>::BlockGasLimit::get().low_u64(),
                        true,
                    )
                }
            }
            Request::Get(get) => GasLimits::<T>::get(get.nonce)
                .map(|limit| {
                    <T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(limit, true)
                })
                .unwrap_or(<T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
                    <T as pallet_evm::Config>::BlockGasLimit::get().low_u64(),
                    true,
                )),
        }
    }

    fn on_response(&self, response: &Response) -> Weight {
        match response {
            Response::Post(response) => {
                if let Ok(contract_data) = SolContractData::decode(&response.post.data, true) {
                    let gas_limit = u256_to_u64(contract_data.gasLimit);
                    <T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(gas_limit, true)
                } else {
                    <T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
                        <T as pallet_evm::Config>::BlockGasLimit::get().low_u64(),
                        true,
                    )
                }
            }
            Response::Get(response) => GasLimits::<T>::get(response.get.nonce)
                .map(|limit| {
                    <T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(limit, true)
                })
                .unwrap_or(<T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
                    <T as pallet_evm::Config>::BlockGasLimit::get().low_u64(),
                    true,
                )),
        }
    }
}
