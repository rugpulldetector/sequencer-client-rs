// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

struct PoolKey {
    /// @notice The lower currency of the pool, sorted numerically
    address token0;
    /// @notice The higher currency of the pool, sorted numerically
    address token1;
    /// @notice The pool swap fee, capped at 1_000_000. The upper 4 bits determine if the hook sets any fees.
    uint24 fee;
    /// @notice Ticks that involve positions must be a multiple of tick spacing
    int24 tickSpacing;
    /// @notice The hooks of the pool
    address hooks;
}