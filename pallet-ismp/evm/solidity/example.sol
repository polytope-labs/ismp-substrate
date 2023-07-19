// SPDX-License-Identifier: UNLICENSED
// Sample ISMP solidity contract for unit tests

pragma solidity ^0.8.0;

import "./ismp_defs.sol";

error NotIsmpHost();

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
    // Mapping of destination chains to the module ids
    mapping(bytes => bytes) public moduleIds;
    event ResponseReceived();
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
        Payload memory payload = decodePayload(request.data.data);
        PostResponse memory response = PostResponse({
            request: request,
            response: abi.encodePacked(msg.sender)
        });
        _mint(payload.to, payload.amount);
        // For this test we expect the ismp post dispatch precompile to be at the  address 0x03
        // In production you would use the precompile address provided by the chain to make the dispatch
        (bool ok, bytes memory out) = address(3).staticcall(
            abi.encode(response)
        );
        emit BalanceMinted();
    }

    function OnPostResponse(PostResponse memory response) public onlyIsmpHost {
        Payload memory payload = decodePayload(response.request.data.data);
        emit ResponseReceived();
    }

    function OnGetResponse(GetResponse memory response) public onlyIsmpHost {
        emit ResponseReceived();
    }

    function OnGetTimeout(GetRequest memory request) public onlyIsmpHost {
        emit ResponseReceived();
    }

    function OnPostTimeout(PostRequest memory request) public onlyIsmpHost {
        Payload memory payload = decodePayload(request.data.data);
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
        uint64 gasLimit
    ) public {
        _burn(msg.sender, amount);
        Payload memory payload = Payload({
            from: msg.sender,
            to: to,
            amount: amount
        });
        ContractData memory contract_data = ContractData({
            data: abi.encode(payload),
            gasLimit: gasLimit
        });
        DispatchPost memory dispatchPost = DispatchPost({
            data: contract_data,
            dest: dest,
            timeoutTimestamp: timeout,
            to: moduleIds[dest]
        });
        // For this test we expect the ismp post dispatch precompile to be at the  address 0x01
        // In production you would use the precompile address provided by the chain to make the dispatch
        address(1).staticcall(abi.encode(dispatchPost));
        emit BalanceBurnt();
    }

    function dispatchGet(
        bytes memory dest,
        bytes[] memory keys,
        uint256 height,
        uint256 timeout,
        uint64 gasLimit
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
        address(2).staticcall(abi.encode(get));
        emit GetDispatched();
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
