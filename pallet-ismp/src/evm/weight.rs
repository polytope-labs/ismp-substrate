//! Weight info utilities for evm contracts
use crate::{
    evm::abi::ContractData as SolContractData, pallet::GasLimits, weight_info::IsmpModuleWeight,
    Config,
};
use alloy_sol_types::{SolCall, SolType};
use core::marker::PhantomData;
use frame_support::dispatch::Weight;
use ismp_rs::router::{Post, Request, Response};
use pallet_evm::GasWeightMapping;
use sp_runtime::traits::Bounded;

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
            let gas_limit = contract_data.gasLimit;
            <T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(gas_limit, true)
        } else {
            Weight::max_value()
        }
    }

    fn on_timeout(&self, request: &Request) -> Weight {
        GasLimits::<T>::get(request.nonce())
            .map(|limit| <T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(limit, true))
            .unwrap_or(Weight::max_value())
    }

    fn on_response(&self, response: &Response) -> Weight {
        GasLimits::<T>::get(response.nonce())
            .map(|limit| <T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(limit, true))
            .unwrap_or(Weight::max_value())
    }
}
