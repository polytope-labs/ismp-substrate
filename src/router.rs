use crate::host::Host;
use crate::mmr::{self, Leaf, Mmr};
use crate::{Config, Event, Pallet, RequestCommitments, ResponseCommitments};
use alloc::string::ToString;
use core::marker::PhantomData;
use ismp_rust::error::Error;
use ismp_rust::host::ISMPHost;
use ismp_rust::paths::{RequestPath, ResponsePath};
use ismp_rust::router::{IISMPRouter, Request, Response};

#[derive(Clone)]
pub struct Router<T: Config>(PhantomData<T>);

impl<T: Config> Default for Router<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Config> IISMPRouter for Router<T> {
    fn dispatch(&self, request: Request) -> Result<(), Error> {
        let host = Host::<T>::default();
        let key = RequestPath {
            dest_chain: request.dest_chain,
            nonce: request.nonce,
        }
        .to_string()
        .as_bytes()
        .to_vec();
        let commitment = host.get_request_commitment(&request);

        if host.host() != request.dest_chain {
            let leaves = Pallet::<T>::number_of_leaves();
            let (dest_chain, nonce) = (request.dest_chain, request.nonce);
            let mut mmr: Mmr<mmr::storage::RuntimeStorage, T, Leaf> = mmr::Mmr::new(leaves);
            let offchain_key = Pallet::<T>::request_leaf_index_offchain_key(dest_chain, nonce);
            let leaf_index = mmr.push(Leaf::Request(request)).ok_or_else(|| {
                Error::ImplementationSpecific("Failed to push request into mmr".to_string())
            })?;
            // Deposit Event
            Pallet::<T>::deposit_event(Event::RequestReceived {
                request_nonce: nonce,
                dest_chain,
            });
            // Store a map of request to leaf_index
            Pallet::<T>::store_leaf_index_offchain(offchain_key, leaf_index)
        }

        RequestCommitments::<T>::insert(key, commitment);
        Ok(())
    }

    fn write_response(&self, response: Response) -> Result<(), Error> {
        let host = Host::<T>::default();
        let key = ResponsePath {
            dest_chain: response.request.source_chain,
            nonce: response.request.nonce,
        }
        .to_string()
        .as_bytes()
        .to_vec();

        let commitment = host.get_response_commitment(&response);
        if host.host() != response.request.source_chain {
            let leaves = Pallet::<T>::number_of_leaves();
            let (source_chain, nonce) = (response.request.source_chain, response.request.nonce);
            let mut mmr: Mmr<mmr::storage::RuntimeStorage, T, Leaf> = mmr::Mmr::new(leaves);
            let offchain_key = Pallet::<T>::response_leaf_index_offchain_key(source_chain, nonce);
            let leaf_index = mmr.push(Leaf::Response(response)).ok_or_else(|| {
                Error::ImplementationSpecific("Failed to push response into mmr".to_string())
            })?;
            Pallet::<T>::deposit_event(Event::ResponseReceived {
                request_nonce: nonce,
                dest_chain: source_chain,
            });
            Pallet::<T>::store_leaf_index_offchain(offchain_key, leaf_index)
        }

        ResponseCommitments::<T>::insert(key, commitment);

        Ok(())
    }
}
