// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract Storage {
    mapping(uint256 => uint256) public data;
    uint256 public counter;

    /// @notice Write to sequential storage slots
    function writeSequential(uint256 count) public {
        for (uint256 i = 0; i < count; i++) {
            data[i] = i * 2;
        }
        counter = count;
    }

    /// @notice Read from sequential storage slots
    function readSequential(uint256 count) public view returns (uint256) {
        uint256 sum = 0;
        for (uint256 i = 0; i < count; i++) {
            sum += data[i];
        }
        return sum;
    }

    /// @notice Interleaved write and read operations
    function writeRead(uint256 iterations) public {
        for (uint256 i = 0; i < iterations; i++) {
            data[i] = i;
            counter += data[i];
        }
    }
}
