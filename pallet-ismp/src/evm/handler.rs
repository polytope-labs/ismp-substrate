//! Module Handler for EVM contracts
use crate::{
    evm::abi::{
        ContractData as SolContractData, GetRequest as SolGetRequest,
        GetResponse as SolGetResponse, OnAcceptCall, OnGetResponseCall, OnGetTimeoutCall,
        OnPostResponseCall, OnPostTimeoutCall, OptionValue, PostRequest,
        PostResponse as SolPostResponse, StorageValue as SolStorageValue,
    },
    primitives::ModuleId,
    Config, GasLimits,
};
use alloy_primitives::U256;
use alloy_sol_types::{SolCall, SolType};
use core::marker::PhantomData;
use ismp_rs::{
    contracts::Gas,
    error::Error,
    module::IsmpModule,
    router::{Post, Request, Response},
};
use pallet_evm::GasWeightMapping;
use sp_core::H160;

/// Handler host address
/// Contracts should only allow ismp module callbacks to be executed by this address
pub const EVM_HOST_ADDRESS: H160 = H160::zero();
/// EVM contract handler
pub struct EvmContractHandler<T: Config + pallet_evm::Config>(PhantomData<T>);

impl<T: Config + pallet_evm::Config> Default for EvmContractHandler<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Config + pallet_evm::Config> IsmpModule for EvmContractHandler<T> {
    fn on_accept(&self, request: Post) -> Result<Gas, Error> {
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

    fn on_response(&self, response: Response) -> Result<Gas, Error> {
        let target_contract = parse_contract_id(&response.destination_module())?;
        let gas_limit = GasLimits::<T>::get(response.nonce())
            .ok_or(Error::ImplementationSpecific("Gas limit not found".to_string()))?;
        GasLimits::<T>::remove(response.nonce());
        let call_data = match response {
            Response::Post(response) => {
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
                OnPostResponseCall { response: post_response }.encode()
            }
            Response::Get(response) => {
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
                            value: value
                                .map(|value| OptionValue { value, isSome: true })
                                .unwrap_or(OptionValue {
                                    value: Default::default(),
                                    isSome: false,
                                }),
                        })
                        .collect(),
                };
                OnGetResponseCall { response: get_response }.encode()
            }
        };

        execute_call::<T>(target_contract, call_data, gas_limit)
    }

    fn on_timeout(&self, request: Request) -> Result<Gas, Error> {
        let target_contract = parse_contract_id(&request.source_module())?;
        let gas_limit = GasLimits::<T>::get(request.nonce())
            .ok_or(Error::ImplementationSpecific("Gas limit not found".to_string()))?;
        GasLimits::<T>::remove(request.nonce());
        let call_data = match request {
            Request::Post(post) => {
                let request = PostRequest {
                    source: post.source.to_string().as_bytes().to_vec(),
                    dest: post.dest.to_string().as_bytes().to_vec(),
                    nonce: u64_to_u256(post.nonce)?,
                    timeoutTimestamp: u64_to_u256(post.timeout_timestamp)?,
                    from: post.from,
                    to: post.to,
                    data: post.data,
                };
                OnPostTimeoutCall { request }.encode()
            }
            Request::Get(get) => {
                let request = SolGetRequest {
                    source: get.source.to_string().as_bytes().to_vec(),
                    dest: get.dest.to_string().as_bytes().to_vec(),
                    nonce: u64_to_u256(get.nonce)?,
                    height: u64_to_u256(get.height)?,
                    timeoutTimestamp: u64_to_u256(get.timeout_timestamp)?,
                    from: get.from,
                    keys: get.keys,
                };
                OnGetTimeoutCall { request }.encode()
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
fn execute_call<T: pallet_evm::Config>(
    target: H160,
    call_data: Vec<u8>,
    gas_limit: u64,
) -> Result<Gas, Error> {
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
            Ok(Gas {
                // used gas would be at most equal to the gas limit as such we could convert
                // U256 to U64 without panic
                gas_used: Some(info.used_gas.standard.low_u64()),
                gas_limit: Some(gas_limit),
            })
        }
        Err(error) => {
            // We still return ok so we can compensate for used gas only
            Ok(Gas {
                gas_used: Some(<T as pallet_evm::Config>::GasWeightMapping::weight_to_gas(
                    error.weight,
                )),
                gas_limit: Some(gas_limit),
            })
        }
    }
}
