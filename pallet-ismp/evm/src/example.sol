// SPDX-License-Identifier: UNLICENSED
// Sample ISMP solidity contract for unit tests

pragma solidity ^0.8.0;

import "./ismp_defs.sol";

error NotIsmpHost();

contract IsmpDemo is IIsmpModule {
    address ismpHost;

    // restricts call to `IsmpHost`
    modifier onlyIsmpHost() {
        if (msg.sender != ismpHost) {
            revert NotIsmpHost();
        }
        _;
    }

    constructor(address _ismpHost) {
        ismpHost = _ismpHost;
    }

    function OnAccept(PostRequest memory request) public onlyIsmpHost {
        
    }

    function OnResponse(PostResponse memory response) public onlyIsmpHost {
        
    }

    function OnGetTimeout(GetRequest memory request) public onlyIsmpHost {
       
    }

    function OnPostTimeout(PostRequest memory request) public onlyIsmpHost {
        
    }

    function decodeContractData(
        bytes memory data
    ) internal pure returns (ContractData memory contractData) {}

    function transfer() public {}
}
