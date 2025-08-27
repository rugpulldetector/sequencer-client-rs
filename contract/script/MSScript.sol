// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.24;

import "forge-std/Script.sol";
import "../src/MSLauncherBase.sol";
import "../src/MSLauncherOP.sol";
import "../src/MSLauncherMainnet.sol";

contract DeployBaseScript is Script {

    function setUp() public {
    }

    function run() public {
        vm.startBroadcast(base_operator_3);

        // address(base_operator_2).call{value: 0.0001 ether}("");
        // address(base_operator_2).call{value: 0.0001 ether}("");

        MSLauncherBase launcher = new MSLauncherBase(base_controller);
        MSLauncherBase simulator = new MSLauncherBase(base_controller);

        console.log("Base Launcher", address(launcher));
        console.log("Base Simulator", address(simulator));

        vm.stopBroadcast();
    }
}


contract DeployOPScript is Script {

    function setUp() public {
    }

    function run() public {
        vm.startBroadcast(base_operator_2);

        // base_operator_2.call{value: 0.0001 ether}("");

        MSLauncherOP_WETH_USDC launcher = new MSLauncherOP_WETH_USDC(base_controller);
        MSLauncherOP_WETH_USDC simulator = new MSLauncherOP_WETH_USDC(base_controller);

        console.log("Launcher", address(launcher));
        console.log("Simulator", address(simulator));

        MSLauncherOP_WETH_OP launcher2 = new MSLauncherOP_WETH_OP(base_controller);
        MSLauncherOP_WETH_OP simulator2 = new MSLauncherOP_WETH_OP(base_controller);

        console.log("Launcher2", address(launcher2));
        console.log("Simulator2", address(simulator2));

        // MSLauncherOP_OP_USDC launcher = new MSLauncherOP_OP_USDC(base_controller);
        // MSLauncherOP_OP_USDC simulator = new MSLauncherOP_OP_USDC(base_controller);

        // MSLauncherOP_WETH_USDC_E wethUSDCeLauncher = new MSLauncherOP_WETH_USDC_E(base_controller, 0.2 ether, 0.07 ether);
        // MSLauncherOP_WETH_OP wethOPLauncher = new MSLauncherOP_WETH_OP(base_controller, 0.2 ether, 0.07 ether);
        // MSLauncherOP_OP_USDC opUSDCLauncher = new MSLauncherOP_OP_USDC(base_controller);
        // MSLauncherOP_OP_USDC_E opUSDCeLauncher = new MSLauncherOP_OP_USDC_E(base_controller);

        // console.log("OP WETH_USDC launcher", address(wethUSDCLauncher));
        // console.log("OP WETH_USDC_E launcher", address(wethUSDCeLauncher)); 
        // console.log("OP WETH_OP launcher", address(wethOPLauncher));

        vm.stopBroadcast();
    }
}



contract DeployMainnetScript is Script {

    function setUp() public {
    }

    function run() public {
        vm.startBroadcast(base_operator_2);

        MSLauncherRouterMainnnet router = new MSLauncherRouterMainnnet(base_controller);
        console.log("Router", address(router));

        // MSLauncherMainnet mainnetLauncher = new MSLauncherMainnet(base_controller);
        // console.log("Mainnet Launcher", address(mainnetLauncher));

        // MSSimulatorMainnet mainnetSimulator = new MSSimulatorMainnet();
        // console.log("Mainnet Simulator", address(mainnetSimulator));

        vm.stopBroadcast();
    }
}

