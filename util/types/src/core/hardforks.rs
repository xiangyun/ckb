//! Types for hard forks.

use crate::core::BlockNumber;

/// The hard fork version.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum HardFork {
    /// No hard forks.
    Not,
    /// Enable hard fork v2021.
    V2021,
}

/// A switch to select hard fork version base on the block number.
#[derive(Debug, Clone)]
pub struct HardForkSwitch {
    v2021_start_number: BlockNumber,
}

/// Builder for [`HardForkSwitch`].
///
/// [`HardForkSwitch`]:  struct.HardForkSwitch.html
#[derive(Debug, Default, Copy, Clone)]
pub struct HardForkSwitchBuilder {
    v2021_start_number: Option<BlockNumber>,
}

impl HardForkSwitch {
    /// Creates a new builder to build an instance.
    pub fn new_builder() -> HardForkSwitchBuilder {
        Default::default()
    }

    /// Returns the hard fork version.
    pub fn version(&self, number: BlockNumber) -> HardFork {
        if number >= self.v2021_start_number {
            HardFork::V2021
        } else {
            HardFork::Not
        }
    }

    /// Returns a switch without any hard fork.
    pub fn always_no_fork() -> Self {
        Self {
            v2021_start_number: BlockNumber::MAX,
        }
    }

    /// Returns a switch enable v2021 hard fork since genesis block.
    pub fn always_v2021() -> Self {
        Self {
            v2021_start_number: 0,
        }
    }
}

impl HardForkSwitchBuilder {
    /// Set the start number for hard fork v2021.
    pub fn set_v2021_start_number(&mut self, number: BlockNumber) -> &mut Self {
        self.v2021_start_number = Some(number);
        self
    }

    /// Build a new [`HardForkSwitch`].
    ///
    /// [`HardForkSwitch`]: struct.HardForkSwitch.html
    pub fn build(self) -> HardForkSwitch {
        let v2021_start_number = self.v2021_start_number.unwrap_or(BlockNumber::MAX);
        HardForkSwitch { v2021_start_number }
    }
}

#[test]
fn test_cmp() {
    assert_eq!(HardFork::Not, HardFork::Not);
    assert_eq!(HardFork::V2021, HardFork::V2021);

    assert!(HardFork::Not < HardFork::V2021);
}
