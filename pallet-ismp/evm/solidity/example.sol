// SPDX-License-Identifier: UNLICENSED
// A Sample ISMP solidity contract for unit tests

pragma solidity ^0.8.2;

import "./ismp_defs.sol";

error NotIsmpHost();
error ExecutionFailed();

struct Payload {
    address to;
    address from;
    uint256 amount;
}

contract IsmpDemo is IIsmpModule {
    address ismpHost;
    uint256 totalSupply;

    // Mapping of user address to balance
    mapping(address => uint256) public balances;
    event ResponseReceived();
    event TimeoutReceived();
    event BalanceMinted();
    event BalanceBurnt();
    event GetDispatched();

    // restricts call to `IsmpHost`
    modifier onlyIsmpHost() {
        if (msg.sender != ismpHost) {
            revert NotIsmpHost();
        }
        _;
    }

    constructor() {
        ismpHost = address(0);
        totalSupply = 1000000000;
    }

    function OnAccept(PostRequest memory request) public onlyIsmpHost {
        Payload memory payload = decodePayload(request.data);
        PostResponse memory response = PostResponse({
            request: request,
            response: abi.encodePacked(msg.sender)
        });
        _mint(payload.to, payload.amount);
        // For this test we expect the ismp post dispatch precompile to be at the  address 0x03
        // In production you would use the precompile address provided by the chain to make the dispatch
        bytes memory input = encodePostResponse(response);
        (bool ok, bytes memory out) = address(3).staticcall(input);
        if (ok) {
            emit BalanceMinted();
        } else {
            revert ExecutionFailed();
        }
    }

    function OnPostResponse(PostResponse memory response) public onlyIsmpHost {
        // In this callback just try to decode the payload of the corresponding request
        Payload memory payload = decodePayload(response.request.data);
        emit ResponseReceived();
    }

    function OnGetResponse(GetResponse memory response) public onlyIsmpHost {
        // For the purpose of this test
        // we just validate the responses in this callback
        for (uint256 index = 0; index < response.values.length; index++) {
            StorageValue memory storageValue = response.values[index];
            if (storageValue.value.length == 0) {
                revert ExecutionFailed();
            }
        }
        emit ResponseReceived();
    }

    function OnGetTimeout(GetRequest memory request) public onlyIsmpHost {
        // We validate the keys in this callback
        for (uint256 index = 0; index < request.keys.length; index++) {
            bytes memory key = request.keys[index];
            // No keys should be empty
            if (key.length == 0) {
                revert ExecutionFailed();
            }
        }
        emit TimeoutReceived();
    }

    function OnPostTimeout(PostRequest memory request) public onlyIsmpHost {
        Payload memory payload = decodePayload(request.data);
        _mint(payload.from, payload.amount);
        emit BalanceMinted();
    }

    function decodePayload(
        bytes memory data
    ) internal pure returns (Payload memory payload) {
        (payload) = abi.decode(data, (Payload));
        return payload;
    }

    function transfer(
        address to,
        bytes memory dest,
        uint256 amount,
        uint256 timeout,
        uint256 gasLimit
    ) public {
        _burn(msg.sender, amount);
        Payload memory payload = Payload({
            from: msg.sender,
            to: to,
            amount: amount
        });
        DispatchPost memory dispatchPost = DispatchPost({
            data: abi.encode(payload.from, payload.to, payload.amount),
            dest: dest,
            timeoutTimestamp: timeout,
            to: abi.encodePacked(address(12)),
            gasLimit: gasLimit
        });
        // For this test we expect the ismp post dispatch precompile to be at the  address 0x01
        // In production you would use the precompile address provided by the chain to make the dispatch
        bytes memory input = encodePostDispatch(dispatchPost);
        (bool ok, bytes memory out) = address(1).staticcall(input);
        if (ok) {
            emit BalanceBurnt();
        } else {
            revert ExecutionFailed();
        }
    }

    function dispatchGet(
        bytes memory dest,
        bytes[] memory keys,
        uint256 height,
        uint256 timeout,
        uint256 gasLimit
    ) public {
        DispatchGet memory get = DispatchGet({
            keys: keys,
            dest: dest,
            height: height,
            timeoutTimestamp: timeout,
            gasLimit: gasLimit
        });
        // For this test we expect the ismp get dispatch precompile to be at the  address 0x02
        // In production you would use the precompile address provided by the chain to make the dispatch
        bytes memory input = encodeGetDispatch(get);
        (bool ok, bytes memory out) = address(2).staticcall(input);
        if (ok) {
            emit GetDispatched();
        } else {
            revert ExecutionFailed();
        }
    }

    function mintTo(address who, uint256 amount) public onlyIsmpHost {
        _mint(who, amount);
    }

    function _mint(address who, uint256 amount) internal {
        totalSupply = totalSupply + amount;
        balances[who] = balances[who] + amount;
    }

    function _burn(address who, uint256 amount) internal {
        totalSupply = totalSupply - amount;
        balances[who] = balances[who] - amount;
    }
}
