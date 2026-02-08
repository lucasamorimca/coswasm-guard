use std::collections::HashMap;

use crate::ast::{ContractInfo, FunctionInfo};

use super::cfg::{BlockId, Cfg};
use super::instruction::*;
use super::types::{ContractIr, FunctionIr};

/// Classifies a path expression to avoid creating phantom SSA vars
/// for enum variants, type paths, and qualified paths.
#[derive(Debug, PartialEq)]
enum PathKind {
    /// A known local variable reference
    Variable,
    /// A type path or enum variant (PascalCase, multi-segment, etc.)
    TypeOrVariant,
}

/// Classify a path expression based on scope and naming conventions.
/// Multi-segment paths (e.g. `Foo::Bar`) are always type/variant.
/// Single-segment PascalCase identifiers (e.g. `Response`) are type/variant.
/// Single-segment identifiers found in the current scope are variables.
fn classify_path(path: &syn::ExprPath, known_vars: &HashMap<String, u32>) -> PathKind {
    if path.path.segments.len() > 1 {
        return PathKind::TypeOrVariant;
    }
    let ident = path.path.segments[0].ident.to_string();
    // Known variable in scope — always a variable
    if known_vars.contains_key(&ident) {
        return PathKind::Variable;
    }
    // SCREAMING_SNAKE_CASE (e.g. MAX_LIMIT, CONFIG) — treat as variable
    // (Rust constants are effectively variable references in expressions)
    if ident.chars().all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()) {
        return PathKind::Variable;
    }
    // PascalCase heuristic: starts with uppercase = type or enum variant
    if ident.starts_with(|c: char| c.is_ascii_uppercase()) {
        return PathKind::TypeOrVariant;
    }
    // Unknown lowercase identifier — treat as variable (could be a parameter
    // not yet lowered, or an external name)
    PathKind::Variable
}

/// Transforms syn AST function bodies into SSA-form IR
pub struct IrBuilder {
    current_block: BlockId,
    cfg: Cfg,
    var_counter: HashMap<String, u32>,
    temp_counter: u32,
}

impl IrBuilder {
    fn new(function_name: &str) -> Self {
        let mut cfg = Cfg::new(function_name.to_string());
        let entry = cfg.add_block();
        Self {
            current_block: entry,
            cfg,
            var_counter: HashMap::new(),
            temp_counter: 0,
        }
    }

    /// Build IR for the entire contract
    pub fn build_contract(contract: &ContractInfo) -> ContractIr {
        let mut ir = ContractIr::new();
        let entry_point_names: Vec<String> = contract
            .entry_points
            .iter()
            .map(|ep| ep.name.clone())
            .collect();
        ir.entry_points = entry_point_names.clone();

        for func in &contract.functions {
            if let Some(body) = &func.body {
                let func_ir =
                    Self::build_function(func, body, entry_point_names.contains(&func.name));
                ir.functions.push(func_ir);
            }
        }

        ir
    }

    /// Build IR for a single function from its syn::Block
    pub fn build_function(
        func: &FunctionInfo,
        body: &syn::Block,
        is_entry_point: bool,
    ) -> FunctionIr {
        let mut builder = IrBuilder::new(&func.name);

        // Create SSA vars for parameters
        let params: Vec<SsaVar> = func
            .params
            .iter()
            .map(|p| builder.new_ssa_var(&p.name))
            .collect();

        // Lower each statement in the function body
        for stmt in &body.stmts {
            builder.lower_stmt(stmt);
        }

        // Mark exit blocks (blocks ending with Return or no successors)
        let exit_blocks: Vec<BlockId> = builder
            .cfg
            .blocks
            .iter()
            .filter(|b| {
                b.successors.is_empty()
                    || b.instructions
                        .iter()
                        .any(|i| matches!(i, Instruction::Return { .. }))
            })
            .map(|b| b.id)
            .collect();
        builder.cfg.exit_blocks = exit_blocks;

        FunctionIr {
            name: func.name.clone(),
            params,
            cfg: builder.cfg,
            is_entry_point,
            source_span: func.span.clone(),
        }
    }

    /// Create a new SSA variable with incremented version
    fn new_ssa_var(&mut self, name: &str) -> SsaVar {
        let version = self.var_counter.entry(name.to_string()).or_insert(0);
        let var = SsaVar {
            name: name.to_string(),
            version: *version,
        };
        *version += 1;
        var
    }

    /// Create a temporary SSA variable
    fn new_temp(&mut self) -> SsaVar {
        let name = format!("_t{}", self.temp_counter);
        self.temp_counter += 1;
        self.new_ssa_var(&name)
    }

    /// Create a new basic block and return its ID
    fn new_block(&mut self) -> BlockId {
        self.cfg.add_block()
    }

    /// Emit an instruction to the current block
    fn emit(&mut self, inst: Instruction) {
        self.cfg.blocks[self.current_block].instructions.push(inst);
    }

    /// Lower a syn statement to IR instructions
    fn lower_stmt(&mut self, stmt: &syn::Stmt) {
        match stmt {
            syn::Stmt::Local(local) => self.lower_local(local),
            syn::Stmt::Expr(expr, _) => {
                self.lower_expr(expr);
            }
            syn::Stmt::Item(_) => {} // Items inside function bodies are rare, skip
            syn::Stmt::Macro(mac) => self.lower_macro_stmt(mac),
        }
    }

    /// Lower a let binding
    fn lower_local(&mut self, local: &syn::Local) {
        let var_name = if let syn::Pat::Ident(ident) = &local.pat {
            ident.ident.to_string()
        } else {
            format!("_pat{}", self.temp_counter)
        };

        let dest = self.new_ssa_var(&var_name);

        if let Some(init) = &local.init {
            let value = self.lower_expr(&init.expr);
            self.emit(Instruction::Assign { dest, value });
        }
    }

    /// Lower an expression, returning the operand representing its value
    fn lower_expr(&mut self, expr: &syn::Expr) -> Operand {
        match expr {
            syn::Expr::Lit(lit) => self.lower_lit(lit),
            syn::Expr::Path(path) => self.lower_path(path),
            syn::Expr::Binary(bin) => self.lower_binary(bin),
            syn::Expr::Unary(un) => self.lower_unary(un),
            syn::Expr::MethodCall(mc) => self.lower_method_call(mc),
            syn::Expr::Call(call) => self.lower_call(call),
            syn::Expr::Field(field) => self.lower_field(field),
            syn::Expr::If(if_expr) => self.lower_if(if_expr),
            syn::Expr::Match(match_expr) => self.lower_match(match_expr),
            syn::Expr::Block(block) => self.lower_block_expr(block),
            syn::Expr::Return(ret) => self.lower_return(ret),
            syn::Expr::Try(try_expr) => self.lower_try(try_expr),
            syn::Expr::Reference(ref_expr) => self.lower_expr(&ref_expr.expr),
            syn::Expr::Paren(paren) => self.lower_expr(&paren.expr),
            _ => {
                // For unhandled expressions, emit a generic opaque operand
                let temp = self.new_temp();
                self.emit(Instruction::Assign {
                    dest: temp.clone(),
                    value: Operand::Literal(LiteralValue::Unit),
                });
                Operand::Var(temp)
            }
        }
    }

    fn lower_lit(&mut self, lit: &syn::ExprLit) -> Operand {
        match &lit.lit {
            syn::Lit::Str(s) => Operand::Literal(LiteralValue::String(s.value())),
            syn::Lit::Int(i) => {
                if let Ok(v) = i.base10_parse::<u128>() {
                    Operand::Literal(LiteralValue::Uint(v))
                } else if let Ok(v) = i.base10_parse::<i128>() {
                    Operand::Literal(LiteralValue::Int(v))
                } else {
                    Operand::Literal(LiteralValue::Unit)
                }
            }
            syn::Lit::Bool(b) => Operand::Literal(LiteralValue::Bool(b.value)),
            _ => Operand::Literal(LiteralValue::Unit),
        }
    }

    fn lower_path(&mut self, path: &syn::ExprPath) -> Operand {
        match classify_path(path, &self.var_counter) {
            PathKind::TypeOrVariant => {
                // Enum variants and type paths produce a literal marker,
                // not an SSA variable, to avoid polluting def-use chains.
                let name = path
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::");
                Operand::Literal(LiteralValue::String(name))
            }
            PathKind::Variable => {
                let ident = path.path.segments[0].ident.to_string();
                if let Some(&version) = self.var_counter.get(&ident) {
                    Operand::Var(SsaVar {
                        name: ident,
                        version: version.saturating_sub(1),
                    })
                } else {
                    // Unknown variable — create fresh SSA var
                    let var = self.new_ssa_var(&ident);
                    Operand::Var(var)
                }
            }
        }
    }

    fn lower_binary(&mut self, bin: &syn::ExprBinary) -> Operand {
        let left = self.lower_expr(&bin.left);
        let right = self.lower_expr(&bin.right);
        let op = match bin.op {
            syn::BinOp::Add(_) | syn::BinOp::AddAssign(_) => BinaryOp::Add,
            syn::BinOp::Sub(_) | syn::BinOp::SubAssign(_) => BinaryOp::Sub,
            syn::BinOp::Mul(_) | syn::BinOp::MulAssign(_) => BinaryOp::Mul,
            syn::BinOp::Div(_) | syn::BinOp::DivAssign(_) => BinaryOp::Div,
            syn::BinOp::Rem(_) | syn::BinOp::RemAssign(_) => BinaryOp::Mod,
            syn::BinOp::Eq(_) => BinaryOp::Eq,
            syn::BinOp::Ne(_) => BinaryOp::Ne,
            syn::BinOp::Lt(_) => BinaryOp::Lt,
            syn::BinOp::Le(_) => BinaryOp::Le,
            syn::BinOp::Gt(_) => BinaryOp::Gt,
            syn::BinOp::Ge(_) => BinaryOp::Ge,
            syn::BinOp::And(_) => BinaryOp::And,
            syn::BinOp::Or(_) => BinaryOp::Or,
            syn::BinOp::BitAnd(_) | syn::BinOp::BitAndAssign(_) => BinaryOp::BitAnd,
            syn::BinOp::BitOr(_) | syn::BinOp::BitOrAssign(_) => BinaryOp::BitOr,
            syn::BinOp::BitXor(_) | syn::BinOp::BitXorAssign(_) => BinaryOp::BitXor,
            syn::BinOp::Shl(_) | syn::BinOp::ShlAssign(_) => BinaryOp::Shl,
            syn::BinOp::Shr(_) | syn::BinOp::ShrAssign(_) => BinaryOp::Shr,
            _ => BinaryOp::Unknown,
        };

        let dest = self.new_temp();
        self.emit(Instruction::BinaryOp {
            dest: dest.clone(),
            op,
            left,
            right,
        });
        Operand::Var(dest)
    }

    fn lower_unary(&mut self, un: &syn::ExprUnary) -> Operand {
        let operand = self.lower_expr(&un.expr);
        let op = match un.op {
            syn::UnOp::Not(_) => UnaryOp::Not,
            syn::UnOp::Neg(_) => UnaryOp::Neg,
            syn::UnOp::Deref(_) => UnaryOp::Deref,
            _ => UnaryOp::Unknown,
        };
        let dest = self.new_temp();
        self.emit(Instruction::UnaryOp {
            dest: dest.clone(),
            op,
            operand,
        });
        Operand::Var(dest)
    }

    fn lower_method_call(&mut self, mc: &syn::ExprMethodCall) -> Operand {
        let receiver = self.lower_expr(&mc.receiver);
        let method = mc.method.to_string();
        let args: Vec<Operand> = mc.args.iter().map(|a| self.lower_expr(a)).collect();

        // Recognize CosmWasm-specific patterns
        if method == "addr_validate" || method == "addr_canonicalize" {
            let dest = self.new_temp();
            let address = args
                .into_iter()
                .next()
                .unwrap_or(Operand::Literal(LiteralValue::Unit));
            self.emit(Instruction::AddrValidate {
                dest: dest.clone(),
                address,
            });
            return Operand::Var(dest);
        }

        if method == "save" || method == "update" {
            // Storage store pattern: ITEM.save(storage, &value) or MAP.save(storage, key, &value)
            if let Operand::Var(ref recv_var) = receiver {
                let storage_item = recv_var.name.clone();
                // args[0] = storage, args[1..] = key + value
                let (key, value) = if args.len() >= 3 {
                    (Some(args[1].clone()), args[2].clone())
                } else if args.len() >= 2 {
                    (None, args[1].clone())
                } else {
                    (None, Operand::Literal(LiteralValue::Unit))
                };
                self.emit(Instruction::StorageStore {
                    storage_item,
                    key,
                    value,
                });
                return Operand::Literal(LiteralValue::Unit);
            }
        }

        if method == "load" || method == "may_load" {
            // Storage load pattern
            if let Operand::Var(ref recv_var) = receiver {
                let dest = self.new_temp();
                let key = args.get(1).cloned();
                self.emit(Instruction::StorageLoad {
                    dest: dest.clone(),
                    storage_item: recv_var.name.clone(),
                    key,
                });
                return Operand::Var(dest);
            }
        }

        if method == "range" || method == "range_raw" {
            // Emit as a method call so detectors can find it
            let dest = self.new_temp();
            self.emit(Instruction::MethodCall {
                dest: Some(dest.clone()),
                receiver,
                method,
                args,
            });
            return Operand::Var(dest);
        }

        // Generic method call
        let dest = self.new_temp();
        self.emit(Instruction::MethodCall {
            dest: Some(dest.clone()),
            receiver,
            method,
            args,
        });
        Operand::Var(dest)
    }

    fn lower_call(&mut self, call: &syn::ExprCall) -> Operand {
        let func_name = if let syn::Expr::Path(path) = call.func.as_ref() {
            path.path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::")
        } else {
            "unknown".to_string()
        };

        let args: Vec<Operand> = call.args.iter().map(|a| self.lower_expr(a)).collect();
        let dest = self.new_temp();

        self.emit(Instruction::Call {
            dest: Some(dest.clone()),
            func: func_name,
            args,
        });
        Operand::Var(dest)
    }

    fn lower_field(&mut self, field: &syn::ExprField) -> Operand {
        let base = self.lower_expr(&field.base);
        let field_name = match &field.member {
            syn::Member::Named(ident) => ident.to_string(),
            syn::Member::Unnamed(idx) => format!("_{}", idx.index),
        };

        Operand::FieldAccess {
            base: Box::new(base),
            field: field_name,
        }
    }

    fn lower_if(&mut self, if_expr: &syn::ExprIf) -> Operand {
        let condition = self.lower_expr(&if_expr.cond);

        let then_block = self.new_block();
        let else_block = self.new_block();
        let merge_block = self.new_block();

        self.emit(Instruction::Branch {
            condition,
            true_block: then_block,
            false_block: else_block,
        });
        self.cfg.add_edge(self.current_block, then_block);
        self.cfg.add_edge(self.current_block, else_block);

        // Then branch
        self.current_block = then_block;
        for stmt in &if_expr.then_branch.stmts {
            self.lower_stmt(stmt);
        }
        self.emit(Instruction::Jump {
            target: merge_block,
        });
        self.cfg.add_edge(self.current_block, merge_block);

        // Else branch
        self.current_block = else_block;
        if let Some((_, else_expr)) = &if_expr.else_branch {
            self.lower_expr(else_expr);
        }
        self.emit(Instruction::Jump {
            target: merge_block,
        });
        self.cfg.add_edge(self.current_block, merge_block);

        self.current_block = merge_block;
        Operand::Literal(LiteralValue::Unit)
    }

    fn lower_match(&mut self, match_expr: &syn::ExprMatch) -> Operand {
        let _scrutinee = self.lower_expr(&match_expr.expr);
        let entry_block = self.current_block;
        let merge_block = self.new_block();

        for arm in &match_expr.arms {
            let arm_block = self.new_block();
            self.cfg.add_edge(entry_block, arm_block);

            self.current_block = arm_block;
            self.lower_expr(&arm.body);
            self.emit(Instruction::Jump {
                target: merge_block,
            });
            self.cfg.add_edge(self.current_block, merge_block);
        }

        // Emit a Jump in the entry block to the merge block as a terminator.
        // The actual dispatch is opaque (match semantics); edges to arm blocks
        // already model the possible control flow paths.
        self.current_block = entry_block;
        self.emit(Instruction::Jump {
            target: merge_block,
        });

        self.current_block = merge_block;
        Operand::Literal(LiteralValue::Unit)
    }

    fn lower_block_expr(&mut self, block: &syn::ExprBlock) -> Operand {
        let mut last = Operand::Literal(LiteralValue::Unit);
        for stmt in &block.block.stmts {
            match stmt {
                syn::Stmt::Expr(expr, None) => {
                    last = self.lower_expr(expr);
                }
                _ => {
                    self.lower_stmt(stmt);
                    last = Operand::Literal(LiteralValue::Unit);
                }
            }
        }
        last
    }

    fn lower_return(&mut self, ret: &syn::ExprReturn) -> Operand {
        let value = ret.expr.as_ref().map(|e| self.lower_expr(e));
        self.emit(Instruction::Return { value });
        Operand::Literal(LiteralValue::Unit)
    }

    fn lower_try(&mut self, try_expr: &syn::ExprTry) -> Operand {
        let value = self.lower_expr(&try_expr.expr);
        let dest = self.new_temp();
        self.emit(Instruction::ResultUnwrap {
            dest: dest.clone(),
            value,
        });
        Operand::Var(dest)
    }

    fn lower_macro_stmt(&mut self, mac: &syn::StmtMacro) {
        // Recognize common macros like ensure!, bail!
        let macro_name = mac
            .mac
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();

        let dest = self.new_temp();
        self.emit(Instruction::Call {
            dest: Some(dest),
            func: format!("macro!{macro_name}"),
            args: Vec::new(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{parse_source, ContractVisitor};
    use std::path::PathBuf;

    fn build_ir(source: &str) -> ContractIr {
        let ast = parse_source(source).unwrap();
        let contract = ContractVisitor::extract(PathBuf::from("test.rs"), ast);
        IrBuilder::build_contract(&contract)
    }

    #[test]
    fn test_simple_function_ir() {
        let source = r#"
            fn hello() -> u32 {
                let x = 42;
                x
            }
        "#;
        let ir = build_ir(source);
        assert_eq!(ir.functions.len(), 1);
        assert_eq!(ir.functions[0].name, "hello");
        assert!(!ir.functions[0].cfg.blocks.is_empty());
    }

    #[test]
    fn test_if_else_creates_branches() {
        let source = r#"
            fn check(x: bool) -> u32 {
                if x { 1 } else { 2 }
            }
        "#;
        let ir = build_ir(source);
        let func = &ir.functions[0];
        // Should have entry + then + else + merge blocks
        assert!(func.cfg.blocks.len() >= 4);
    }

    #[test]
    fn test_match_creates_branches() {
        let source = r#"
            fn dispatch(x: u32) {
                match x {
                    1 => {},
                    2 => {},
                    _ => {},
                }
            }
        "#;
        let ir = build_ir(source);
        let func = &ir.functions[0];
        // Entry + 3 arms + merge = at least 5 blocks
        assert!(func.cfg.blocks.len() >= 5);
    }

    #[test]
    fn test_entry_point_detected() {
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> Result<Response, ContractError> {
                Ok(Response::new())
            }
        "#;
        let ir = build_ir(source);
        assert!(ir.functions[0].is_entry_point);
        assert!(ir.entry_points.contains(&"execute".to_string()));
    }

    #[test]
    fn test_addr_validate_recognized() {
        let source = r#"
            fn validate(deps: DepsMut) {
                let addr = deps.api.addr_validate("someone");
            }
        "#;
        let ir = build_ir(source);
        let func = &ir.functions[0];
        let has_addr_validate = func.cfg.blocks.iter().any(|b| {
            b.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddrValidate { .. }))
        });
        assert!(has_addr_validate);
    }

    // --- H1 regression: enum variants and type paths should NOT create SSA vars ---

    #[test]
    fn test_h1_enum_variant_not_ssa_var() {
        // Enum variant paths like Response::new() should not pollute def-use chains
        let source = r#"
            fn make_response() {
                let x = Response::new();
            }
        "#;
        let ir = build_ir(source);
        let func = &ir.functions[0];
        // Should NOT have an SSA var named "Response::new"
        let has_phantom = func.cfg.blocks.iter().any(|b| {
            b.instructions.iter().any(|i| match i {
                Instruction::Assign { dest, .. } => dest.name.contains("Response"),
                _ => false,
            })
        });
        assert!(!has_phantom, "H1: enum variant path created phantom SSA var");
    }

    #[test]
    fn test_h1_local_var_still_works() {
        // Known local variables should still be tracked as SSA vars
        let source = r#"
            fn use_var() {
                let count = 5;
                let result = count;
            }
        "#;
        let ir = build_ir(source);
        let func = &ir.functions[0];
        // 'count' should be an SSA var used in the assignment to 'result'
        let has_count_var = func.cfg.blocks.iter().any(|b| {
            b.instructions.iter().any(|i| match i {
                Instruction::Assign { value: Operand::Var(v), .. } => v.name == "count",
                _ => false,
            })
        });
        assert!(has_count_var, "H1: local variable should still be an SSA var");
    }
}
