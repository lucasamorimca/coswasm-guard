use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::instruction::{Instruction, Operand, SsaVar};

pub type BlockId = usize;

/// A basic block in the CFG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicBlock {
    pub id: BlockId,
    pub instructions: Vec<Instruction>,
    pub successors: Vec<BlockId>,
    pub predecessors: Vec<BlockId>,
}

impl BasicBlock {
    pub fn new(id: BlockId) -> Self {
        Self {
            id,
            instructions: Vec::new(),
            successors: Vec::new(),
            predecessors: Vec::new(),
        }
    }
}

/// Def-use information for a single SSA variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefUse {
    pub def_block: BlockId,
    pub def_instruction_idx: usize,
    pub uses: Vec<(BlockId, usize)>,
}

/// Control flow graph for a single function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cfg {
    pub function_name: String,
    pub blocks: Vec<BasicBlock>,
    pub entry_block: BlockId,
    pub exit_blocks: Vec<BlockId>,
}

impl Cfg {
    pub fn new(function_name: String) -> Self {
        Self {
            function_name,
            blocks: Vec::new(),
            entry_block: 0,
            exit_blocks: Vec::new(),
        }
    }

    /// Add a new basic block, returning its ID
    pub fn add_block(&mut self) -> BlockId {
        let id = self.blocks.len();
        self.blocks.push(BasicBlock::new(id));
        id
    }

    /// Add an edge from source to target
    pub fn add_edge(&mut self, source: BlockId, target: BlockId) {
        if !self.blocks[source].successors.contains(&target) {
            self.blocks[source].successors.push(target);
        }
        if !self.blocks[target].predecessors.contains(&source) {
            self.blocks[target].predecessors.push(source);
        }
    }

    /// Get all variables defined in this function
    pub fn defined_vars(&self) -> HashSet<SsaVar> {
        let mut vars = HashSet::new();
        for block in &self.blocks {
            for inst in &block.instructions {
                if let Some(var) = instruction_def(inst) {
                    vars.insert(var.clone());
                }
            }
        }
        vars
    }

    /// Get all variables used in this function
    pub fn used_vars(&self) -> HashSet<SsaVar> {
        let mut vars = HashSet::new();
        for block in &self.blocks {
            for inst in &block.instructions {
                for var in instruction_uses(inst) {
                    vars.insert(var.clone());
                }
            }
        }
        vars
    }

    /// Get def-use chains: for each variable, where it's defined and where it's used
    pub fn def_use_chains(&self) -> HashMap<SsaVar, DefUse> {
        let mut chains: HashMap<SsaVar, DefUse> = HashMap::new();

        // Collect definitions
        for block in &self.blocks {
            for (idx, inst) in block.instructions.iter().enumerate() {
                if let Some(var) = instruction_def(inst) {
                    chains.insert(
                        var.clone(),
                        DefUse {
                            def_block: block.id,
                            def_instruction_idx: idx,
                            uses: Vec::new(),
                        },
                    );
                }
            }
        }

        // Collect uses
        for block in &self.blocks {
            for (idx, inst) in block.instructions.iter().enumerate() {
                for var in instruction_uses(inst) {
                    if let Some(du) = chains.get_mut(var) {
                        du.uses.push((block.id, idx));
                    }
                }
            }
        }

        chains
    }

    /// Iterate blocks in reverse postorder (useful for dataflow analysis)
    pub fn reverse_postorder(&self) -> Vec<BlockId> {
        let mut visited = HashSet::new();
        let mut postorder = Vec::new();
        self.dfs_postorder(self.entry_block, &mut visited, &mut postorder);
        postorder.reverse();
        postorder
    }

    fn dfs_postorder(
        &self,
        block_id: BlockId,
        visited: &mut HashSet<BlockId>,
        postorder: &mut Vec<BlockId>,
    ) {
        if !visited.insert(block_id) {
            return;
        }
        if let Some(block) = self.blocks.get(block_id) {
            for &succ in &block.successors {
                self.dfs_postorder(succ, visited, postorder);
            }
            postorder.push(block_id);
        }
    }
}

/// Extract the defined variable from an instruction (if any)
fn instruction_def(inst: &Instruction) -> Option<&SsaVar> {
    match inst {
        Instruction::Assign { dest, .. }
        | Instruction::BinaryOp { dest, .. }
        | Instruction::UnaryOp { dest, .. }
        | Instruction::Phi { dest, .. }
        | Instruction::StorageLoad { dest, .. }
        | Instruction::AddrValidate { dest, .. }
        | Instruction::ResultUnwrap { dest, .. } => Some(dest),
        Instruction::Call { dest, .. } | Instruction::MethodCall { dest, .. } => dest.as_ref(),
        _ => None,
    }
}

/// Extract all used variables from an instruction
fn instruction_uses(inst: &Instruction) -> Vec<&SsaVar> {
    let mut uses = Vec::new();
    match inst {
        Instruction::Assign { value, .. } => collect_operand_vars(value, &mut uses),
        Instruction::BinaryOp { left, right, .. } => {
            collect_operand_vars(left, &mut uses);
            collect_operand_vars(right, &mut uses);
        }
        Instruction::UnaryOp { operand, .. } => collect_operand_vars(operand, &mut uses),
        Instruction::Phi { sources, .. } => {
            for (var, _) in sources {
                uses.push(var);
            }
        }
        Instruction::Call { args, .. } => {
            for arg in args {
                collect_operand_vars(arg, &mut uses);
            }
        }
        Instruction::MethodCall { receiver, args, .. } => {
            collect_operand_vars(receiver, &mut uses);
            for arg in args {
                collect_operand_vars(arg, &mut uses);
            }
        }
        Instruction::StorageLoad { key, .. } => {
            if let Some(k) = key {
                collect_operand_vars(k, &mut uses);
            }
        }
        Instruction::StorageStore { key, value, .. } => {
            if let Some(k) = key {
                collect_operand_vars(k, &mut uses);
            }
            collect_operand_vars(value, &mut uses);
        }
        Instruction::AddrValidate { address, .. } => {
            collect_operand_vars(address, &mut uses);
        }
        Instruction::Branch { condition, .. } => collect_operand_vars(condition, &mut uses),
        Instruction::Return { value } => {
            if let Some(v) = value {
                collect_operand_vars(v, &mut uses);
            }
        }
        Instruction::ResultUnwrap { value, .. } | Instruction::ErrorReturn { error: value } => {
            collect_operand_vars(value, &mut uses);
        }
        Instruction::CheckSender {
            sender_var,
            expected,
        } => {
            collect_operand_vars(sender_var, &mut uses);
            collect_operand_vars(expected, &mut uses);
        }
        Instruction::SendMsg { fields, .. } => {
            for (_, op) in fields {
                collect_operand_vars(op, &mut uses);
            }
        }
        Instruction::Jump { .. } => {}
    }
    uses
}

fn collect_operand_vars<'a>(operand: &'a Operand, vars: &mut Vec<&'a SsaVar>) {
    match operand {
        Operand::Var(v) => vars.push(v),
        Operand::FieldAccess { base, .. } => collect_operand_vars(base, vars),
        Operand::Literal(_) => {}
    }
}
