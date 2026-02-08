pub mod builder;
pub mod cfg;
pub mod instruction;
pub mod types;

pub use cfg::{BasicBlock, BlockId, Cfg};
pub use instruction::{BinaryOp, Instruction, LiteralValue, Operand, SsaVar, UnaryOp};
pub use types::{ContractIr, FunctionIr};
