// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract Arithmetic {
    /// @notice Simple arithmetic computation
    function compute(uint256 a, uint256 b) public pure returns (uint256) {
        uint256 result = 0;
        result += a + b;
        result += a * b;
        result += a / (b + 1);
        result += a % (b + 1);
        return result;
    }

    /// @notice Iterative arithmetic computation
    function computeMany(uint256 iterations) public pure returns (uint256) {
        uint256 sum = 0;
        for (uint256 i = 1; i <= iterations; i++) {
            sum += (i * i) / i;
        }
        return sum;
    }
}
