//! Lowers the AST to Setae bytecode.
//!
//! Only the subset the VM can run is lowered. Anything else is rejected here
//! rather than compiled into something wrong.

use ast::{BinOp, BoolOp, CmpOp, Expr, Module, Param, Stmt, UnOp};
use bytecode::{Code, Const, Instr, Op};

#[derive(Debug, Clone, PartialEq)]
pub struct CompileError {
    pub message: String,
}

pub fn compile(module: &Module) -> Result<Code, CompileError> {
    let mut c = Compiler {
        code: new_code("<module>", 0, 0, 0, 0),
        scope: None,
        next_child: 0,
    };
    for stmt in &module.body {
        c.stmt(stmt)?;
    }
    let mut code = c.code;
    assemble(&mut code);
    Ok(code)
}

fn new_code(name: &str, nlocals: u32, nparams: u32, ncells: u32, nfrees: u32) -> Code {
    Code {
        name: name.to_string(),
        consts: Vec::new(),
        names: Vec::new(),
        ops: Vec::new(),
        nlocals,
        nparams,
        ncells,
        nfrees,
        codes: Vec::new(),
    }
}

#[derive(Debug, Clone)]
struct ScopeInfo {
    locals: Vec<String>,
    cells: Vec<String>,
    frees: Vec<String>,
    children: Vec<ScopeInfo>,
}

enum Slot {
    Cell(u32),
    Free(u32),
    Local(u32),
    Global,
}

fn resolve(scope: Option<&ScopeInfo>, name: &str) -> Slot {
    let Some(s) = scope else {
        return Slot::Global;
    };
    if let Some(i) = s.cells.iter().position(|n| n == name) {
        return Slot::Cell(i as u32);
    }
    if let Some(i) = s.frees.iter().position(|n| n == name) {
        return Slot::Free(s.cells.len() as u32 + i as u32);
    }
    if let Some(i) = s.locals.iter().position(|n| n == name) {
        return Slot::Local(i as u32);
    }
    Slot::Global
}

struct Compiler {
    code: Code,
    scope: Option<ScopeInfo>,
    next_child: usize,
}

fn unsupported<T>(what: &str) -> Result<T, CompileError> {
    Err(CompileError {
        message: format!("{what} not supported yet"),
    })
}

fn add_unique(out: &mut Vec<String>, name: &str) {
    if !out.iter().any(|n| n == name) {
        out.push(name.to_string());
    }
}

fn collect_assigned(stmts: &[Stmt], out: &mut Vec<String>) {
    for s in stmts {
        match s {
            Stmt::Assign { targets, .. } => {
                for t in targets {
                    if let Expr::Name(n) = t {
                        add_unique(out, n);
                    }
                }
            }
            Stmt::AugAssign {
                target: Expr::Name(n),
                ..
            } => add_unique(out, n),
            Stmt::For {
                target,
                body,
                orelse,
                ..
            } => {
                if let Expr::Name(n) = target {
                    add_unique(out, n);
                }
                collect_assigned(body, out);
                collect_assigned(orelse, out);
            }
            Stmt::If { body, orelse, .. } | Stmt::While { body, orelse, .. } => {
                collect_assigned(body, out);
                collect_assigned(orelse, out);
            }
            Stmt::FunctionDef { name, .. } => add_unique(out, name),
            _ => {}
        }
    }
}

fn collect_nonlocals(stmts: &[Stmt], out: &mut Vec<String>) {
    for s in stmts {
        match s {
            Stmt::Nonlocal(names) => {
                for n in names {
                    add_unique(out, n);
                }
            }
            Stmt::If { body, orelse, .. } | Stmt::While { body, orelse, .. } => {
                collect_nonlocals(body, out);
                collect_nonlocals(orelse, out);
            }
            Stmt::For { body, orelse, .. } => {
                collect_nonlocals(body, out);
                collect_nonlocals(orelse, out);
            }
            _ => {}
        }
    }
}

fn expr_reads(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Name(n) => add_unique(out, n),
        Expr::Int { .. } | Expr::Float(_) | Expr::Str(_) | Expr::Bool(_) | Expr::None => {}
        Expr::List(elts) | Expr::Tuple(elts) => {
            for e in elts {
                expr_reads(e, out);
            }
        }
        Expr::Dict(pairs) => {
            for (k, v) in pairs {
                expr_reads(k, out);
                expr_reads(v, out);
            }
        }
        Expr::Unary { operand, .. } => expr_reads(operand, out),
        Expr::Bin { left, right, .. } => {
            expr_reads(left, out);
            expr_reads(right, out);
        }
        Expr::Bool_ { values, .. } => {
            for v in values {
                expr_reads(v, out);
            }
        }
        Expr::Compare {
            left, comparators, ..
        } => {
            expr_reads(left, out);
            for c in comparators {
                expr_reads(c, out);
            }
        }
        Expr::Call {
            func,
            args,
            keywords,
        } => {
            expr_reads(func, out);
            for a in args {
                expr_reads(a, out);
            }
            for k in keywords {
                expr_reads(&k.value, out);
            }
        }
        Expr::Attribute { value, .. } => expr_reads(value, out),
        Expr::Subscript { value, index } => {
            expr_reads(value, out);
            expr_reads(index, out);
        }
    }
}

fn collect_reads(stmts: &[Stmt], out: &mut Vec<String>) {
    for s in stmts {
        match s {
            Stmt::Expr(e) => expr_reads(e, out),
            Stmt::Assign { targets, value } => {
                expr_reads(value, out);
                for t in targets {
                    if !matches!(t, Expr::Name(_)) {
                        expr_reads(t, out);
                    }
                }
            }
            Stmt::AugAssign { target, value, .. } => {
                expr_reads(target, out);
                expr_reads(value, out);
            }
            Stmt::If { test, body, orelse } | Stmt::While { test, body, orelse } => {
                expr_reads(test, out);
                collect_reads(body, out);
                collect_reads(orelse, out);
            }
            Stmt::For {
                iter, body, orelse, ..
            } => {
                expr_reads(iter, out);
                collect_reads(body, out);
                collect_reads(orelse, out);
            }
            Stmt::Return(Some(e)) => expr_reads(e, out),
            _ => {}
        }
    }
}

fn for_each_def<'a>(stmts: &'a [Stmt], out: &mut Vec<(&'a [Param], &'a [Stmt])>) {
    for s in stmts {
        match s {
            Stmt::FunctionDef { params, body, .. } => out.push((params, body)),
            Stmt::If { body, orelse, .. } | Stmt::While { body, orelse, .. } => {
                for_each_def(body, out);
                for_each_def(orelse, out);
            }
            Stmt::For { body, orelse, .. } => {
                for_each_def(body, out);
                for_each_def(orelse, out);
            }
            _ => {}
        }
    }
}

fn analyze_function(
    params: &[Param],
    body: &[Stmt],
    enclosing: &[Vec<String>],
) -> Result<ScopeInfo, CompileError> {
    let mut nonlocals = Vec::new();
    collect_nonlocals(body, &mut nonlocals);

    let mut locals: Vec<String> = Vec::new();
    for p in params {
        if locals.iter().any(|n| n == &p.name) {
            return Err(CompileError {
                message: format!("duplicate parameter '{}'", p.name),
            });
        }
        locals.push(p.name.clone());
    }
    for n in &nonlocals {
        if locals.iter().any(|l| l == n) {
            return Err(CompileError {
                message: format!("name '{n}' is parameter and nonlocal"),
            });
        }
        if !enclosing.iter().any(|s| s.iter().any(|b| b == n)) {
            return Err(CompileError {
                message: format!("no binding for nonlocal '{n}' found"),
            });
        }
    }
    collect_assigned(body, &mut locals);
    locals.retain(|l| !nonlocals.iter().any(|n| n == l));

    let mut frees = nonlocals;
    let mut reads = Vec::new();
    collect_reads(body, &mut reads);
    for r in &reads {
        if locals.iter().any(|l| l == r) || frees.iter().any(|f| f == r) {
            continue;
        }
        if enclosing.iter().any(|s| s.iter().any(|b| b == r)) {
            frees.push(r.clone());
        }
    }

    let mut chain: Vec<Vec<String>> = enclosing.to_vec();
    let mut bindings = locals.clone();
    bindings.extend(frees.iter().cloned());
    chain.push(bindings);

    let mut cells: Vec<String> = Vec::new();
    let mut children = Vec::new();
    let mut defs = Vec::new();
    for_each_def(body, &mut defs);
    for (cparams, cbody) in defs {
        let ci = analyze_function(cparams, cbody, &chain)?;
        for f in &ci.frees {
            if locals.iter().any(|l| l == f) {
                add_unique(&mut cells, f);
            } else {
                add_unique(&mut frees, f);
            }
        }
        children.push(ci);
    }
    Ok(ScopeInfo {
        locals,
        cells,
        frees,
        children,
    })
}

impl Compiler {
    fn emit(&mut self, op: Op, arg: u32) {
        self.code.ops.push(Instr { op, arg });
    }

    fn const_idx(&mut self, c: Const) -> u32 {
        if let Some(i) = self.code.consts.iter().position(|existing| *existing == c) {
            return i as u32;
        }
        self.code.consts.push(c);
        (self.code.consts.len() - 1) as u32
    }

    fn name_idx(&mut self, name: &str) -> u32 {
        if let Some(i) = self.code.names.iter().position(|n| n == name) {
            return i as u32;
        }
        self.code.names.push(name.to_string());
        (self.code.names.len() - 1) as u32
    }

    fn load_const(&mut self, c: Const) {
        let i = self.const_idx(c);
        self.emit(Op::LoadConst, i);
    }

    /// Emit a jump with a placeholder target, and return its index so a later
    /// patch can fill the target in.
    fn emit_jump(&mut self, op: Op) -> usize {
        self.code.ops.push(Instr { op, arg: 0 });
        self.code.ops.len() - 1
    }

    /// Point a previously emitted jump at the current position.
    fn patch(&mut self, idx: usize) {
        self.code.ops[idx].arg = self.code.ops.len() as u32;
    }

    fn here(&self) -> u32 {
        self.code.ops.len() as u32
    }

    fn load(&mut self, name: &str) -> Result<(), CompileError> {
        match resolve(self.scope.as_ref(), name) {
            Slot::Cell(i) | Slot::Free(i) => self.emit(Op::LoadDeref, i),
            Slot::Local(i) => self.emit(Op::LoadLocal, i),
            Slot::Global => {
                let i = self.name_idx(name);
                self.emit(Op::LoadName, i);
            }
        }
        Ok(())
    }

    fn store(&mut self, name: &str) {
        match resolve(self.scope.as_ref(), name) {
            Slot::Cell(i) | Slot::Free(i) => self.emit(Op::StoreDeref, i),
            Slot::Local(i) => self.emit(Op::StoreLocal, i),
            Slot::Global => {
                let i = self.name_idx(name);
                self.emit(Op::StoreName, i);
            }
        }
    }

    fn funcdef(&mut self, name: &str, params: &[Param], body: &[Stmt]) -> Result<(), CompileError> {
        if params.iter().any(|p| p.default.is_some()) {
            return unsupported("parameter defaults");
        }
        let info = match &self.scope {
            Some(s) => {
                let ci = s.children[self.next_child].clone();
                self.next_child += 1;
                ci
            }
            None => analyze_function(params, body, &[])?,
        };
        let frees = info.frees.clone();
        let mut sub = Compiler {
            code: new_code(
                name,
                info.locals.len() as u32,
                params.len() as u32,
                info.cells.len() as u32,
                info.frees.len() as u32,
            ),
            scope: Some(info),
            next_child: 0,
        };
        let cells = sub.scope.as_ref().unwrap().cells.clone();
        for (ci, cname) in cells.iter().enumerate() {
            if let Some(pi) = params.iter().position(|p| &p.name == cname) {
                sub.emit(Op::LoadLocal, pi as u32);
                sub.emit(Op::StoreDeref, ci as u32);
            }
        }
        for s in body {
            sub.stmt(s)?;
        }
        sub.load_const(Const::None);
        sub.emit(Op::Return, 0);
        let idx = self.code.codes.len() as u32;
        self.code.codes.push(sub.code);
        for f in &frees {
            match resolve(self.scope.as_ref(), f) {
                Slot::Cell(i) | Slot::Free(i) => self.emit(Op::LoadClosure, i),
                _ => {
                    return Err(CompileError {
                        message: format!("cannot capture '{f}'"),
                    });
                }
            }
        }
        self.emit(Op::MakeFunction, idx);
        self.store(name);
        Ok(())
    }

    fn stmt(&mut self, s: &Stmt) -> Result<(), CompileError> {
        match s {
            Stmt::Expr(e) => {
                self.expr(e)?;
                self.emit(Op::PopTop, 0);
                Ok(())
            }
            Stmt::Assign { targets, value } => {
                if targets.len() != 1 {
                    return unsupported("chained assignment");
                }
                match &targets[0] {
                    Expr::Name(name) => {
                        self.expr(value)?;
                        self.store(name);
                    }
                    Expr::Subscript { value: obj, index } => {
                        self.expr(value)?;
                        self.expr(obj)?;
                        self.expr(index)?;
                        self.emit(Op::StoreSubscr, 0);
                    }
                    _ => return unsupported("this assignment target"),
                }
                Ok(())
            }
            Stmt::AugAssign { target, op, value } => {
                let Expr::Name(name) = target else {
                    return unsupported("this augmented assignment target");
                };
                self.load(name)?;
                self.expr(value)?;
                let sel = bin_selector(*op)?;
                self.emit(Op::BinaryOp, sel);
                self.store(name);
                Ok(())
            }
            Stmt::If { test, body, orelse } => {
                self.expr(test)?;
                let to_else = self.emit_jump(Op::PopJumpIfFalse);
                for s in body {
                    self.stmt(s)?;
                }
                if orelse.is_empty() {
                    self.patch(to_else);
                } else {
                    let to_end = self.emit_jump(Op::Jump);
                    self.patch(to_else);
                    for s in orelse {
                        self.stmt(s)?;
                    }
                    self.patch(to_end);
                }
                Ok(())
            }
            Stmt::While { test, body, orelse } => {
                let start = self.here();
                self.expr(test)?;
                let to_end = self.emit_jump(Op::PopJumpIfFalse);
                for s in body {
                    self.stmt(s)?;
                }
                self.emit(Op::Jump, start);
                self.patch(to_end);
                // No break yet, so the else clause always runs after the loop.
                for s in orelse {
                    self.stmt(s)?;
                }
                Ok(())
            }
            Stmt::For {
                target,
                iter,
                body,
                orelse,
            } => {
                let Expr::Name(name) = target else {
                    return unsupported("this for target");
                };
                self.expr(iter)?;
                self.emit(Op::GetIter, 0);
                let start = self.here();
                let exit = self.emit_jump(Op::ForIter);
                self.store(name);
                for s in body {
                    self.stmt(s)?;
                }
                self.emit(Op::Jump, start);
                self.patch(exit);
                // No break yet, so the else clause always runs after the loop.
                for s in orelse {
                    self.stmt(s)?;
                }
                Ok(())
            }
            Stmt::FunctionDef { name, params, body } => self.funcdef(name, params, body),
            Stmt::Return(value) => {
                if self.scope.is_none() {
                    return unsupported("return outside a function");
                }
                match value {
                    Some(e) => self.expr(e)?,
                    None => self.load_const(Const::None),
                }
                self.emit(Op::Return, 0);
                Ok(())
            }
            Stmt::Nonlocal(_) => {
                if self.scope.is_none() {
                    return Err(CompileError {
                        message: "nonlocal declaration not allowed at module level".into(),
                    });
                }
                Ok(())
            }
            Stmt::Pass => Ok(()),
            Stmt::Break | Stmt::Continue => unsupported("break and continue"),
        }
    }

    fn expr(&mut self, e: &Expr) -> Result<(), CompileError> {
        match e {
            Expr::Str(s) => self.load_const(Const::Str(s.clone())),
            Expr::Float(f) => self.load_const(Const::Float(*f)),
            Expr::Bool(b) => self.load_const(Const::Bool(*b)),
            Expr::None => self.load_const(Const::None),
            Expr::Int { digits, radix } => {
                let n = i128::from_str_radix(digits, *radix).map_err(|_| CompileError {
                    message: "invalid integer literal".into(),
                })?;
                if n < i32::MIN as i128 || n > i32::MAX as i128 {
                    return unsupported("integers outside the 32-bit range");
                }
                self.load_const(Const::Int(n as i32));
            }
            Expr::Name(name) => self.load(name)?,
            Expr::List(elts) => {
                for e in elts {
                    self.expr(e)?;
                }
                self.emit(Op::BuildList, elts.len() as u32);
            }
            Expr::Dict(pairs) => {
                for (k, v) in pairs {
                    self.expr(k)?;
                    self.expr(v)?;
                }
                self.emit(Op::BuildDict, pairs.len() as u32);
            }
            Expr::Subscript { value, index } => {
                self.expr(value)?;
                self.expr(index)?;
                self.emit(Op::Subscr, 0);
            }
            Expr::Call {
                func,
                args,
                keywords,
            } => {
                if !keywords.is_empty() {
                    return unsupported("keyword arguments");
                }
                if args.len() > 255 {
                    return unsupported("more than 255 arguments");
                }
                if let Expr::Attribute { value, attr } = func.as_ref() {
                    self.expr(value)?;
                    for a in args {
                        self.expr(a)?;
                    }
                    let name = self.name_idx(attr);
                    self.emit(Op::CallMethod, (name << 8) | args.len() as u32);
                } else {
                    self.expr(func)?;
                    for a in args {
                        self.expr(a)?;
                    }
                    self.emit(Op::Call, args.len() as u32);
                }
            }
            Expr::Bin { op, left, right } => {
                let sel = bin_selector(*op)?;
                self.expr(left)?;
                self.expr(right)?;
                self.emit(Op::BinaryOp, sel);
            }
            Expr::Compare {
                left,
                ops,
                comparators,
            } => {
                if ops.len() != 1 {
                    return unsupported("chained comparison");
                }
                let sel = match ops[0] {
                    CmpOp::Eq => bytecode::CMP_EQ,
                    CmpOp::NotEq => bytecode::CMP_NE,
                    CmpOp::Lt => bytecode::CMP_LT,
                    CmpOp::LtE => bytecode::CMP_LE,
                    CmpOp::Gt => bytecode::CMP_GT,
                    CmpOp::GtE => bytecode::CMP_GE,
                    CmpOp::In => bytecode::CMP_IN,
                    CmpOp::NotIn => bytecode::CMP_NOT_IN,
                    _ => return unsupported("this comparison"),
                };
                self.expr(left)?;
                self.expr(&comparators[0])?;
                self.emit(Op::CompareOp, sel);
            }
            Expr::Bool_ { op, values } => {
                let short = match op {
                    BoolOp::And => Op::JumpIfFalseOrPop,
                    BoolOp::Or => Op::JumpIfTrueOrPop,
                };
                self.expr(&values[0])?;
                let mut jumps = Vec::new();
                for v in &values[1..] {
                    jumps.push(self.emit_jump(short));
                    self.expr(v)?;
                }
                for j in jumps {
                    self.patch(j);
                }
            }
            Expr::Unary { op, operand } => {
                self.expr(operand)?;
                match op {
                    UnOp::Not => self.emit(Op::UnaryNot, 0),
                    UnOp::Neg => self.emit(Op::UnaryNeg, 0),
                    UnOp::Pos => {}
                    UnOp::Invert => return unsupported("the ~ operator"),
                }
            }
            Expr::Attribute { .. } => return unsupported("attribute access"),
            Expr::Tuple(_) => return unsupported("tuples"),
        }
        Ok(())
    }
}

fn bin_selector(op: BinOp) -> Result<u32, CompileError> {
    Ok(match op {
        BinOp::Add => bytecode::BIN_ADD,
        BinOp::Sub => bytecode::BIN_SUB,
        BinOp::Mul => bytecode::BIN_MUL,
        BinOp::Div => bytecode::BIN_DIV,
        BinOp::Mod => bytecode::BIN_MOD,
        BinOp::FloorDiv => bytecode::BIN_FLOORDIV,
        _ => unsupported("this operator")?,
    })
}

fn is_jump(op: Op) -> bool {
    matches!(
        op,
        Op::Jump
            | Op::PopJumpIfFalse
            | Op::PopJumpIfTrue
            | Op::JumpIfFalseOrPop
            | Op::JumpIfTrueOrPop
            | Op::ForIter
    )
}

fn ext_count(arg: u32) -> u32 {
    match arg {
        0..0x100 => 0,
        0x100..0x10000 => 1,
        0x10000..0x1000000 => 2,
        _ => 3,
    }
}

fn assemble(code: &mut Code) {
    for child in &mut code.codes {
        assemble(child);
    }
    let ops = std::mem::take(&mut code.ops);
    let n = ops.len();
    let mut width = vec![1u32; n];
    let mut offsets = vec![0u32; n + 1];
    loop {
        let mut total = 0u32;
        for i in 0..n {
            offsets[i] = total;
            total += width[i];
        }
        offsets[n] = total;
        let mut changed = false;
        for (instr, w) in ops.iter().zip(width.iter_mut()) {
            let arg = final_arg(instr, &offsets);
            let needed = 1 + ext_count(arg);
            if needed > *w {
                *w = needed;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut out = Vec::with_capacity(n);
    for instr in &ops {
        let arg = final_arg(instr, &offsets);
        let mut shift = ext_count(arg) * 8;
        while shift > 0 {
            out.push(Instr {
                op: Op::ExtendedArg,
                arg: (arg >> shift) & 0xff,
            });
            shift -= 8;
        }
        out.push(Instr {
            op: instr.op,
            arg: arg & 0xff,
        });
    }
    code.ops = out;
}

fn final_arg(i: &Instr, offsets: &[u32]) -> u32 {
    if is_jump(i.op) {
        offsets[i.arg as usize]
    } else {
        i.arg
    }
}

#[cfg(test)]
mod tests {
    use super::compile;
    use bytecode::{Const, Op};

    fn code(src: &str) -> bytecode::Code {
        compile(&parser::parse(src).unwrap()).unwrap()
    }

    fn err(src: &str) -> String {
        compile(&parser::parse(src).unwrap()).unwrap_err().message
    }

    #[test]
    fn compiles_hello_world() {
        let c = code("print(\"hello world\")\n");
        assert_eq!(c.names, vec!["print".to_string()]);
        assert_eq!(c.consts, vec![Const::Str("hello world".into())]);
        let ops: Vec<Op> = c.ops.iter().map(|i| i.op).collect();
        assert_eq!(ops, vec![Op::LoadName, Op::LoadConst, Op::Call, Op::PopTop]);
        assert_eq!(c.ops[2].arg, 1);
    }

    #[test]
    fn folds_precedence_into_bytecode() {
        let c = code("1 + 2 * 3\n");
        let bins: Vec<u32> = c
            .ops
            .iter()
            .filter(|i| i.op == Op::BinaryOp)
            .map(|i| i.arg)
            .collect();
        assert_eq!(bins, vec![bytecode::BIN_MUL, bytecode::BIN_ADD]);
    }

    #[test]
    fn assignment_stores_name() {
        let c = code("x = 41\n");
        let ops: Vec<Op> = c.ops.iter().map(|i| i.op).collect();
        assert_eq!(ops, vec![Op::LoadConst, Op::StoreName]);
        assert_eq!(c.names, vec!["x".to_string()]);
    }

    #[test]
    fn rejects_unsupported() {
        assert!(err("x, y = 1, 2\n").contains("this assignment target"));
        assert!(err("break\n").contains("break and continue"));
    }

    #[test]
    fn if_emits_conditional_jump() {
        let c = code("if 1 < 2:\n    print(1)\n");
        assert!(c.ops.iter().any(|i| i.op == Op::PopJumpIfFalse));
        assert!(c.ops.iter().any(|i| i.op == Op::CompareOp));
    }

    #[test]
    fn while_jumps_backward() {
        let c = code("while 1:\n    print(1)\n");
        // The trailing unconditional jump targets the loop test at the top.
        let back = c.ops.iter().rev().find(|i| i.op == Op::Jump).unwrap();
        assert_eq!(back.arg, 0);
    }

    #[test]
    fn function_lowers_to_child_code() {
        let c = code("def add(a, b):\n    return a + b\n");
        assert!(c.ops.iter().any(|i| i.op == Op::MakeFunction));
        assert!(c.ops.iter().any(|i| i.op == Op::StoreName));
        assert_eq!(c.codes.len(), 1);
        let f = &c.codes[0];
        assert_eq!(f.name, "add");
        assert_eq!(f.nparams, 2);
        assert_eq!(f.nlocals, 2);
        let ops: Vec<Op> = f.ops.iter().map(|i| i.op).collect();
        assert_eq!(
            ops,
            vec![
                Op::LoadLocal,
                Op::LoadLocal,
                Op::BinaryOp,
                Op::Return,
                Op::LoadConst,
                Op::Return
            ]
        );
    }

    #[test]
    fn assigned_names_become_locals() {
        let c = code("def f(x):\n    y = x * 2\n    return y\n");
        let f = &c.codes[0];
        assert_eq!(f.nlocals, 2);
        assert!(f.ops.iter().any(|i| i.op == Op::StoreLocal && i.arg == 1));
        assert!(f.names.is_empty());
    }

    #[test]
    fn globals_read_from_inside_functions() {
        let c = code("def f():\n    return g()\n");
        let f = &c.codes[0];
        assert_eq!(f.names, vec!["g".to_string()]);
        assert!(f.ops.iter().any(|i| i.op == Op::LoadName));
    }

    #[test]
    fn closures_get_cells_and_frees() {
        let src = "def f():\n    x = 1\n    def g():\n        return x\n    return g\n";
        let c = code(src);
        let f = &c.codes[0];
        assert_eq!(f.ncells, 1);
        assert_eq!(f.nfrees, 0);
        let g = &f.codes[0];
        assert_eq!(g.ncells, 0);
        assert_eq!(g.nfrees, 1);
        assert!(f.ops.iter().any(|i| i.op == Op::LoadClosure));
        assert!(g.ops.iter().any(|i| i.op == Op::LoadDeref));
    }

    #[test]
    fn nonlocal_stores_through_the_cell() {
        let src = "def outer():\n    n = 0\n    def inc():\n        nonlocal n\n        n += 1\n    inc()\n    return n\n";
        let c = code(src);
        let outer = &c.codes[0];
        assert_eq!(outer.ncells, 1);
        assert!(outer.ops.iter().any(|i| i.op == Op::StoreDeref));
        let inc = &outer.codes[0];
        assert_eq!(inc.nfrees, 1);
        assert!(inc.ops.iter().any(|i| i.op == Op::StoreDeref));
        assert_eq!(inc.nlocals, 0);
    }

    #[test]
    fn captured_param_gets_a_prologue_copy() {
        let src = "def make(y):\n    def get():\n        return y\n    return get\n";
        let c = code(src);
        let make = &c.codes[0];
        assert_eq!(make.ncells, 1);
        let ops: Vec<Op> = make.ops.iter().take(2).map(|i| i.op).collect();
        assert_eq!(ops, vec![Op::LoadLocal, Op::StoreDeref]);
    }

    #[test]
    fn transitive_capture_passes_through() {
        let src = "def a():\n    x = 1\n    def b():\n        def c():\n            return x\n        return c()\n    return b()\n";
        let c = code(src);
        let a = &c.codes[0];
        let b = &a.codes[0];
        assert_eq!(a.ncells, 1);
        assert_eq!(b.nfrees, 1);
        assert_eq!(b.codes[0].nfrees, 1);
        assert!(b.ops.iter().any(|i| i.op == Op::LoadClosure));
    }

    #[test]
    fn nonlocal_without_binding_errors() {
        assert!(err("def f():\n    nonlocal x\n    x = 1\n").contains("no binding for nonlocal"));
        assert!(err("nonlocal x\n").contains("module level"));
    }

    #[test]
    fn nonlocal_parameter_errors() {
        let src = "def f():\n    x = 1\n    def g(x):\n        nonlocal x\n    return g\n";
        assert!(err(src).contains("parameter and nonlocal"));
    }

    #[test]
    fn for_emits_iteration_ops() {
        let c = code("for i in range(3):\n    print(i)\n");
        let ops: Vec<Op> = c.ops.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Op::GetIter));
        assert!(ops.contains(&Op::ForIter));
        let exit = c.ops.iter().find(|i| i.op == Op::ForIter).unwrap();
        assert_eq!(exit.arg as usize, c.ops.len());
    }

    #[test]
    fn containers_and_subscripts_lower() {
        let c = code("d = {\"a\": 1}\nl = [1, 2]\nd[\"b\"] = l[0]\nprint(d[\"a\"])\n");
        let ops: Vec<Op> = c.ops.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Op::BuildDict));
        assert!(ops.contains(&Op::BuildList));
        assert!(ops.contains(&Op::StoreSubscr));
        assert!(ops.contains(&Op::Subscr));
    }

    #[test]
    fn method_call_packs_name_and_argc() {
        let c = code("l = []\nl.append(1)\n");
        let at = c.ops.iter().position(|i| i.op == Op::CallMethod).unwrap();
        let mut arg = c.ops[at].arg;
        let mut i = at;
        while i > 0 && c.ops[i - 1].op == Op::ExtendedArg {
            i -= 1;
            arg |= c.ops[i].arg << (8 * (at - i) as u32);
        }
        let name = &c.names[(arg >> 8) as usize];
        assert_eq!(name, "append");
        assert_eq!(arg & 0xff, 1);
    }

    #[test]
    fn wide_args_get_extended_arg_prefixes() {
        let mut src = String::new();
        for i in 0..300 {
            src.push_str(&format!("x = {}\n", i + 1000));
        }
        src.push_str("while x:\n    x = x - 1\n");
        let c = code(&src);
        assert!(c.ops.iter().any(|i| i.op == Op::ExtendedArg));
        assert!(c.ops.iter().all(|i| i.arg <= 255));
        let back = c.ops.iter().rev().find(|i| i.op == Op::Jump).unwrap();
        assert!(back.arg <= 255);
    }

    #[test]
    fn aug_assign_reuses_binary_op() {
        let c = code("x = 1\nx += 2\n");
        assert!(
            c.ops
                .iter()
                .any(|i| i.op == Op::BinaryOp && i.arg == bytecode::BIN_ADD)
        );
    }
}
