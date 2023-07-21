use crate::{default::DefaultConfig, verify_parachain_headers_with_grandpa_finality_proof};
use codec::{Decode, Encode};
use futures::StreamExt;
use grandpa_prover::GrandpaProver;
use ismp::host::StateMachine;
use polkadot_core_primitives::Header;
use primitives::{
    justification::GrandpaJustification, FinalityProof, ParachainHeadersWithFinalityProof,
};
use serde::{Deserialize, Serialize};
use sp_core::H256;
use std::sync::Arc;
use subxt::{
    config::substrate::{BlakeTwo256, SubstrateHeader},
    rpc_params,
};

pub type Justification = GrandpaJustification<Header>;

/// An encoded justification proving that the given header has been finalized
#[derive(Clone, Serialize, Deserialize)]
pub struct JustificationNotification(sp_core::Bytes);

#[tokio::test]
#[ignore]
async fn follow_grandpa_justifications() {
    env_logger::builder()
        .filter_module("grandpa", log::LevelFilter::Trace)
        .format_module_path(false)
        .init();

    let relay = std::env::var("RELAY_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let para = std::env::var("PARA_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

    let relay_ws_url = format!("ws://{relay}:9944");
    let _para_ws_url = format!("ws://{para}:9188");

    let para_ids = Vec::new();
    let babe_epoch_start = Vec::new();

    let consensus_state_id = [0u8; 4];

    let prover = GrandpaProver::<DefaultConfig>::new(
        &relay_ws_url,
        para_ids,
        StateMachine::Grandpa(consensus_state_id),
        babe_epoch_start,
        Vec::new(),
    )
    .await
    .unwrap();

    println!("Waiting for grandpa proofs to become available");
    let session_length = prover.session_length().await.unwrap();
    prover
        .client
        .blocks()
        .subscribe_finalized()
        .await
        .unwrap()
        .filter_map(|result| futures::future::ready(result.ok()))
        .skip_while(|h| futures::future::ready(h.number() < (session_length * 2) + 10))
        .take(1)
        .collect::<Vec<_>>()
        .await;

    let mut subscription = prover
        .client
        .rpc()
        .subscribe::<JustificationNotification>(
            "grandpa_subscribeJustifications",
            rpc_params![],
            "grandpa_unsubscribeJustifications",
        )
        .await
        .unwrap()
        .take((2 * session_length).try_into().unwrap());

    let slot_duration = 0;

    let mut consensus_state = prover.initialize_consensus_state(slot_duration).await.unwrap();
    println!("Grandpa proofs are now available");
    while let Some(Ok(JustificationNotification(sp_core::Bytes(_)))) = subscription.next().await {
        let next_relay_height = consensus_state.latest_height + 1;

        let encoded = finality_grandpa_rpc::GrandpaApiClient::<JustificationNotification, H256, u32>::prove_finality(
            // we cast between the same type but different crate versions.
            &*unsafe {
                unsafe_arc_cast::<_, jsonrpsee_ws_client::WsClient>(prover.ws_client.clone())
            },
            next_relay_height,
        )
            .await
            .unwrap()
            .unwrap()
            .0;

        let finality_proof =
            FinalityProof::<SubstrateHeader<u32, BlakeTwo256>>::decode(&mut &encoded[..]).unwrap();

        let justification = Justification::decode(&mut &finality_proof.justification[..]).unwrap();

        let para_id = 0;

        let finalized_para_header = prover
            .query_latest_finalized_parachain_header(para_id, justification.commit.target_number)
            .await
            .expect("Failed to fetch finalized parachain headers");

        // notice the inclusive range
        let header_numbers = ((consensus_state.latest_height + 1)..=finalized_para_header.number)
            .collect::<Vec<_>>();

        if header_numbers.len() == 0 {
            continue
        }

        println!("current_set_id: {}", consensus_state.current_set_id);
        println!("latest_relay_height: {}", consensus_state.latest_height);
        println!(
            "For relay chain header: Hash({:?}), Number({})",
            justification.commit.target_hash, justification.commit.target_number
        );

        dbg!(&consensus_state.latest_height);
        dbg!(&header_numbers);

        let proof = prover
            .query_finalized_parachain_headers_with_proof::<SubstrateHeader<u32, BlakeTwo256>>(
                consensus_state.latest_height,
                justification.commit.target_number,
                finality_proof.clone(),
            )
            .await
            .expect("Failed to fetch finalized parachain headers with proof");

        let proof = proof.encode();
        let proof = ParachainHeadersWithFinalityProof::<Header>::decode(&mut &*proof).unwrap();

        let (new_consensus_state, _parachain_headers) =
            verify_parachain_headers_with_grandpa_finality_proof::<Header>(
                consensus_state.clone(),
                proof.clone(),
            )
            .expect("Failed to verify parachain headers with grandpa finality_proof");

        if !proof.parachain_headers.is_empty() {
            assert!(new_consensus_state.latest_height > consensus_state.latest_height);
        }

        consensus_state = new_consensus_state;
        println!("========= Successfully verified grandpa justification =========");
    }
}

/// Perform a highly unsafe type-casting between two types hidden behind an Arc.
pub unsafe fn unsafe_arc_cast<T, U>(arc: Arc<T>) -> Arc<U> {
    let ptr = Arc::into_raw(arc).cast::<U>();
    Arc::from_raw(ptr)
}
