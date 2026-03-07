// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract Keccak256 {
    /// @notice Single keccak256 hash
    function hashOnce(bytes memory data) public pure returns (bytes32) {
        return keccak256(data);
    }

    /// @notice Iterative keccak256 hashing - hash the result repeatedly
    function hashMany(uint256 iterations) public pure returns (bytes32) {
        bytes32 result = keccak256(abi.encodePacked(uint256(0)));
        for (uint256 i = 1; i < iterations; i++) {
            result = keccak256(abi.encodePacked(result));
        }
        return result;
    }

    /// @notice Hash different sized inputs
    function hashVariableSize(uint256 dataSize) public pure returns (bytes32) {
        bytes memory data = new bytes(dataSize);
        for (uint256 i = 0; i < dataSize; i++) {
            data[i] = bytes1(uint8(i & 0xff));
        }
        return keccak256(data);
    }
}
