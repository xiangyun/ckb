//! Hard forks parameters.

use ckb_constant::hardforks::{mainnet, testnet};
use ckb_types::core::{hardforks, BlockNumber};
use serde::{Deserialize, Serialize};

/// Hard forks parameters for spec.
#[derive(Default, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct HardForks {
    /// The hard fork v2021 will start at this block.
    v2021_start_number: Option<BlockNumber>,
}

impl From<HardForks> for hardforks::HardForkSwitch {
    fn from(input: HardForks) -> hardforks::HardForkSwitch {
        let mut builder = hardforks::HardForkSwitch::new_builder();
        if let Some(value) = input.v2021_start_number {
            builder.set_v2021_start_number(value);
        }
        builder.build()
    }
}

macro_rules! if_notset_or_correct {
    ($chain:ident, $parameter:ident, $actual:expr) => {
        if let Some(value) = $actual {
            if value != $chain::$parameter {
                let errmsg = format!(
                    "The parameter \"{}\" for {} is incorrect,
                    actual: {}, expected: {}.",
                    stringify!($parameter),
                    stringify!($chain),
                    value,
                    $chain::$parameter
                );
                return Err(errmsg);
            }
        } else {
            $actual = Some($chain::$parameter);
        }
    };
}

impl HardForks {
    /// Sets all None to default values if all parameters which have been set
    /// are correct for mainnet, otherwise, return an `Err`.
    pub fn complete_mainnet(mut self) -> Result<Self, String> {
        if_notset_or_correct!(mainnet, V2021_START_NUMBER, self.v2021_start_number);
        Ok(self)
    }

    /// Sets all None to default values if all parameters which have been set
    /// are correct for testnet, otherwise, return an `Err`.
    pub fn complete_testnet(mut self) -> Result<Self, String> {
        if_notset_or_correct!(testnet, V2021_START_NUMBER, self.v2021_start_number);
        Ok(self)
    }
}
