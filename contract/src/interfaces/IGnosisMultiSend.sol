// SPDX-License-Identifier: LGPL-3.0-only
pragma solidity 0.8.24;

interface IGnosisMultiSend {
    function multiSend(bytes memory transactions) external payable;
}
