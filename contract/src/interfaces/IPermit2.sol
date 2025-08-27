pragma solidity 0.8.24;

interface IPermit2 {
    function approve(address token, address spender, uint160 amount, uint48 expiration) external;
}

address constant       permit2 = 0x000000000022D473030F116dDEE9F6B43aC78BA3;
