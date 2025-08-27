use ethers::prelude::*;

abigen!(
    MSLauncher,
    "src/abi/MSLauncher.abi",
    event_derives(serde::Deserialize, serde::Serialize)
);

abigen!(
    MSLauncherMainnet,
    "src/abi/MSLauncherMainnet.abi",
    event_derives(serde::Deserialize, serde::Serialize)
);

abigen!(
    MSLauncherRouterMainnet,
    "src/abi/MSLauncherRouterMainnet.abi",
    event_derives(serde::Deserialize, serde::Serialize)
);


abigen!(
    MSSimulatorMainnet,
    "src/abi/MSSimulatorMainnet.abi",
    event_derives(serde::Deserialize, serde::Serialize)
);

abigen!(
    IERC20,
    "src/abi/IERC20.abi",
    event_derives(serde::Deserialize, serde::Serialize)
);