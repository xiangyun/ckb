use ckb_types::core::hardforks::HardFork;
#[cfg(has_asm)]
use ckb_vm::machine::asm::AsmCoreMachine;
use ckb_vm::{
    machine::{VERSION0, VERSION1},
    ISA_B, ISA_IMC, ISA_MOP,
};
#[cfg(any(not(has_asm), test))]
use ckb_vm::{DefaultCoreMachine, SparseMemory, WXorXMemory};

use crate::CoreMachineType;

pub(crate) fn core_machine(hardfork: HardFork, max_cycles: u64) -> CoreMachineType {
    let version = match hardfork {
        HardFork::Not => VERSION0,
        HardFork::V2021 => VERSION1,
    };
    let isa = match hardfork {
        HardFork::Not => ISA_IMC,
        HardFork::V2021 => ISA_IMC | ISA_B | ISA_MOP,
    };
    #[cfg(has_asm)]
    let core_machine = AsmCoreMachine::new(isa, version, max_cycles);
    #[cfg(not(has_asm))]
    let core_machine =
        DefaultCoreMachine::<u64, WXorXMemory<SparseMemory<u64>>>::new(isa, version, max_cycles);
    core_machine
}

#[cfg(test)]
pub(crate) fn default_core_machine(
    hardfork: HardFork,
    max_cycles: u64,
) -> DefaultCoreMachine<u64, WXorXMemory<SparseMemory<u64>>> {
    let version = match hardfork {
        HardFork::Not => VERSION0,
        HardFork::V2021 => VERSION1,
    };
    let isa = match hardfork {
        HardFork::Not => ISA_IMC,
        HardFork::V2021 => ISA_IMC | ISA_B | ISA_MOP,
    };
    DefaultCoreMachine::<u64, WXorXMemory<SparseMemory<u64>>>::new(isa, version, max_cycles)
}
