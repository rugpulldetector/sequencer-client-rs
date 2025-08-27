pragma solidity 0.8.24;

interface AggregatorInterface {
	function aggregator() external view returns (address);
}

interface IChainlinkAdapter is AggregatorInterface {
    function latestRoundData() external view returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound);
	function latestAnswer() external view returns (int256);
	function latestTimestamp() external view returns (uint256);
	function chainlinkFeed() external view returns (AggregatorInterface);
	function ASSET_TO_USD_AGGREGATOR() external view returns (address);
}
