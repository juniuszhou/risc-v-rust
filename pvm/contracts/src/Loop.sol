// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract Loop {
    /// @notice Simple for loop
    function simpleLoop(uint256 n) public pure returns (uint256) {
        uint256 sum = 0;
        for (uint256 i = 0; i < n; i++) {
            sum += i;
        }
        return sum;
    }

    /// @notice Nested for loops - O(n^2) complexity
    function nestedLoop(uint256 n) public pure returns (uint256) {
        uint256 sum = 0;
        for (uint256 i = 0; i < n; i++) {
            for (uint256 j = 0; j < n; j++) {
                sum += i * j;
            }
        }
        return sum;
    }

    /// @notice While loop variant
    function whileLoop(uint256 n) public pure returns (uint256) {
        uint256 sum = 0;
        uint256 i = 0;
        while (i < n) {
            sum += i;
            i++;
        }
        return sum;
    }
}
