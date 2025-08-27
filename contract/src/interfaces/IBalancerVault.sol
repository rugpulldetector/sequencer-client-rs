pragma solidity 0.8.24;

interface IBalancerVault {
  enum SwapKind { GIVEN_IN, GIVEN_OUT }
  struct SingleSwap {
        bytes32 poolId;
        SwapKind kind;
        address assetIn;
        address assetOut;
        uint256 amount;
        bytes userData;
    }
  struct FundManagement {
        address sender;
        bool fromInternalBalance;
        address payable recipient;
        bool toInternalBalance;
    }
  function swap(
        SingleSwap memory singleSwap,
        FundManagement memory funds,
        uint256 limit,
        uint256 deadline
    )
        external
        payable
        returns(uint256 amountCalculated);
  struct JoinPoolRequest {
        address[] asset;
        uint256[] maxAmountsIn;
        bytes userData;
        bool fromInternalBalance;
    }

  struct ExitPoolRequest {
        address[] asset;
        uint256[] minAmountsOut;
        bytes userData;
        bool toInternalBalance;
    }

  function joinPool(
        bytes32 poolId,
        address sender,
        address recipient,
        JoinPoolRequest memory request
    ) external payable;
  
  function exitPool(
        bytes32 poolId,
        address sender,
        address payable recipient,
        ExitPoolRequest memory request
    ) external payable;

  function flashLoan(
    address recipient,
    address[] memory tokens,
    uint256[] memory amounts,
    bytes memory userData
  ) external;
}

IBalancerVault constant  balancer = IBalancerVault(0xBA12222222228d8Ba445958a75a0704d566BF2C8);
