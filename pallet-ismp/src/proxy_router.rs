//! A proxy router implementation
//! Allows routing requests to other chains through the host

use crate::{
    dispatcher::Receipt, host::Host, Config, Event, IncomingRequestAcks, IncomingResponseAcks,
    Pallet,
};
use alloc::{boxed::Box, string::ToString};
use core::marker::PhantomData;
use ismp_primitives::mmr::Leaf;
use ismp_rs::{
    host::IsmpHost,
    router::{DispatchError, DispatchResult, DispatchSuccess, IsmpRouter, Request, Response},
    util::{hash_request, hash_response},
};
use sp_core::H256;

/// The proxy router, This router allows for routing requests & responses from a source chain
/// to a destination chain.
pub struct ProxyRouter<T> {
    inner: Option<Box<dyn IsmpRouter>>,
    _phantom: PhantomData<T>,
}

impl<T> ProxyRouter<T> {
    /// Initialize the proxy router with an inner router.
    pub fn new<R>(router: R) -> Self
    where
        R: IsmpRouter + 'static,
    {
        Self { inner: Some(Box::new(router)), _phantom: PhantomData }
    }
}

impl<T> Default for ProxyRouter<T> {
    fn default() -> Self {
        Self { inner: None, _phantom: PhantomData }
    }
}

impl<T> IsmpRouter for ProxyRouter<T>
where
    T: Config,
    <T as frame_system::Config>::Hash: From<H256>,
{
    fn handle_request(&self, request: Request) -> DispatchResult {
        let host = Host::<T>::default();

        if host.host_state_machine() != request.dest_chain() {
            let commitment = hash_request::<Host<T>>(&request).0.to_vec();

            if IncomingRequestAcks::<T>::contains_key(commitment.clone()) {
                Err(DispatchError {
                    msg: "Duplicate request".to_string(),
                    nonce: request.nonce(),
                    source: request.source_chain(),
                    dest: request.dest_chain(),
                })?
            }

            let (dest_chain, source_chain, nonce) =
                (request.dest_chain(), request.source_chain(), request.nonce());
            Pallet::<T>::mmr_push(Leaf::Request(request)).ok_or_else(|| DispatchError {
                msg: "Failed to push request into mmr".to_string(),
                nonce,
                source: source_chain,
                dest: dest_chain,
            })?;
            // Deposit Event
            Pallet::<T>::deposit_event(Event::Request {
                request_nonce: nonce,
                source_chain,
                dest_chain,
            });
            // We have this step because we can't delete leaves from the mmr
            // So this helps us prevent processing of duplicate outgoing requests
            IncomingRequestAcks::<T>::insert(commitment, Receipt::Ok);
            Ok(DispatchSuccess { dest_chain, source_chain, nonce })
        } else if let Some(ref router) = self.inner {
            router.handle_request(request)
        } else {
            Err(DispatchError {
                msg: "Missing a module router".to_string(),
                nonce: request.nonce(),
                source: request.source_chain(),
                dest: request.dest_chain(),
            })?
        }
    }

    fn handle_timeout(&self, request: Request) -> DispatchResult {
        if let Some(ref router) = self.inner {
            router.handle_timeout(request)
        } else {
            Err(DispatchError {
                msg: "Missing a module router".to_string(),
                nonce: request.nonce(),
                source: request.source_chain(),
                dest: request.dest_chain(),
            })?
        }
    }

    fn handle_response(&self, response: Response) -> DispatchResult {
        let host = Host::<T>::default();

        if host.host_state_machine() != response.dest_chain() {
            let commitment = hash_response::<Host<T>>(&response).0.to_vec();

            if IncomingResponseAcks::<T>::contains_key(commitment.clone()) {
                Err(DispatchError {
                    msg: "Duplicate response".to_string(),
                    nonce: response.nonce(),
                    source: response.source_chain(),
                    dest: response.dest_chain(),
                })?
            }

            let (dest_chain, source_chain, nonce) =
                (response.dest_chain(), response.source_chain(), response.nonce());

            Pallet::<T>::mmr_push(Leaf::Response(response)).ok_or_else(|| DispatchError {
                msg: "Failed to push response into mmr".to_string(),
                nonce,
                source: source_chain,
                dest: dest_chain,
            })?;

            Pallet::<T>::deposit_event(Event::Response {
                request_nonce: nonce,
                dest_chain,
                source_chain,
            });
            IncomingResponseAcks::<T>::insert(commitment, Receipt::Ok);
            Ok(DispatchSuccess { dest_chain, source_chain, nonce })
        } else if let Some(ref router) = self.inner {
            router.handle_response(response)
        } else {
            Err(DispatchError {
                msg: "Missing a module router".to_string(),
                nonce: response.nonce(),
                source: response.source_chain(),
                dest: response.dest_chain(),
            })?
        }
    }
}
