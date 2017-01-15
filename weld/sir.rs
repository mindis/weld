//! Sequential IR for Weld programs

use std::fmt;
use std::collections::{BTreeMap, HashMap, HashSet};

use super::ast::*;
use super::ast::ExprKind::*;
use super::error::*;
use super::pretty_print::*;
use super::util::SymbolGenerator;

pub type BasicBlockId = usize;
pub type FunctionId = usize;

/// A non-terminating statement inside a basic block.
#[derive(Clone)]
pub enum Statement {
    /// output, op, type, left, right
    AssignBinOp(Symbol, BinOpKind, Type, Symbol, Symbol),
    /// output, value
    Assign(Symbol, Symbol),
    /// output, value
    AssignLiteral(Symbol, LiteralKind),
    /// builder, value
    DoMerge(Symbol, Symbol),
    /// output, builder
    GetResult(Symbol, Symbol),
    /// output, builder type
    CreateBuilder(Symbol, Type),
}

#[derive(Clone)]
pub struct ParallelForData {
    pub data: Symbol,
    pub builder: Symbol,
    pub data_arg: Symbol,
    pub builder_arg: Symbol,
    pub body: FunctionId,
    pub cont: FunctionId
}

/// A terminating statement inside a basic block.
#[derive(Clone)]
pub enum Terminator {
    /// condition, on_true, on_false
    Branch(Symbol, BasicBlockId, BasicBlockId),
    JumpBlock(BasicBlockId),
    JumpFunction(FunctionId),
    ProgramReturn(Symbol),
    EndFunction,
    ParallelFor(ParallelForData),
    Crash
}

/// A basic block inside a SIR program
#[derive(Clone)]
pub struct BasicBlock {
    pub id: BasicBlockId,
    pub statements: Vec<Statement>,
    pub terminator: Terminator
}

pub struct SirFunction {
    pub id: FunctionId,
    pub params: HashMap<Symbol, Type>,
    pub locals: HashMap<Symbol, Type>,
    pub blocks: Vec<BasicBlock>
}

pub struct SirProgram {
    /// funcs[0] is the main function
    pub funcs: Vec<SirFunction>,
    pub ret_ty: Type,
    pub top_params: Vec<TypedParameter>,
    sym_gen: SymbolGenerator
}

impl SirProgram {
    pub fn new(ret_ty: &Type, top_params: &Vec<TypedParameter>) -> SirProgram {
        let mut prog = SirProgram {
            funcs: vec![],
            ret_ty: ret_ty.clone(),
            top_params: top_params.clone(),
            sym_gen: SymbolGenerator::new()
        };
        /// add main
        prog.add_func();
        prog
    }

    pub fn add_func(&mut self) -> FunctionId {
        let func = SirFunction {
            id: self.funcs.len(),
            params: HashMap::new(),
            blocks: vec![],
            locals: HashMap::new()
        };
        self.funcs.push(func);
        self.funcs.len() - 1       
    }

    /// Add a local variable of the given type and return a symbol for it.
    pub fn add_local(&mut self, ty: &Type, func: FunctionId) -> Symbol {
        let sym = self.sym_gen.new_symbol(format!("fn{}_tmp", func).as_str());
        self.funcs[func].locals.insert(sym.clone(), ty.clone());
        sym
    }

    /// Add a local variable of the given type and name
    pub fn add_local_named(&mut self, ty: &Type, sym: &Symbol, func: FunctionId) {
        self.funcs[func].locals.insert(sym.clone(), ty.clone());
    }
}

impl SirFunction {
    /// Add a new basic block and return its block ID.
    pub fn add_block(&mut self) -> BasicBlockId {
        let block = BasicBlock {
            id: self.blocks.len(),
            statements: vec![],
            terminator: Terminator::Crash
        };
        self.blocks.push(block);
        self.blocks.len() - 1
    }
}

impl BasicBlock {
    pub fn add_statement(&mut self, statement: Statement) {
        self.statements.push(statement);
    }
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Statement::*;
        match *self {
            AssignBinOp(ref out, ref op, ref ty, ref left, ref right) => {
                write!(f, "{} = {} {} {} {}", out, op, print_type(ty), left, right)
            },
            Assign(ref out, ref value) => write!(f, "{} = {}", out, value),
            AssignLiteral(ref out, ref lit) => write!(f, "{} = {}", out, print_literal(lit)),
            DoMerge(ref bld, ref elem) => write!(f, "merge {} {}", bld, elem),
            GetResult(ref out, ref value) => write!(f, "{} = result {}", out, value),
            CreateBuilder(ref out, ref ty) => write!(f, "{} = new {}", out, print_type(ty)),
        }
    }
}

impl fmt::Display for Terminator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Terminator::*;
        match *self {
            Branch(ref cond, ref on_true, ref on_false) => {
                write!(f, "branch {} B{} B{}", cond, on_true, on_false)
            },
            ParallelFor(ref pf) => {
                write!(f, "for {} {} {} {} F{} F{}",
                    pf.data, pf.builder, pf.data_arg, pf.builder_arg, pf.body, pf.cont)?;
                Ok(())
            },
            JumpBlock(block) => write!(f, "jump B{}", block),
            JumpFunction(func) => write!(f, "jump F{}", func),
            ProgramReturn(ref sym) => write!(f, "return {}", sym),
            EndFunction => write!(f, "end"),
            Crash => write!(f, "crash")
        }
    }
}

impl fmt::Display for BasicBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "B{}:\n", self.id)?;
        for stmt in &self.statements {
            write!(f, "  {}\n", stmt)?;
        }
        write!(f, "  {}\n", self.terminator)?;
        Ok(())
    }
}

impl fmt::Display for SirFunction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "F{}:\n", self.id)?;
        write!(f, "Params:\n")?;
        let params_sorted: BTreeMap<&Symbol, &Type> = self.params.iter().collect();
        for (name, ty) in params_sorted {
            write!(f, "  {}: {}\n", name, print_type(ty))?;
        }
        write!(f, "Locals:\n")?;
        let locals_sorted: BTreeMap<&Symbol, &Type> = self.locals.iter().collect();
        for (name, ty) in locals_sorted {
            write!(f, "  {}: {}\n", name, print_type(ty))?;
        }
        for block in &self.blocks {
            write!(f, "{}", block)?;
        }
        Ok(())
    }
}

impl fmt::Display for SirProgram {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for func in &self.funcs {
            write!(f, "{}\n", func)?;
        }
        Ok(())
    }
}

/// Recursive helper function for sir_param_correction. env contains the symbol to type mappings
/// that have been defined previously in the program. Any symbols that need to be passed in
/// as closure parameters to func_id will be added to closure (so that func_id's
/// callers can also add these symbols to their parameters list, if necessary).
fn sir_param_correction_helper(prog: &mut SirProgram, func_id: FunctionId,
env: &mut HashMap<Symbol, Type>, closure: &mut HashSet<Symbol>) {
    for (name, ty) in &prog.funcs[func_id].params {
        env.insert(name.clone(), ty.clone());
    }
    for (name, ty) in &prog.funcs[func_id].locals {
        env.insert(name.clone(), ty.clone());
    }
    // All symbols are unique, so there is no need to remove stuff from env at any point.
    for block in prog.funcs[func_id].blocks.clone() {
        let mut vars = vec![];
        for statement in &block.statements {
            use self::Statement::*;
            match *statement {
                AssignBinOp(_, _, _, ref left, ref right) => {
                    vars.push(left.clone());
                    vars.push(right.clone());
                },
                Assign(_, ref value) => vars.push(value.clone()),
                DoMerge(ref bld, ref elem) => {
                    vars.push(bld.clone());
                    vars.push(elem.clone());
                },
                GetResult(_, ref value) => vars.push(value.clone()),
                _ => {}
            }   
        }
        for var in &vars {
            if prog.funcs[func_id].locals.get(&var) == None {
                prog.funcs[func_id].params.insert(var.clone(), env.get(&var).unwrap().clone());
                closure.insert(var.clone());
            }
        }
        let mut inner_closure = HashSet::new();
        use self::Terminator::*;
        match block.terminator {
            // TODO how do we get rid of unused variable warnings here?
            ParallelFor(ref pf) => {
                sir_param_correction_helper(prog, pf.body, env, &mut inner_closure);
                sir_param_correction_helper(prog, pf.cont, env, &mut inner_closure);
            },
            JumpFunction(jump_func) => {
                sir_param_correction_helper(prog, jump_func, env, &mut inner_closure);
            },
            _ => {}       
        }
        for var in inner_closure {
            if prog.funcs[func_id].locals.get(&var) == None {
                prog.funcs[func_id].params.insert(var.clone(), env.get(&var).unwrap().clone());
                closure.insert(var.clone());
            }
        }
    }
}

/// gen_expr may result in the use of symbols across function boundaries,
/// so ast_to_sir calls sir_param_correction to correct function parameters
/// to ensure that such symbols (the closure) are passed in as parameters.
fn sir_param_correction(prog: &mut SirProgram) -> WeldResult<()> {
    let mut env = HashMap::new();
    let mut closure = HashSet::new();
    sir_param_correction_helper(prog, 0, &mut env, &mut closure);
    let ref func = prog.funcs[0];
    for name in closure {
        if func.params.get(&name) == None {
            weld_err!("Unbound symbol {}#{}", name.name, name.id)?;
        }
    }
    Ok(())
}

/// Convert an AST to a SIR program. Symbols must be unique in expr.
pub fn ast_to_sir(expr: &TypedExpr) -> WeldResult<SirProgram> {
    if let Lambda { ref params, ref body } = expr.kind {
        let mut prog = SirProgram::new(&expr.ty, params);
        prog.sym_gen = SymbolGenerator::from_expression(expr);
        for tp in params {
            prog.funcs[0].params.insert(tp.name.clone(), tp.ty.clone());
        }
        let first_block = prog.funcs[0].add_block();
        let (res_func, res_block, res_sym) = gen_expr(body, &mut prog, 0, first_block)?;
        prog.funcs[res_func].blocks[res_block].terminator = Terminator::ProgramReturn(res_sym);
        sir_param_correction(&mut prog)?;
        Ok((prog))
    } else {
        weld_err!("Expression passed to ast_to_sir was not a Lambda")
    }
}

/// Generate code to compute the expression `expr` starting at the current tail of `cur_block`,
/// possibly creating new basic blocks and functions in the process. Return the function and
/// basic block that the expression will be ready in, and its symbol therein.
fn gen_expr(
    expr: &TypedExpr,
    prog: &mut SirProgram,
    cur_func: FunctionId,
    cur_block: BasicBlockId
) -> WeldResult<(FunctionId, BasicBlockId, Symbol)> {
    use self::Statement::*;
    use self::Terminator::*;
    match expr.kind {
        Ident(ref sym) => {
            Ok((cur_func, cur_block, sym.clone()))
        },

        Literal(lit) => {
            let res_sym = prog.add_local(&expr.ty, cur_func);
            prog.funcs[cur_func].blocks[cur_block].add_statement(AssignLiteral(res_sym.clone(), lit));
            Ok((cur_func, cur_block, res_sym))
        },

        Let { ref name, ref value, ref body } => {
            let (cur_func, cur_block, val_sym) = gen_expr(value, prog, cur_func, cur_block)?;
            prog.add_local_named(&value.ty, name, cur_func);
            prog.funcs[cur_func].blocks[cur_block].add_statement(Assign(name.clone(), val_sym));
            let (cur_func, cur_block, res_sym) = gen_expr(body, prog, cur_func, cur_block)?;
            Ok((cur_func, cur_block, res_sym))
        },

        BinOp { kind, ref left, ref right } => {
            let (cur_func, cur_block, left_sym) = gen_expr(left, prog, cur_func, cur_block)?;
            let (cur_func, cur_block, right_sym) = gen_expr(right, prog, cur_func, cur_block)?;
            let res_sym = prog.add_local(&expr.ty, cur_func);
            prog.funcs[cur_func].blocks[cur_block].add_statement(
                AssignBinOp(res_sym.clone(), kind, left.ty.clone(), left_sym, right_sym));
            Ok((cur_func, cur_block, res_sym))
        },

        If { ref cond, ref on_true, ref on_false } => {
            let (cur_func, cur_block, cond_sym) = gen_expr(cond, prog, cur_func, cur_block)?;
            let true_block = prog.funcs[cur_func].add_block();
            let false_block = prog.funcs[cur_func].add_block();
            prog.funcs[cur_func].blocks[cur_block].terminator = Branch(cond_sym, true_block, false_block);
            let (true_func, true_block, true_sym) = gen_expr(on_true, prog, cur_func, true_block)?;
            let (false_func, false_block, false_sym) = gen_expr(on_false, prog, cur_func, false_block)?;
            let res_sym = prog.add_local(&expr.ty, true_func);
            prog.funcs[true_func].blocks[true_block].add_statement(Assign(res_sym.clone(), true_sym));
            prog.funcs[false_func].blocks[false_block].add_statement(Assign(res_sym.clone(), false_sym));
            if true_func != cur_func || false_func != cur_func {
                // TODO we probably want a better for name for this symbol than whatever res_sym is
                prog.add_local_named(&expr.ty, &res_sym, false_func);
                // the part after the if-else block is split out into a separate continuation
                // function so that we don't have to duplicate this code
                let cont_func = prog.add_func();
                let cont_block = prog.funcs[cont_func].add_block();
                prog.funcs[true_func].blocks[true_block].terminator = JumpFunction(cont_func);
                prog.funcs[false_func].blocks[false_block].terminator = JumpFunction(cont_func);
                Ok((cont_func, cont_block, res_sym))
            } else {
                let cont_block = prog.funcs[cur_func].add_block();
                prog.funcs[true_func].blocks[true_block].terminator = JumpBlock(cont_block);
                prog.funcs[false_func].blocks[false_block].terminator = JumpBlock(cont_block);
                Ok((cur_func, cont_block, res_sym))
            }
        },

        Merge { ref builder, ref value } => {
            let (cur_func, cur_block, builder_sym) = gen_expr(builder, prog, cur_func, cur_block)?;
            let (cur_func, cur_block, elem_sym) = gen_expr(value, prog, cur_func, cur_block)?;
            prog.funcs[cur_func].blocks[cur_block].add_statement(DoMerge(builder_sym.clone(),
                elem_sym));
            Ok((cur_func, cur_block, builder_sym))
        },

        Res { ref builder } => {
            let (cur_func, cur_block, builder_sym) = gen_expr(builder, prog, cur_func, cur_block)?;
            let res_sym = prog.add_local(&expr.ty, cur_func);
            prog.funcs[cur_func].blocks[cur_block].add_statement(GetResult(res_sym.clone(), builder_sym));
            Ok((cur_func, cur_block, res_sym))
        },

        NewBuilder => {
            let res_sym = prog.add_local(&expr.ty, cur_func);
            prog.funcs[cur_func].blocks[cur_block].add_statement(CreateBuilder(res_sym.clone(), expr.ty.clone()));
            Ok((cur_func, cur_block, res_sym))
        },

        For { ref iters, ref builder, ref func } => {
            if iters.len() != 1 || iters[0].start.is_some() || iters[0].end.is_some()
            || iters[0].stride.is_some() {
                // TODO support this
                weld_err!("Only single-array loops with null start/end/stride currently supported")?
            }
            let data: &TypedExpr = &iters[0].data;
            if let Lambda { ref params, ref body } = func.kind {
                let (cur_func, cur_block, data_sym) = gen_expr(data, prog, cur_func, cur_block)?;
                let (cur_func, cur_block, builder_sym) = gen_expr(builder, prog, cur_func, cur_block)?;
                let body_func = prog.add_func();
                let body_block = prog.funcs[body_func].add_block();
                prog.add_local_named(&params[0].ty, &params[0].name, body_func);
                prog.add_local_named(&params[1].ty, &params[1].name, body_func);
                prog.funcs[body_func].params.insert(data_sym.clone(), data.ty.clone());
                prog.funcs[body_func].params.insert(builder_sym.clone(), builder.ty.clone());
                let (body_end_func, body_end_block, _) = gen_expr(body, prog, body_func, body_block)?;
                // TODO this is a useless line
                prog.funcs[body_end_func].blocks[body_end_block].terminator = EndFunction;
                let cont_func = prog.add_func();
                let cont_block = prog.funcs[cont_func].add_block();
                prog.funcs[cur_func].blocks[cur_block].terminator = ParallelFor(
                    ParallelForData {
                        data: data_sym,
                        builder: builder_sym.clone(),
                        data_arg: params[1].name.clone(),
                        builder_arg: params[0].name.clone(),
                        body: body_func,
                        cont: cont_func
                    }
                );
                Ok((cont_func, cont_block, builder_sym))
            } else {
                weld_err!("Argument to For was not a Lambda: {}", print_expr(func))
            }
        },

        _ => weld_err!("Unsupported expression: {}", print_expr(expr))
    }
}