// Copyright (C) 2023 Polytope Labs.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! ISMP Parachain Consensus Client
//!
//! This allows parachains communicate over ISMP leveraging the relay chain as a consensus oracle.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::ToString;
use frame_support::{traits::fungible::Mutate, PalletId};
use ismp::{
    module::ISMPModule,
    router::{Request, Response},
};
pub use pallet::*;

pub const PALLET_ID: PalletId = PalletId(*b"ismp-ast");

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        traits::{
            fungible::{Inspect, Mutate},
            tokens::Balance,
        },
    };
    use frame_system::pallet_prelude::*;
    use ismp::{
        host::StateMachine,
        router::{ISMPRouter, Post, Request},
    };
    use pallet_ismp::primitives::NonceProvider;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_ismp::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Balance: Balance + Into<<Self::NativeCurrency as Inspect<Self::AccountId>>::Balance>;
        type NativeCurrency: Mutate<Self::AccountId>;
        type Router: ISMPRouter + Default;
        type NonceProvider: NonceProvider;
        #[pallet::constant]
        type ChainID: Get<StateMachine>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        BalanceTransferred { from: T::AccountId, to: T::AccountId, amount: T::Balance },

        BalanceReceived { from: T::AccountId, to: T::AccountId, amount: T::Balance },
    }

    #[pallet::error]
    pub enum Error<T> {
        TransferFailed,
    }

    // Pallet implements [`Hooks`] trait to define some logic to execute in some context.
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(1_000_000)]
        #[pallet::call_index(0)]
        pub fn transfer(
            origin: OriginFor<T>,
            params: TransferParams<T::AccountId, T::Balance>,
        ) -> DispatchResult {
            let origin = ensure_signed(origin)?;
            let payload = Payload { to: params.to, from: origin.clone(), amount: params.amount };
            let request = Post {
                source_chain: T::ChainID::get(),
                dest_chain: params.dest_chain,
                nonce: T::NonceProvider::next_nonce(),
                from: PALLET_ID.0.to_vec(),
                to: PALLET_ID.0.to_vec(),
                timeout_timestamp: params.timeout,
                data: payload.encode(),
            };

            let router = T::Router::default();
            router.dispatch(Request::Post(request)).map_err(|_| Error::<T>::TransferFailed)?;
            <T::NativeCurrency as Mutate<T::AccountId>>::burn_from(&origin, params.amount.into())?;
            Self::deposit_event(Event::<T>::BalanceTransferred {
                from: payload.from,
                to: payload.to,
                amount: payload.amount,
            });
            Ok(())
        }
    }

    #[derive(
        Clone, codec::Encode, codec::Decode, scale_info::TypeInfo, PartialEq, Eq, RuntimeDebug,
    )]
    pub struct Payload<AccountId, Balance> {
        pub to: AccountId,
        pub from: AccountId,
        pub amount: Balance,
    }

    #[derive(
        Clone, codec::Encode, codec::Decode, scale_info::TypeInfo, PartialEq, Eq, RuntimeDebug,
    )]
    pub struct TransferParams<AccountId, Balance> {
        pub to: AccountId,
        pub amount: Balance,
        pub dest_chain: StateMachine,
        /// Timeout timestamp in seconds
        pub timeout: u64,
    }
}

impl<T: Config> ISMPModule for Pallet<T> {
    fn on_accept(request: Request) -> Result<(), ismp::error::Error> {
        let data = match request {
            Request::Post(post) => post.data,
            _ => Err(ismp::error::Error::ImplementationSpecific(
                "Only Post requests allowed, found Get".to_string(),
            ))?,
        };
        let payload = <Payload<T::AccountId, T::Balance> as codec::Decode>::decode(&mut &*data)
            .map_err(|_| {
                ismp::error::Error::ImplementationSpecific(
                    "Failed to decode request data".to_string(),
                )
            })?;
        <T::NativeCurrency as Mutate<T::AccountId>>::mint_into(&payload.to, payload.amount.into())
            .map_err(|_| {
                ismp::error::Error::ImplementationSpecific("Failed to mint funds".to_string())
            })?;
        Pallet::<T>::deposit_event(Event::<T>::BalanceReceived {
            from: payload.from,
            to: payload.to,
            amount: payload.amount,
        });
        Ok(())
    }

    fn on_response(_response: Response) -> Result<(), ismp::error::Error> {
        Err(ismp::error::Error::ImplementationSpecific(
            "Balance transfer protocol does not use accept responses".to_string(),
        ))
    }

    fn on_timeout(request: Request) -> Result<(), ismp::error::Error> {
        let data = match request {
            Request::Post(post) => post.data,
            _ => Err(ismp::error::Error::ImplementationSpecific(
                "Only Post requests allowed, found Get".to_string(),
            ))?,
        };
        let payload = <Payload<T::AccountId, T::Balance> as codec::Decode>::decode(&mut &*data)
            .map_err(|_| {
                ismp::error::Error::ImplementationSpecific(
                    "Failed to decode request data".to_string(),
                )
            })?;
        <T::NativeCurrency as Mutate<T::AccountId>>::mint_into(
            &payload.from,
            payload.amount.into(),
        )
        .map_err(|_| {
            ismp::error::Error::ImplementationSpecific("Failed to mint funds".to_string())
        })?;
        Pallet::<T>::deposit_event(Event::<T>::BalanceReceived {
            from: payload.from,
            to: payload.to,
            amount: payload.amount,
        });
        Ok(())
    }
}
