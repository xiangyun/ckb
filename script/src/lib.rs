//! CKB component to run the type/lock scripts.
pub mod cost_model;
mod error;
pub(crate) mod hardforks;
mod ill_transaction_checker;
mod syscalls;
mod type_id;
mod types;
mod verify;

pub use crate::error::{ScriptError, TransactionScriptError};
pub use crate::ill_transaction_checker::IllTransactionChecker;
pub use crate::types::{ScriptGroup, ScriptGroupType};
pub(crate) use crate::verify::CoreMachineType;
pub use crate::verify::TransactionScriptsVerifier;
