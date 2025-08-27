pragma solidity 0.8.24;
interface ICurvePool{
    function exchange(int128 i, int128 j, uint256 dx, uint256 min_dy) external returns(uint256);
    function exchange_underlying(int128 i, int128 j, uint256 dx, uint256 min_dy) external returns(uint256);
    function get_dy_underlying(int128 i, int128 j, uint256 amount) external view returns(uint256);
}

interface ICurvePool_Crypto{
    function exchange_underlying(uint256 i, uint256 j, uint256 dx, uint256 min_dy) payable external returns(uint256);
    function get_dx(uint256 i, uint256 j, uint256 dy) external view returns(uint256);
}
