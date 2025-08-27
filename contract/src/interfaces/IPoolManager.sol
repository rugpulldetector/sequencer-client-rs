// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import './IPoolKey.sol';
import "../types/BalanceDelta.sol";

interface IPoolManager {
    struct SwapParams {
        bool zeroForOne;
        int256 amountSpecified;
        uint160 sqrtPriceLimitX96;
    }

    function unlock(bytes calldata data) external returns (bytes memory result);

    function take(address currency, address to, uint256 amount) external;
    
    function swap(PoolKey memory key, IPoolManager.SwapParams memory params, bytes calldata hookData) external
        returns (BalanceDelta delta);

    function sync(address currency) external;

    function settle() external payable returns (uint256);
}

interface IStateView {
    function getSlot0(bytes32 poolId)
        external
        view
        returns (uint160 sqrtPriceX96, int24 tick, uint24 protocolFee, uint24 lpFee);
}
