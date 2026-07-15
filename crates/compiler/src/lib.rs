use ast::{BinOp, BoolOp, CmpOp, Expr, Module, Param, Stmt, UnOp};
use bytecode::{Code, Const, Instr, Op};

#[derive(Debug, Clone, PartialEq)]
pub struct CompileError {
    pub message: String,
}

pub fn compile(module: &Module) -> Result<Code, CompileError> {
    let mut module = module.clone();
    let mut comps = 0u32;
    desugar_block(&mut module.body, &mut comps);
    let mut c = Compiler {
        code: new_code("<module>", 0, 0, 0, 0),
        scope: None,
        next_child: 0,
        loops: Vec::new(),
        finally_loops: Vec::new(),
    };
    for stmt in &module.body {
        c.stmt(stmt)?;
    }
    let mut code = c.code;
    assemble(&mut code);
    Ok(code)
}

fn desugar_block(stmts: &mut Vec<Stmt>, n: &mut u32) {
    let mut i = 0;
    while i < stmts.len() {
        match &mut stmts[i] {
            Stmt::FunctionDef { body, .. } => desugar_block(body, n),
            Stmt::If { body, orelse, .. } | Stmt::While { body, orelse, .. } => {
                desugar_block(body, n);
                desugar_block(orelse, n);
            }
            Stmt::For { body, orelse, .. } => {
                desugar_block(body, n);
                desugar_block(orelse, n);
            }
            Stmt::Try {
                body,
                handlers,
                orelse,
                finalbody,
            } => {
                desugar_block(body, n);
                for h in handlers.iter_mut() {
                    desugar_block(&mut h.body, n);
                }
                desugar_block(orelse, n);
                desugar_block(finalbody, n);
            }
            _ => {}
        }
        let mut hoisted = Vec::new();
        stmt_exprs(&mut stmts[i], &mut hoisted, n);
        let count = hoisted.len();
        for (k, h) in hoisted.into_iter().enumerate() {
            stmts.insert(i + k, h);
        }
        i += count + 1;
    }
}

fn stmt_exprs(s: &mut Stmt, hoisted: &mut Vec<Stmt>, n: &mut u32) {
    match s {
        Stmt::Expr(e) => desugar_expr(e, hoisted, n),
        Stmt::Assign { targets, value } => {
            desugar_expr(value, hoisted, n);
            for t in targets {
                desugar_expr(t, hoisted, n);
            }
        }
        Stmt::AugAssign { target, value, .. } => {
            desugar_expr(target, hoisted, n);
            desugar_expr(value, hoisted, n);
        }
        Stmt::If { test, .. } | Stmt::While { test, .. } => desugar_expr(test, hoisted, n),
        Stmt::For { iter, .. } => desugar_expr(iter, hoisted, n),
        Stmt::Return(Some(e)) => desugar_expr(e, hoisted, n),
        Stmt::Raise(Some(e)) => desugar_expr(e, hoisted, n),
        Stmt::Try { handlers, .. } => {
            for h in handlers.iter_mut() {
                if let Some(t) = &mut h.typ {
                    desugar_expr(t, hoisted, n);
                }
            }
        }
        _ => {}
    }
}

fn desugar_expr(e: &mut Expr, hoisted: &mut Vec<Stmt>, n: &mut u32) {
    match e {
        Expr::ListComp { .. } | Expr::DictComp { .. } => {
            let owned = std::mem::replace(e, Expr::None);
            *e = lower_comp(owned, hoisted, n);
        }
        Expr::List(elts) | Expr::Tuple(elts) => {
            for x in elts {
                desugar_expr(x, hoisted, n);
            }
        }
        Expr::Dict(pairs) => {
            for (k, v) in pairs {
                desugar_expr(k, hoisted, n);
                desugar_expr(v, hoisted, n);
            }
        }
        Expr::Unary { operand, .. } => desugar_expr(operand, hoisted, n),
        Expr::Bin { left, right, .. } => {
            desugar_expr(left, hoisted, n);
            desugar_expr(right, hoisted, n);
        }
        Expr::Bool_ { values, .. } => {
            for v in values {
                desugar_expr(v, hoisted, n);
            }
        }
        Expr::Compare {
            left, comparators, ..
        } => {
            desugar_expr(left, hoisted, n);
            for c in comparators {
                desugar_expr(c, hoisted, n);
            }
        }
        Expr::Call {
            func,
            args,
            keywords,
        } => {
            desugar_expr(func, hoisted, n);
            for a in args {
                desugar_expr(a, hoisted, n);
            }
            for k in keywords {
                desugar_expr(&mut k.value, hoisted, n);
            }
        }
        Expr::Attribute { value, .. } => desugar_expr(value, hoisted, n),
        Expr::Subscript { value, index } => {
            desugar_expr(value, hoisted, n);
            desugar_expr(index, hoisted, n);
        }
        _ => {}
    }
}

fn lower_comp(comp: Expr, hoisted: &mut Vec<Stmt>, n: &mut u32) -> Expr {
    *n += 1;
    let acc = || Expr::Name(".acc".into());
    let (fname, generators, innermost, empty_acc) = match comp {
        Expr::ListComp { elt, generators } => {
            let append = Stmt::Expr(Expr::Call {
                func: Box::new(Expr::Attribute {
                    value: Box::new(acc()),
                    attr: "append".into(),
                }),
                args: vec![*elt],
                keywords: Vec::new(),
            });
            (
                format!("<listcomp{}>", *n),
                generators,
                append,
                Expr::List(Vec::new()),
            )
        }
        Expr::DictComp {
            key,
            value,
            generators,
        } => {
            let setitem = Stmt::Assign {
                targets: vec![Expr::Subscript {
                    value: Box::new(acc()),
                    index: key,
                }],
                value: *value,
            };
            (
                format!("<dictcomp{}>", *n),
                generators,
                setitem,
                Expr::Dict(Vec::new()),
            )
        }
        _ => unreachable!("lower_comp on a non-comprehension"),
    };
    let mut outer_iter = Expr::None;
    let mut cur = innermost;
    for (gi, g) in generators.into_iter().enumerate().rev() {
        for cond in g.ifs.into_iter().rev() {
            cur = Stmt::If {
                test: cond,
                body: vec![cur],
                orelse: Vec::new(),
            };
        }
        let iter = if gi == 0 {
            outer_iter = g.iter;
            Expr::Name(".0".into())
        } else {
            g.iter
        };
        cur = Stmt::For {
            target: g.target,
            iter,
            body: vec![cur],
            orelse: Vec::new(),
        };
    }
    let mut body = vec![
        Stmt::Assign {
            targets: vec![acc()],
            value: empty_acc,
        },
        cur,
        Stmt::Return(Some(acc())),
    ];
    desugar_block(&mut body, n);
    hoisted.push(Stmt::FunctionDef {
        name: fname.clone(),
        params: vec![Param {
            name: ".0".into(),
            default: None,
        }],
        body,
    });
    desugar_expr(&mut outer_iter, hoisted, n);
    Expr::Call {
        func: Box::new(Expr::Name(fname)),
        args: vec![outer_iter],
        keywords: Vec::new(),
    }
}

fn new_code(name: &str, nlocals: u32, nparams: u32, ncells: u32, nfrees: u32) -> Code {
    Code {
        name: name.to_string(),
        consts: Vec::new(),
        names: Vec::new(),
        ops: Vec::new(),
        excs: Vec::new(),
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

struct Loop {
    head: u32,
    is_for: bool,
    breaks: Vec<usize>,
}

struct Compiler {
    code: Code,
    scope: Option<ScopeInfo>,
    next_child: usize,
    loops: Vec<Loop>,
    finally_loops: Vec<usize>,
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

fn target_names(target: &Expr, out: &mut Vec<String>) {
    match target {
        Expr::Name(n) => add_unique(out, n),
        Expr::Tuple(elts) | Expr::List(elts) => {
            for t in elts {
                target_names(t, out);
            }
        }
        _ => {}
    }
}

fn collect_assigned(stmts: &[Stmt], out: &mut Vec<String>) {
    for s in stmts {
        match s {
            Stmt::Assign { targets, .. } => {
                for t in targets {
                    target_names(t, out);
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
                target_names(target, out);
                collect_assigned(body, out);
                collect_assigned(orelse, out);
            }
            Stmt::If { body, orelse, .. } | Stmt::While { body, orelse, .. } => {
                collect_assigned(body, out);
                collect_assigned(orelse, out);
            }
            Stmt::Try {
                body,
                handlers,
                orelse,
                finalbody,
            } => {
                collect_assigned(body, out);
                for h in handlers {
                    if let Some(n) = &h.name {
                        add_unique(out, n);
                    }
                    collect_assigned(&h.body, out);
                }
                collect_assigned(orelse, out);
                collect_assigned(finalbody, out);
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
            Stmt::Try {
                body,
                handlers,
                orelse,
                finalbody,
            } => {
                collect_nonlocals(body, out);
                for h in handlers {
                    collect_nonlocals(&h.body, out);
                }
                collect_nonlocals(orelse, out);
                collect_nonlocals(finalbody, out);
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
        Expr::ListComp { .. } | Expr::DictComp { .. } | Expr::GeneratorExp { .. } => {}
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
            Stmt::Raise(Some(e)) => expr_reads(e, out),
            Stmt::Try {
                body,
                handlers,
                orelse,
                finalbody,
            } => {
                collect_reads(body, out);
                for h in handlers {
                    if let Some(t) = &h.typ {
                        expr_reads(t, out);
                    }
                    collect_reads(&h.body, out);
                }
                collect_reads(orelse, out);
                collect_reads(finalbody, out);
            }
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
            Stmt::Try {
                body,
                handlers,
                orelse,
                finalbody,
            } => {
                for_each_def(body, out);
                for h in handlers {
                    for_each_def(&h.body, out);
                }
                for_each_def(orelse, out);
                for_each_def(finalbody, out);
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

    fn emit_jump(&mut self, op: Op) -> usize {
        self.code.ops.push(Instr { op, arg: 0 });
        self.code.ops.len() - 1
    }

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
            loops: Vec::new(),
            finally_loops: Vec::new(),
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
                self.expr(value)?;
                self.store_target(&targets[0])?;
                Ok(())
            }
            Stmt::AugAssign { target, op, value } => {
                let Expr::Name(name) = target else {
                    return unsupported("this augmented assignment target");
                };
                self.load(name)?;
                self.expr(value)?;
                let sel = bin_selector(*op)?;
                self.emit(Op::BinaryOp, sel | bytecode::BIN_AUG_FLAG);
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
                let to_orelse = self.emit_jump(Op::PopJumpIfFalse);
                self.loops.push(Loop {
                    head: start,
                    is_for: false,
                    breaks: Vec::new(),
                });
                for s in body {
                    self.stmt(s)?;
                }
                let done = self.loops.pop().unwrap();
                self.emit(Op::Jump, start);
                self.patch(to_orelse);
                for s in orelse {
                    self.stmt(s)?;
                }
                for b in done.breaks {
                    self.patch(b);
                }
                Ok(())
            }
            Stmt::For {
                target,
                iter,
                body,
                orelse,
            } => {
                self.expr(iter)?;
                self.emit(Op::GetIter, 0);
                let start = self.here();
                let exit = self.emit_jump(Op::ForIter);
                self.store_target(target)?;
                self.loops.push(Loop {
                    head: start,
                    is_for: true,
                    breaks: Vec::new(),
                });
                for s in body {
                    self.stmt(s)?;
                }
                let done = self.loops.pop().unwrap();
                self.emit(Op::Jump, start);
                self.patch(exit);
                for s in orelse {
                    self.stmt(s)?;
                }
                for b in done.breaks {
                    self.patch(b);
                }
                Ok(())
            }
            Stmt::FunctionDef { name, params, body } => self.funcdef(name, params, body),
            Stmt::Return(value) => {
                if self.scope.is_none() {
                    return unsupported("return outside a function");
                }
                if !self.finally_loops.is_empty() {
                    return unsupported("return out of a try with a finally clause");
                }
                match value {
                    Some(e) => self.expr(e)?,
                    None => self.load_const(Const::None),
                }
                self.emit(Op::Return, 0);
                Ok(())
            }
            Stmt::Try {
                body,
                handlers,
                orelse,
                finalbody,
            } => self.try_stmt(body, handlers, orelse, finalbody),
            Stmt::Raise(value) => {
                let Some(e) = value else {
                    return unsupported("bare raise");
                };
                self.expr(e)?;
                self.emit(Op::Raise, 0);
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
            Stmt::Break => {
                let Some(l) = self.loops.last() else {
                    return Err(CompileError {
                        message: "'break' outside loop".into(),
                    });
                };
                if self
                    .finally_loops
                    .last()
                    .is_some_and(|fl| self.loops.len() <= *fl)
                {
                    return unsupported("break out of a try with a finally clause");
                }
                if l.is_for {
                    self.emit(Op::PopTop, 0);
                }
                let j = self.emit_jump(Op::Jump);
                self.loops.last_mut().unwrap().breaks.push(j);
                Ok(())
            }
            Stmt::Continue => {
                let Some(l) = self.loops.last() else {
                    return Err(CompileError {
                        message: "'continue' not properly in loop".into(),
                    });
                };
                if self
                    .finally_loops
                    .last()
                    .is_some_and(|fl| self.loops.len() <= *fl)
                {
                    return unsupported("continue out of a try with a finally clause");
                }
                let head = l.head;
                self.emit(Op::Jump, head);
                Ok(())
            }
        }
    }

    fn try_stmt(
        &mut self,
        body: &[Stmt],
        handlers: &[ast::ExceptHandler],
        orelse: &[Stmt],
        finalbody: &[Stmt],
    ) -> Result<(), CompileError> {
        let depth = self.loops.iter().filter(|l| l.is_for).count() as u32;
        if !finalbody.is_empty() {
            self.finally_loops.push(self.loops.len());
        }
        let body_start = self.here();
        for s in body {
            self.stmt(s)?;
        }
        let to_else = self.emit_jump(Op::Jump);
        let dispatch = self.here();
        let mut exits = Vec::new();
        if !handlers.is_empty() {
            for h in handlers {
                let mut to_next = None;
                if let Some(t) = &h.typ {
                    self.expr(t)?;
                    self.emit(Op::ExcMatch, 0);
                    to_next = Some(self.emit_jump(Op::PopJumpIfFalse));
                }
                match &h.name {
                    Some(n) => self.store(n),
                    None => self.emit(Op::PopTop, 0),
                }
                for s in &h.body {
                    self.stmt(s)?;
                }
                exits.push(self.emit_jump(Op::Jump));
                if let Some(j) = to_next {
                    self.patch(j);
                }
            }
            self.emit(Op::Reraise, 0);
        }
        self.patch(to_else);
        for s in orelse {
            self.stmt(s)?;
        }
        for e in exits {
            self.patch(e);
        }
        let protected_end = self.here();
        if !handlers.is_empty() {
            self.code.excs.push(bytecode::ExcEntry {
                start: body_start,
                end: dispatch,
                target: dispatch,
                depth,
            });
        }
        if !finalbody.is_empty() {
            self.finally_loops.pop();
            for s in finalbody {
                self.stmt(s)?;
            }
            let to_end = self.emit_jump(Op::Jump);
            let cleanup = self.here();
            for s in finalbody {
                self.stmt(s)?;
            }
            self.emit(Op::Reraise, 0);
            self.patch(to_end);
            self.code.excs.push(bytecode::ExcEntry {
                start: body_start,
                end: protected_end,
                target: cleanup,
                depth,
            });
        }
        Ok(())
    }

    fn store_target(&mut self, target: &Expr) -> Result<(), CompileError> {
        match target {
            Expr::Name(name) => {
                self.store(name);
                Ok(())
            }
            Expr::Subscript { value, index } => {
                self.expr(value)?;
                self.expr(index)?;
                self.emit(Op::StoreSubscr, 0);
                Ok(())
            }
            Expr::Tuple(elts) | Expr::List(elts) => {
                self.emit(Op::UnpackSequence, elts.len() as u32);
                for t in elts {
                    self.store_target(t)?;
                }
                Ok(())
            }
            _ => unsupported("this assignment target"),
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
            Expr::Tuple(elts) => {
                for e in elts {
                    self.expr(e)?;
                }
                self.emit(Op::BuildTuple, elts.len() as u32);
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
            Expr::GeneratorExp { .. } => return unsupported("generator expressions"),
            Expr::ListComp { .. } | Expr::DictComp { .. } => {
                return Err(CompileError {
                    message: "comprehension survived desugaring".into(),
                });
            }
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
    for e in &mut code.excs {
        e.start = offsets[e.start as usize];
        e.end = offsets[e.end as usize];
        e.target = offsets[e.target as usize];
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
        assert!(err("x = y = 1\n").contains("chained assignment"));
        assert!(err("x = (i for i in [1])\n").contains("generator expressions"));
        assert!(err("break\n").contains("outside loop"));
        assert!(err("continue\n").contains("in loop"));
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
    fn break_in_for_pops_the_iterator() {
        let c = code("for i in [1]:\n    break\n");
        let pos = c.ops.iter().position(|i| i.op == Op::PopTop).unwrap();
        assert_eq!(c.ops[pos + 1].op, Op::Jump);
        assert_eq!(c.ops[pos + 1].arg as usize, c.ops.len());
    }

    #[test]
    fn comprehension_desugars_to_a_hidden_function() {
        let c = code("l = [x * 2 for x in range(3)]\n");
        assert_eq!(c.codes.len(), 1);
        let comp = &c.codes[0];
        assert_eq!(comp.name, "<listcomp1>");
        assert_eq!(comp.nparams, 1);
        assert!(comp.ops.iter().any(|i| i.op == Op::ForIter));
        assert!(comp.ops.iter().any(|i| i.op == Op::CallMethod));
        assert!(c.names.iter().any(|n| n == "<listcomp1>"));
    }

    #[test]
    fn tuple_assignment_unpacks() {
        let c = code("a, b = 1, 2\n");
        let ops: Vec<Op> = c.ops.iter().map(|i| i.op).collect();
        assert_eq!(
            ops,
            vec![
                Op::LoadConst,
                Op::LoadConst,
                Op::BuildTuple,
                Op::UnpackSequence,
                Op::StoreName,
                Op::StoreName
            ]
        );
    }

    #[test]
    fn try_emits_a_table_entry_and_dispatch() {
        let c = code("try:\n    x = 1\nexcept ValueError:\n    x = 2\n");
        assert_eq!(c.excs.len(), 1);
        let e = &c.excs[0];
        assert_eq!(e.start, 0);
        assert!(e.end <= e.target);
        assert_eq!(e.depth, 0);
        assert!(c.ops.iter().any(|i| i.op == Op::ExcMatch));
        assert!(c.ops.iter().any(|i| i.op == Op::Reraise));
    }

    #[test]
    fn finally_duplicates_the_cleanup() {
        let c = code("try:\n    x = 1\nfinally:\n    y = 2\n");
        assert_eq!(c.excs.len(), 1);
        let stores = c
            .ops
            .iter()
            .filter(|i| i.op == Op::StoreName && i.arg == 1)
            .count();
        assert_eq!(stores, 2);
        assert!(c.ops.iter().any(|i| i.op == Op::Reraise));
    }

    #[test]
    fn try_in_a_for_loop_records_iterator_depth() {
        let c =
            code("for i in [1]:\n    try:\n        x = 1\n    except ValueError:\n        pass\n");
        assert_eq!(c.excs.len(), 1);
        assert_eq!(c.excs[0].depth, 1);
    }

    #[test]
    fn control_flow_through_finally_is_rejected() {
        assert!(
            err("def f():\n    try:\n        return 1\n    finally:\n        pass\n")
                .contains("finally")
        );
        assert!(
            err("for i in [1]:\n    try:\n        break\n    finally:\n        pass\n")
                .contains("finally")
        );
        assert!(
            err("for i in [1]:\n    try:\n        continue\n    finally:\n        pass\n")
                .contains("finally")
        );
        let ok = "for i in [1]:\n    try:\n        for j in [1]:\n            break\n    finally:\n        pass\n";
        assert!(compile(&parser::parse(ok).unwrap()).is_ok());
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
                .any(|i| i.op == Op::BinaryOp
                    && i.arg == (bytecode::BIN_ADD | bytecode::BIN_AUG_FLAG))
        );
    }
}
