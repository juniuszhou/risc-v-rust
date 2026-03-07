// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

contract Counter {
    uint256 public count;

    event Incremented(uint256 newCount);
    event Set(uint256 newCount);

    function increment() external {
        count += 1;
        emit Incremented(count);
    }

    function set(uint256 _count) external {
        count = _count;
        emit Set(count);
    }
}
