use serde::Serialize;

use super::cfg::BlockId;

/// SSA variable: each assigned exactly once
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct SsaVar {
    pub name: String,
    pub version: u32,
}

impl std::fmt::Display for SsaVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}_{}", self.name, self.version)
    }
}

/// IR instructions — normalized operations
#[derive(Debug, Clone)]
pub enum Instruction {
    // Core operations
    Assign {
        dest: SsaVar,
        value: Operand,
    },
    BinaryOp {
        dest: SsaVar,
        op: BinaryOp,
        left: Operand,
        right: Operand,
    },
    UnaryOp {
        dest: SsaVar,
        op: UnaryOp,
        operand: Operand,
    },
    Phi {
        dest: SsaVar,
        sources: Vec<(SsaVar, BlockId)>,
    },

    // Function calls
    Call {
        dest: Option<SsaVar>,
        func: String,
        args: Vec<Operand>,
    },
    MethodCall {
        dest: Option<SsaVar>,
        receiver: Operand,
        method: String,
        args: Vec<Operand>,
    },

    // CosmWasm-specific
    StorageLoad {
        dest: SsaVar,
        storage_item: String,
        key: Option<Operand>,
    },
    StorageStore {
        storage_item: String,
        key: Option<Operand>,
        value: Operand,
    },
    AddrValidate {
        dest: SsaVar,
        address: Operand,
    },
    SendMsg {
        msg_type: String,
        fields: Vec<(String, Operand)>,
    },
    CheckSender {
        sender_var: Operand,
        expected: Operand,
    },

    // Control flow
    Branch {
        condition: Operand,
        true_block: BlockId,
        false_block: BlockId,
    },
    Jump {
        target: BlockId,
    },
    Return {
        value: Option<Operand>,
    },

    // Error handling
    ResultUnwrap {
        dest: SsaVar,
        value: Operand,
    },
    ErrorReturn {
        error: Operand,
    },
}

/// Operand — values used in instructions
#[derive(Debug, Clone)]
pub enum Operand {
    Var(SsaVar),
    Literal(LiteralValue),
    FieldAccess { base: Box<Operand>, field: String },
}

/// Binary operations
#[derive(Debug, Clone, Serialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Unknown,
}

/// Unary operations
#[derive(Debug, Clone, Serialize)]
pub enum UnaryOp {
    Not,
    Neg,
    Deref,
    Ref,
    Unknown,
}

/// Literal values
#[derive(Debug, Clone, Serialize)]
pub enum LiteralValue {
    Int(i128),
    Uint(u128),
    String(String),
    Bool(bool),
    Unit,
}
