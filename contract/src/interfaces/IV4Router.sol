// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

struct PoolKey {
    /// @notice The lower currency of the pool, sorted numerically
    address currency0;
    /// @notice The higher currency of the pool, sorted numerically
    address currency1;
    /// @notice The pool swap fee, capped at 1_000_000. The upper 4 bits determine if the hook sets any fees.
    uint24 fee;
    /// @notice Ticks that involve positions must be a multiple of tick spacing
    int24 tickSpacing;
    /// @notice The hooks of the pool
    address hooks;
}

/// @title IV4Router
/// @notice Interface for the V4Router contract
interface IV4Router {
    /// @notice Emitted when an exactInput swap does not receive its minAmountOut
    error V4TooLittleReceived(uint256 minAmountOutReceived, uint256 amountReceived);
    /// @notice Emitted when an exactOutput is asked for more than its maxAmountIn
    error V4TooMuchRequested(uint256 maxAmountInRequested, uint256 amountRequested);

    /// @notice Parameters for a single-hop exact-input swap
    struct ExactInputSingleParams {
        PoolKey poolKey;
        bool zeroForOne;
        uint128 amountIn;
        uint128 amountOutMinimum;
        bytes hookData;
    }

    // /// @notice Parameters for a multi-hop exact-input swap
    // struct ExactInputParams {
    //     Currency currencyIn;
    //     PathKey[] path;
    //     uint128 amountIn;
    //     uint128 amountOutMinimum;
    // }

    /// @notice Parameters for a single-hop exact-output swap
    struct ExactOutputSingleParams {
        PoolKey poolKey;
        bool zeroForOne;
        uint128 amountOut;
        uint128 amountInMaximum;
        bytes hookData;
    }

    // /// @notice Parameters for a multi-hop exact-output swap
    // struct ExactOutputParams {
    //     Currency currencyOut;
    //     PathKey[] path;
    //     uint128 amountOut;
    //     uint128 amountInMaximum;
    // }

    function execute(bytes calldata commands, bytes[] calldata inputs, uint256 deadline) external;
}

IV4Router constant routerV4 = IV4Router(0x66a9893cC07D91D95644AEDD05D03f95e1dBA8Af);


interface IPoolManager {
    struct SwapParams {
        bool zeroForOne;
        int256 amountSpecified;
        uint160 sqrtPriceLimitX96;
    }

    function unlock(bytes calldata data) external returns (bytes memory result);

    function take(address currency, address to, uint256 amount) external;
    
    function swap(PoolKey memory key, IPoolManager.SwapParams memory params, bytes calldata hookData) external
        returns (int256 delta);

    function sync(address currency) external;

    function settle() external returns (uint256);
}

IPoolManager constant   poolManager = IPoolManager(0x000000000004444c5dc75cB358380D2e3dE08A90);
