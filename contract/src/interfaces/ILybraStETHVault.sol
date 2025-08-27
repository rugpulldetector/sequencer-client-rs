pragma solidity 0.8.24;

interface LybraStETHVault {
    function excessIncomeDistribution(uint256 stETHAmount) external;
    function totalDepositedAsset() external view returns(uint256);
    function getDutchAuctionDiscountPrice() external view returns (uint256);
    function lidoRebaseTime() external view returns (uint256);
    function getAssetPrice() external returns (uint256);

    function depositEtherToMint(uint256 mintAmount) external payable;
    function depositAssetToMint(uint256 assetAmount, uint256 mintAmount) external;
    function withdraw(address onBehalfOf, uint256 amount) external;
    function mint(address onBehalfOf, uint256 amount) external;
    function burn(address onBehalfOf, uint256 amount) external;
}

LybraStETHVault constant lybraStETHVault = LybraStETHVault(0xa980d4c0C2E48d305b582AA439a3575e3de06f0E);