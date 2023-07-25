// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.2;

struct PostRequest {
    // the source state machine of this request as utf8 string bytes
    bytes source;
    // the destination state machine of this request as utf8 string bytes
    bytes dest;
    // request nonce
    uint256 nonce;
    // Module Id of this request origin
    bytes from;
    // destination module id
    bytes to;
    // timestamp by which this request times out.
    uint256 timeoutTimestamp;
    // request body
    ContractData data;
}

struct GetRequest {
    // the source state machine of this request as utf8 string bytes
    bytes source;
    // the destination state machine of this request as utf8 string bytes
    bytes dest;
    // request nonce
    uint256 nonce;
    // Module Id of this request origin
    bytes from;
    // timestamp by which this request times out.
    uint256 timeoutTimestamp;
    // raw storage keys
    bytes[] keys;
    // height at which to read destination state machine
    uint256 height;
}

struct StorageValue {
    bytes key;
    bytes value;
}

struct GetResponse {
    // The request that initiated this response
    GetRequest request;
    // storage values for get response
    StorageValue[] values;
}

struct PostResponse {
    // The request that initiated this response
    PostRequest request;
    // bytes for post response
    bytes response;
}

// An object for dispatching post requests to the IsmpDispatcher
struct DispatchPost {
    // bytes representation of the destination chain as utf8 string bytes
    bytes dest;
    // the destination module
    bytes to;
    // the request body
    ContractData data;
    // Timeout
    uint256 timeoutTimestamp;
}

// An object for dispatching post requests to the IsmpDispatcher
struct DispatchGet {
    // bytes representation of the destination chain as utf8 string bytes
    bytes dest;
    // Height
    uint256 height;
    // the request body
    bytes[] keys;
    // Timeout
    uint256 timeoutTimestamp;
    // Gas limit that should be used to execute the response or timeout for this request
    uint256 gasLimit;
}

// An object that represents the standard data format for contract post request bodies
// To be abi encoded as the bytes for a request
// This is the data structure expected by all EVM contracts executing on substrate chains
struct ContractData {
    // Actual contract data to that would be abi decoded by contract internally
    bytes data;
    // Gas limit to be used to execute the contract call back on destination chain
    uint256 gasLimit;
}

interface IIsmpModule {
    function OnAccept(PostRequest memory request) external;

    function OnPostResponse(PostResponse memory response) external;

    function OnGetResponse(GetResponse memory response) external;

    function OnPostTimeout(PostRequest memory request) external;

    function OnGetTimeout(GetRequest memory request) external;
}

function encodePostDispatch(
    DispatchPost memory dispatch
) pure returns (bytes memory) {
    return
        abi.encode(
            dispatch.dest,
            dispatch.to,
            dispatch.data,
            dispatch.timeoutTimestamp
        );
}

function encodeGetDispatch(
    DispatchGet memory dispatch
) pure returns (bytes memory) {
    return
        abi.encode(
            dispatch.dest,
            dispatch.height,
            dispatch.keys,
            dispatch.timeoutTimestamp,
            dispatch.gasLimit
        );
}

function encodePostResponse(
    PostResponse memory postResponse
) pure returns (bytes memory) {
   bytes memory request = abi.encode(
            postResponse.request.source,
            postResponse.request.dest,
            postResponse.request.nonce,
            postResponse.request.from,
            postResponse.request.to,
            postResponse.request.timeoutTimestamp,
            postResponse.request.data
        );
    return abi.encode(request, postResponse.response);
}
