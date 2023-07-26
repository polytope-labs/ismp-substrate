//! Module Handler for EVM contracts
use crate::abi::{
    ContractData as SolContractData, GetRequest as SolGetRequest, GetResponse as SolGetResponse,
    OnAcceptCall, OnGetResponseCall, OnGetTimeoutCall, OnPostResponseCall, OnPostTimeoutCall,
    PostRequest, PostResponse as SolPostResponse, StorageValue as SolStorageValue,
};
use alloy_primitives::U256;
use alloy_sol_types::{SolCall, SolType};
use core::marker::PhantomData;
use ismp_rs::{
    error::Error,
    module::IsmpModule,
    router::{Post, Request, Response},
};
use pallet_evm::GasWeightMapping;
use pallet_ismp::{primitives::ModuleId, GasLimits, WeightConsumed};
use sp_core::H160;

/// Handler host address
/// Contracts should only allow ismp module callbacks to be executed by this address
pub const EVM_HOST_ADDRESS: H160 = H160::zero();
/// EVM contract handler
pub struct EvmContractHandler<T: pallet_ismp::Config + pallet_evm::Config>(PhantomData<T>);

impl<T: pallet_ismp::Config + pallet_evm::Config> Default for EvmContractHandler<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: pallet_ismp::Config + pallet_evm::Config> IsmpModule for EvmContractHandler<T> {
    fn on_accept(&self, request: Post) -> Result<(), Error> {
        let target_contract = parse_contract_id(&request.to)?;
        let contract_data = SolContractData::decode(&request.data, true).map_err(|_| {
            Error::ImplementationSpecific(
                "Failed to decode request data to the standard format".to_string(),
            )
        })?;
        let gas_limit = contract_data.gasLimit;
        let post = PostRequest {
            source: request.source.to_string().as_bytes().to_vec(),
            dest: request.dest.to_string().as_bytes().to_vec(),
            nonce: u64_to_u256(request.nonce)?,
            timeoutTimestamp: u64_to_u256(request.timeout_timestamp)?,
            from: request.from,
            to: request.to,
            data: request.data,
        };
        let call_data = OnAcceptCall { request: post }.encode();
        execute_call::<T>(target_contract, call_data, gas_limit)
    }

    fn on_response(&self, response: Response) -> Result<(), Error> {
        let target_contract = parse_contract_id(&response.destination_module())?;

        let (call_data, gas_limit) = match response {
            Response::Post(response) => {
                // we set the gas limit for executing the contract to the same as used in the
                // request we assume the request was dispatched with a gas limit
                // that accounts for execution of the response on this source chain
                let contract_data =
                    SolContractData::decode(&response.post.data, true).map_err(|_| {
                        Error::ImplementationSpecific(
                            "Failed to decode request data to the standard format".to_string(),
                        )
                    })?;
                let gas_limit = contract_data.gasLimit;
                let post_response = SolPostResponse {
                    request: PostRequest {
                        source: response.post.source.to_string().as_bytes().to_vec(),
                        dest: response.post.dest.to_string().as_bytes().to_vec(),
                        nonce: u64_to_u256(response.post.nonce)?,
                        timeoutTimestamp: u64_to_u256(response.post.timeout_timestamp)?,
                        from: response.post.from,
                        to: response.post.to,
                        data: response.post.data,
                    },
                    response: response.response,
                };
                (OnPostResponseCall { response: post_response }.encode(), gas_limit)
            }
            Response::Get(response) => {
                let gas_limit = GasLimits::<T>::get(response.get.nonce)
                    .ok_or(Error::ImplementationSpecific("Gas limit not found".to_string()))?;
                GasLimits::<T>::remove(response.get.nonce);
                let get_response = SolGetResponse {
                    request: SolGetRequest {
                        source: response.get.source.to_string().as_bytes().to_vec(),
                        dest: response.get.dest.to_string().as_bytes().to_vec(),
                        nonce: u64_to_u256(response.get.nonce)?,
                        height: u64_to_u256(response.get.height)?,
                        timeoutTimestamp: u64_to_u256(response.get.timeout_timestamp)?,
                        from: response.get.from,
                        keys: response.get.keys,
                    },
                    values: response
                        .values
                        .into_iter()
                        .map(|(key, value)| SolStorageValue {
                            key,
                            value: value.unwrap_or_default(),
                        })
                        .collect(),
                };
                (OnGetResponseCall { response: get_response }.encode(), gas_limit)
            }
        };

        execute_call::<T>(target_contract, call_data, gas_limit)
    }

    fn on_timeout(&self, request: Request) -> Result<(), Error> {
        let target_contract = parse_contract_id(&request.source_module())?;
        let (call_data, gas_limit) = match request {
            Request::Post(post) => {
                let contract_data = SolContractData::decode(&post.data, true).map_err(|_| {
                    Error::ImplementationSpecific(
                        "Failed to decode request data to the standard format".to_string(),
                    )
                })?;
                let gas_limit = contract_data.gasLimit;
                let request = PostRequest {
                    source: post.source.to_string().as_bytes().to_vec(),
                    dest: post.dest.to_string().as_bytes().to_vec(),
                    nonce: u64_to_u256(post.nonce)?,
                    timeoutTimestamp: u64_to_u256(post.timeout_timestamp)?,
                    from: post.from,
                    to: post.to,
                    data: post.data,
                };
                (OnPostTimeoutCall { request }.encode(), gas_limit)
            }
            Request::Get(get) => {
                let gas_limit = GasLimits::<T>::get(get.nonce)
                    .ok_or(Error::ImplementationSpecific("Gas limit not found".to_string()))?;
                GasLimits::<T>::remove(get.nonce);
                let request = SolGetRequest {
                    source: get.source.to_string().as_bytes().to_vec(),
                    dest: get.dest.to_string().as_bytes().to_vec(),
                    nonce: u64_to_u256(get.nonce)?,
                    height: u64_to_u256(get.height)?,
                    timeoutTimestamp: u64_to_u256(get.timeout_timestamp)?,
                    from: get.from,
                    keys: get.keys,
                };
                (OnGetTimeoutCall { request }.encode(), gas_limit)
            }
        };
        execute_call::<T>(target_contract, call_data, gas_limit)
    }
}

/// Parse contract id from raw bytes
pub fn parse_contract_id(bytes: &[u8]) -> Result<H160, Error> {
    let module_id =
        ModuleId::from_bytes(bytes).map_err(|e| Error::ImplementationSpecific(e.to_string()))?;
    match module_id {
        ModuleId::Evm(id) => Ok(id),
        _ => Err(Error::ImplementationSpecific("Expected Evm contract id".to_string())),
    }
}

/// Convert u64 to U256
fn u64_to_u256(value: u64) -> Result<U256, Error> {
    U256::try_from(value)
        .map_err(|_| Error::ImplementationSpecific("Failed to convert u64 to u256".to_string()))
}

/// Call execute call data
fn execute_call<T: pallet_ismp::Config + pallet_evm::Config>(
    target: H160,
    call_data: Vec<u8>,
    gas_limit: u64,
) -> Result<(), Error> {
    match <<T as pallet_evm::Config>::Runner as pallet_evm::Runner<T>>::call(
        EVM_HOST_ADDRESS,
        target,
        call_data,
        Default::default(),
        gas_limit,
        None,
        None,
        None,
        Default::default(),
        true,
        true,
        None,
        None,
        <T as pallet_evm::Config>::config(),
    ) {
        Ok(info) => {
            let mut total_weight_used = WeightConsumed::<T>::get();
            let weight_limit = T::GasWeightMapping::gas_to_weight(gas_limit, true);
            let weight_used =
                T::GasWeightMapping::gas_to_weight(info.used_gas.standard.low_u64(), true);
            total_weight_used.weight_used = total_weight_used.weight_used + weight_used;
            total_weight_used.weight_limit = total_weight_used.weight_limit + weight_limit;
            WeightConsumed::<T>::put(total_weight_used);
            Ok(())
        }
        Err(error) => {
            let mut total_weight_used = WeightConsumed::<T>::get();
            let weight_limit = T::GasWeightMapping::gas_to_weight(gas_limit, true);
            total_weight_used.weight_used = total_weight_used.weight_used + error.weight;
            total_weight_used.weight_limit = total_weight_used.weight_limit + weight_limit;
            WeightConsumed::<T>::put(total_weight_used);
            // We still return ok so we can compensate for used gas only
            Err(Error::ImplementationSpecific(
                "Contract encountered error while executing".to_string(),
            ))
        }
    }
}
