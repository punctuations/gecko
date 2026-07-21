use ast::{BinOp, BoolOp, CmpOp, ExceptHandler, Expr, Module, Param, Stmt, UnOp, WithItem};
use bytecode::{Code, Const, Instr, Op};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub struct CompileError {
    pub message: String,
}

enum LoadError {
    NotFound,
    Hard(CompileError),
}

struct Registry {
    search: Vec<PathBuf>,
    by_path: Vec<(PathBuf, u32)>,
    modules: Vec<Code>,
}

pub fn compile(module: &Module) -> Result<Code, CompileError> {
    compile_with_base(module, None)
}

pub fn compile_with_base(module: &Module, base: Option<PathBuf>) -> Result<Code, CompileError> {
    let mut search = Vec::new();
    let dir = base.clone();
    if let Some(base) = base {
        search.push(base);
    }
    if let Some(path) = std::env::var_os("GECKO_PATH") {
        search.extend(std::env::split_paths(&path));
    }
    if let Some(site) = site_packages() {
        search.push(site);
    }
    let reg = Rc::new(RefCell::new(Registry {
        search,
        by_path: Vec::new(),
        modules: Vec::new(),
    }));
    let mut code = compile_unit(module, "<module>", dir, &reg)?;
    code.modules = std::mem::take(&mut reg.borrow_mut().modules);
    assemble(&mut code);
    Ok(code)
}

pub fn gecko_home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("GECKO_HOME") {
        return Some(PathBuf::from(home));
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".gecko"))
}

pub fn site_packages() -> Option<PathBuf> {
    gecko_home().map(|h| h.join("site-packages"))
}

fn resolve_in_dir(dir: &std::path::Path, seg: &str) -> Option<(PathBuf, bool)> {
    let package = dir.join(seg).join("__init__.py");
    if let Ok(key) = package.canonicalize() {
        if key.is_file() {
            return Some((key, true));
        }
    }
    let file = dir.join(format!("{seg}.py"));
    if let Ok(key) = file.canonicalize() {
        if key.is_file() {
            return Some((key, false));
        }
    }
    None
}

fn resolve_on_path(search: &[PathBuf], seg: &str) -> Option<(PathBuf, bool)> {
    search.iter().find_map(|dir| resolve_in_dir(dir, seg))
}

fn compile_unit(
    module: &Module,
    name: &str,
    dir: Option<PathBuf>,
    reg: &Rc<RefCell<Registry>>,
) -> Result<Code, CompileError> {
    let mut module = module.clone();
    let mut comps = 0u32;
    lower_with(&mut module.body, &mut comps);
    desugar_block(&mut module.body, &mut comps);
    let mut c = Compiler {
        code: new_code(name, 0, 0, 0, 0),
        scope: None,
        next_child: 0,
        loops: Vec::new(),
        finallies: Vec::new(),
        registry: reg.clone(),
        dir,
    };
    for stmt in &module.body {
        c.stmt(stmt)?;
    }
    Ok(c.code)
}

fn lower_with(stmts: &mut Vec<Stmt>, n: &mut u32) {
    let mut i = 0;
    while i < stmts.len() {
        match &mut stmts[i] {
            Stmt::FunctionDef { body, .. } | Stmt::ClassDef { body, .. } => lower_with(body, n),
            Stmt::If { body, orelse, .. } | Stmt::While { body, orelse, .. } => {
                lower_with(body, n);
                lower_with(orelse, n);
            }
            Stmt::For { body, orelse, .. } => {
                lower_with(body, n);
                lower_with(orelse, n);
            }
            Stmt::Try {
                body,
                handlers,
                orelse,
                finalbody,
            } => {
                lower_with(body, n);
                for h in handlers.iter_mut() {
                    lower_with(&mut h.body, n);
                }
                lower_with(orelse, n);
                lower_with(finalbody, n);
            }
            _ => {}
        }
        if matches!(stmts[i], Stmt::With { .. }) {
            let Stmt::With { items, body } = std::mem::replace(&mut stmts[i], Stmt::Pass) else {
                unreachable!()
            };
            let lowered = build_with(items, body, n);
            let len = lowered.len();
            stmts.splice(i..=i, lowered);
            i += len;
        } else {
            i += 1;
        }
    }
}

fn build_with(items: Vec<WithItem>, mut body: Vec<Stmt>, n: &mut u32) -> Vec<Stmt> {
    lower_with(&mut body, n);
    for item in items.into_iter().rev() {
        body = with_one(item, body, n);
    }
    body
}

fn with_one(item: WithItem, body: Vec<Stmt>, n: &mut u32) -> Vec<Stmt> {
    *n += 1;
    let cm = format!(".cm{}", *n);
    let hit = format!(".hit{}", *n);
    let exc = format!(".exc{}", *n);
    let dunder = |attr: &str| Expr::Attribute {
        value: Box::new(Expr::Name(cm.clone())),
        attr: attr.into(),
    };
    let call = |func: Expr, args: Vec<Expr>| Expr::Call {
        func: Box::new(func),
        args,
        keywords: Vec::new(),
    };

    let mut out = Vec::new();
    out.push(Stmt::Assign {
        targets: vec![Expr::Name(cm.clone())],
        value: item.context,
    });
    let enter = call(dunder("__enter__"), Vec::new());
    match item.optional_vars {
        Some(var) => out.push(Stmt::Assign {
            targets: vec![var],
            value: enter,
        }),
        None => out.push(Stmt::Expr(enter)),
    }
    out.push(Stmt::Assign {
        targets: vec![Expr::Name(hit.clone())],
        value: Expr::Bool(false),
    });

    let exc_type = call(Expr::Name("type".into()), vec![Expr::Name(exc.clone())]);
    let exit_exc = call(
        dunder("__exit__"),
        vec![exc_type, Expr::Name(exc.clone()), Expr::None],
    );
    let handler_body = vec![
        Stmt::Assign {
            targets: vec![Expr::Name(hit.clone())],
            value: Expr::Bool(true),
        },
        Stmt::If {
            test: Expr::Unary {
                op: UnOp::Not,
                operand: Box::new(exit_exc),
            },
            body: vec![Stmt::Raise(Some(Expr::Name(exc.clone())))],
            orelse: Vec::new(),
        },
    ];
    let exit_normal = call(dunder("__exit__"), vec![Expr::None, Expr::None, Expr::None]);
    let finalbody = vec![Stmt::If {
        test: Expr::Unary {
            op: UnOp::Not,
            operand: Box::new(Expr::Name(hit.clone())),
        },
        body: vec![Stmt::Expr(exit_normal)],
        orelse: Vec::new(),
    }];
    out.push(Stmt::Try {
        body,
        handlers: vec![ExceptHandler {
            typ: Some(Expr::Name("Exception".into())),
            name: Some(exc),
            body: handler_body,
        }],
        orelse: Vec::new(),
        finalbody,
    });
    out
}

fn desugar_block(stmts: &mut Vec<Stmt>, n: &mut u32) {
    let mut i = 0;
    while i < stmts.len() {
        match &mut stmts[i] {
            Stmt::FunctionDef { body, .. } | Stmt::ClassDef { body, .. } => desugar_block(body, n),
            Stmt::If { body, orelse, .. } | Stmt::While { body, orelse, .. } => {
                desugar_block(body, n);
                desugar_block(orelse, n);
            }
            Stmt::For { body, orelse, .. } => {
                desugar_block(body, n);
                desugar_block(orelse, n);
            }
            Stmt::With { body, .. } => desugar_block(body, n),
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
        Stmt::Assert { test, msg } => {
            desugar_expr(test, hoisted, n);
            if let Some(m) = msg {
                desugar_expr(m, hoisted, n);
            }
        }
        Stmt::Delete(targets) => {
            for t in targets {
                desugar_expr(t, hoisted, n);
            }
        }
        Stmt::With { items, .. } => {
            for it in items {
                desugar_expr(&mut it.context, hoisted, n);
                if let Some(v) = &mut it.optional_vars {
                    desugar_expr(v, hoisted, n);
                }
            }
        }
        Stmt::Return(Some(e)) => desugar_expr(e, hoisted, n),
        Stmt::Raise(Some(e)) => desugar_expr(e, hoisted, n),
        Stmt::Try { handlers, .. } => {
            for h in handlers.iter_mut() {
                if let Some(t) = &mut h.typ {
                    desugar_expr(t, hoisted, n);
                }
            }
        }
        Stmt::ClassDef {
            bases, decorators, ..
        } => {
            for b in bases {
                desugar_expr(b, hoisted, n);
            }
            for d in decorators {
                desugar_expr(d, hoisted, n);
            }
        }
        Stmt::FunctionDef { decorators, .. } => {
            for d in decorators {
                desugar_expr(d, hoisted, n);
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
        Expr::Starred(inner) => desugar_expr(inner, hoisted, n),
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
            for c in comparators.iter_mut() {
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
        Expr::IfExp { test, body, orelse } => {
            desugar_expr(test, hoisted, n);
            desugar_expr(body, hoisted, n);
            desugar_expr(orelse, hoisted, n);
        }
        Expr::Lambda { params, body } => {
            *n += 1;
            let fname = format!("<lambda{}>", *n);
            for p in params.iter_mut() {
                if let Some(d) = &mut p.default {
                    desugar_expr(d, hoisted, n);
                }
            }
            let mut inner = Vec::new();
            desugar_expr(body, &mut inner, n);
            let mut fbody = inner;
            fbody.push(Stmt::Return(Some(std::mem::replace(
                &mut **body,
                Expr::None,
            ))));
            let func = Stmt::FunctionDef {
                name: fname.clone(),
                params: std::mem::take(params),
                body: fbody,
                decorators: Vec::new(),
            };
            hoisted.push(func);
            *e = Expr::Name(fname);
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
            kind: ast::ParamKind::Normal,
        }],
        body,
        decorators: Vec::new(),
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
        ndefaults: 0,
        ncells,
        nfrees,
        param_names: Vec::new(),
        varargs: false,
        kwargs: false,
        codes: Vec::new(),
        modules: Vec::new(),
        parent_module: -1,
    }
}

#[derive(Debug, Clone)]
struct ScopeInfo {
    locals: Vec<String>,
    cells: Vec<String>,
    frees: Vec<String>,
    children: Vec<ScopeInfo>,
    is_class: bool,
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
    if s.is_class {
        if let Some(i) = s.locals.iter().position(|n| n == name) {
            return Slot::Local(i as u32);
        }
        if let Some(i) = s.frees.iter().position(|n| n == name) {
            return Slot::Free(s.cells.len() as u32 + i as u32);
        }
        return Slot::Global;
    }
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

fn capture_slot(s: &ScopeInfo, name: &str) -> Option<u32> {
    if let Some(i) = s.cells.iter().position(|n| n == name) {
        return Some(i as u32);
    }
    if let Some(i) = s.frees.iter().position(|n| n == name) {
        return Some(s.cells.len() as u32 + i as u32);
    }
    None
}

struct Loop {
    head: u32,
    is_for: bool,
    breaks: Vec<usize>,
}

#[derive(Clone)]
struct FinallyFrame {
    body: Vec<Stmt>,
    loops_at: usize,
}

struct Compiler {
    code: Code,
    scope: Option<ScopeInfo>,
    next_child: usize,
    loops: Vec<Loop>,
    finallies: Vec<FinallyFrame>,
    registry: Rc<RefCell<Registry>>,
    dir: Option<PathBuf>,
}

impl Compiler {
    fn resolves_on_path(&self, top: &str) -> bool {
        let search = self.registry.borrow().search.clone();
        resolve_on_path(&search, top).is_some()
    }

    fn relative_base(&self, level: u32) -> Result<PathBuf, CompileError> {
        let Some(dir) = &self.dir else {
            return Err(CompileError {
                message: "attempted relative import with no known source directory".into(),
            });
        };
        let mut d = dir.clone();
        for _ in 1..level {
            d = d
                .parent()
                .map(|p| p.to_path_buf())
                .ok_or_else(|| CompileError {
                    message: "attempted relative import beyond top-level package".into(),
                })?;
        }
        Ok(d)
    }

    fn load_dotted(&self, dotted: &str) -> Result<Vec<u32>, LoadError> {
        self.load_dotted_from(None, dotted)
    }

    fn load_dotted_from(
        &self,
        start: Option<&std::path::Path>,
        dotted: &str,
    ) -> Result<Vec<u32>, LoadError> {
        let search = self.registry.borrow().search.clone();
        if start.is_none() && search.is_empty() {
            return Err(LoadError::NotFound);
        }
        let segments: Vec<&str> = dotted.split('.').collect();
        let mut ids = Vec::new();
        let mut parent_id: i32 = -1;
        let mut parent_dir: Option<PathBuf> = start.map(|p| p.to_path_buf());
        let mut qual = String::new();
        for (i, seg) in segments.iter().enumerate() {
            let is_leaf = i + 1 == segments.len();
            if qual.is_empty() {
                qual.push_str(seg);
            } else {
                qual.push('.');
                qual.push_str(seg);
            }
            let resolved = match &parent_dir {
                Some(dir) => resolve_in_dir(dir, seg),
                None => resolve_on_path(&search, seg),
            };
            let Some((key, is_pkg)) = resolved else {
                return Err(LoadError::NotFound);
            };
            if !is_leaf && !is_pkg {
                return Err(LoadError::NotFound);
            }
            let id = self
                .get_or_compile(&key, &qual, parent_id)
                .map_err(LoadError::Hard)?;
            ids.push(id);
            parent_id = id as i32;
            parent_dir = if is_pkg {
                key.parent().map(|p| p.to_path_buf())
            } else {
                None
            };
        }
        Ok(ids)
    }

    fn get_or_compile(
        &self,
        key: &std::path::Path,
        qual: &str,
        parent_id: i32,
    ) -> Result<u32, CompileError> {
        if let Some(id) = self
            .registry
            .borrow()
            .by_path
            .iter()
            .find_map(|(p, id)| if p == key { Some(*id) } else { None })
        {
            return Ok(id);
        }
        let src = std::fs::read_to_string(key).map_err(|e| CompileError {
            message: format!("cannot read module '{qual}': {e}"),
        })?;
        let ast = parser::parse(&src).map_err(|e| CompileError {
            message: format!("in module '{qual}': SyntaxError: {}", e.message),
        })?;
        let id = {
            let mut reg = self.registry.borrow_mut();
            let id = reg.modules.len() as u32;
            reg.modules.push(new_code(qual, 0, 0, 0, 0));
            reg.by_path.push((key.to_path_buf(), id));
            id
        };
        let dir = key.parent().map(|p| p.to_path_buf());
        let mut code = compile_unit(&ast, qual, dir, &self.registry)?;
        code.parent_module = parent_id;
        self.registry.borrow_mut().modules[id as usize] = code;
        Ok(id)
    }

    fn emit_import_missing(&mut self, name: &str) {
        let i = self.name_idx(name);
        self.emit(Op::ImportMissing, i);
    }

    fn import_from_relative(
        &mut self,
        level: u32,
        module: &str,
        names: &[ast::Alias],
    ) -> Result<(), CompileError> {
        let base = self.relative_base(level)?;
        if module.is_empty() {
            for a in names {
                match self.load_dotted_from(Some(&base), &a.name) {
                    Ok(sub_ids) => {
                        for id in &sub_ids {
                            self.emit(Op::Import, *id);
                            self.emit(Op::PopTop, 0);
                        }
                        self.emit(Op::Import, *sub_ids.last().unwrap());
                        let bound = a.asname.as_deref().unwrap_or(&a.name);
                        self.store(bound);
                    }
                    Err(LoadError::NotFound) => self.emit_import_missing(&a.name),
                    Err(LoadError::Hard(e)) => return Err(e),
                }
            }
            return Ok(());
        }
        let ids = match self.load_dotted_from(Some(&base), module) {
            Ok(ids) => ids,
            Err(LoadError::NotFound) => {
                self.emit_import_missing(module);
                return Ok(());
            }
            Err(LoadError::Hard(e)) => return Err(e),
        };
        for id in &ids {
            self.emit(Op::Import, *id);
            self.emit(Op::PopTop, 0);
        }
        let leaf_id = *ids.last().unwrap();
        for a in names {
            match self.load_dotted_from(Some(&base), &format!("{module}.{}", a.name)) {
                Ok(sub_ids) => {
                    for id in &sub_ids {
                        self.emit(Op::Import, *id);
                        self.emit(Op::PopTop, 0);
                    }
                }
                Err(LoadError::Hard(e)) => return Err(e),
                Err(LoadError::NotFound) => {}
            }
            self.emit(Op::Import, leaf_id);
            let i = self.name_idx(&a.name);
            self.emit(Op::LoadAttr, i);
            let bound = a.asname.as_deref().unwrap_or(&a.name);
            self.store(bound);
        }
        Ok(())
    }
}

fn builtin_module(name: &str) -> Option<&'static str> {
    match name {
        "gecko" | "_gecko" => Some("_gecko"),
        _ => None,
    }
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
            Stmt::With { items, body } => {
                for it in items {
                    if let Some(v) = &it.optional_vars {
                        target_names(v, out);
                    }
                }
                collect_assigned(body, out);
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
            Stmt::FunctionDef { name, .. } | Stmt::ClassDef { name, .. } => add_unique(out, name),
            Stmt::Import(aliases) => {
                for a in aliases {
                    let bound = match &a.asname {
                        Some(x) => x.as_str(),
                        None => a.name.split('.').next().unwrap_or(&a.name),
                    };
                    add_unique(out, bound);
                }
            }
            Stmt::ImportFrom { names, .. } => {
                for a in names {
                    add_unique(out, a.asname.as_deref().unwrap_or(&a.name));
                }
            }
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
            Stmt::With { body, .. } => collect_nonlocals(body, out),
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

fn collect_globals(stmts: &[Stmt], out: &mut Vec<String>) {
    for s in stmts {
        match s {
            Stmt::Global(names) => {
                for n in names {
                    add_unique(out, n);
                }
            }
            Stmt::If { body, orelse, .. } | Stmt::While { body, orelse, .. } => {
                collect_globals(body, out);
                collect_globals(orelse, out);
            }
            Stmt::For { body, orelse, .. } => {
                collect_globals(body, out);
                collect_globals(orelse, out);
            }
            Stmt::With { body, .. } => collect_globals(body, out),
            Stmt::Try {
                body,
                handlers,
                orelse,
                finalbody,
            } => {
                collect_globals(body, out);
                for h in handlers {
                    collect_globals(&h.body, out);
                }
                collect_globals(orelse, out);
                collect_globals(finalbody, out);
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
        Expr::Starred(inner) => expr_reads(inner, out),
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
        Expr::IfExp { test, body, orelse } => {
            expr_reads(test, out);
            expr_reads(body, out);
            expr_reads(orelse, out);
        }
        Expr::ListComp { .. }
        | Expr::DictComp { .. }
        | Expr::GeneratorExp { .. }
        | Expr::Lambda { .. } => {}
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
            Stmt::ClassDef {
                bases, decorators, ..
            } => {
                for b in bases {
                    expr_reads(b, out);
                }
                for d in decorators {
                    expr_reads(d, out);
                }
            }
            Stmt::FunctionDef { decorators, .. } => {
                for d in decorators {
                    expr_reads(d, out);
                }
            }
            Stmt::Assert { test, msg } => {
                expr_reads(test, out);
                if let Some(m) = msg {
                    expr_reads(m, out);
                }
            }
            Stmt::Delete(targets) => {
                for t in targets {
                    expr_reads(t, out);
                }
            }
            Stmt::With { items, body } => {
                for it in items {
                    expr_reads(&it.context, out);
                    if let Some(v) = &it.optional_vars {
                        expr_reads(v, out);
                    }
                }
                collect_reads(body, out);
            }
            _ => {}
        }
    }
}

enum ScopeChild<'a> {
    Func(&'a [Param], &'a [Stmt]),
    Class(&'a [Stmt]),
}

fn for_each_def<'a>(stmts: &'a [Stmt], out: &mut Vec<ScopeChild<'a>>) {
    for s in stmts {
        match s {
            Stmt::FunctionDef { params, body, .. } => out.push(ScopeChild::Func(params, body)),
            Stmt::ClassDef { body, .. } => out.push(ScopeChild::Class(body)),
            Stmt::If { body, orelse, .. } | Stmt::While { body, orelse, .. } => {
                for_each_def(body, out);
                for_each_def(orelse, out);
            }
            Stmt::For { body, orelse, .. } => {
                for_each_def(body, out);
                for_each_def(orelse, out);
            }
            Stmt::With { body, .. } => for_each_def(body, out),
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
    is_class: bool,
) -> Result<ScopeInfo, CompileError> {
    let mut nonlocals = Vec::new();
    collect_nonlocals(body, &mut nonlocals);
    let mut globals = Vec::new();
    collect_globals(body, &mut globals);

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
    locals.retain(|l| !globals.iter().any(|g| g == l));

    let mut frees = nonlocals;
    let mut reads = Vec::new();
    collect_reads(body, &mut reads);
    for r in &reads {
        if locals.iter().any(|l| l == r) || frees.iter().any(|f| f == r) {
            continue;
        }
        if globals.iter().any(|g| g == r) {
            continue;
        }
        if enclosing.iter().any(|s| s.iter().any(|b| b == r)) {
            frees.push(r.clone());
        }
    }

    let mut chain: Vec<Vec<String>> = enclosing.to_vec();
    if !is_class {
        let mut bindings = locals.clone();
        bindings.extend(frees.iter().cloned());
        chain.push(bindings);
    }

    let mut cells: Vec<String> = Vec::new();
    let mut children = Vec::new();
    let mut defs = Vec::new();
    for_each_def(body, &mut defs);
    for child in defs {
        let ci = match child {
            ScopeChild::Func(cparams, cbody) => analyze_function(cparams, cbody, &chain, false)?,
            ScopeChild::Class(cbody) => analyze_function(&[], cbody, &chain, true)?,
        };
        for f in &ci.frees {
            if !is_class && locals.iter().any(|l| l == f) {
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
        is_class,
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

    fn funcdef(
        &mut self,
        name: &str,
        params: &[Param],
        body: &[Stmt],
        decorators: &[Expr],
    ) -> Result<(), CompileError> {
        let mut seen_star = false;
        let mut seen_dstar = false;
        let mut nvar = 0;
        let mut nkw = 0;
        for p in params {
            match p.kind {
                ast::ParamKind::Normal => {
                    if seen_star || seen_dstar {
                        return Err(CompileError {
                            message: "parameter follows * or ** parameter".into(),
                        });
                    }
                }
                ast::ParamKind::VarArgs => {
                    if seen_star || seen_dstar {
                        return Err(CompileError {
                            message: "duplicate or misplaced * parameter".into(),
                        });
                    }
                    seen_star = true;
                    nvar += 1;
                }
                ast::ParamKind::KwArgs => {
                    if seen_dstar {
                        return Err(CompileError {
                            message: "duplicate ** parameter".into(),
                        });
                    }
                    seen_dstar = true;
                    nkw += 1;
                }
            }
        }
        let k = params.len() - nvar - nkw;
        let first_default = params[..k].iter().position(|p| p.default.is_some());
        let ndefaults = match first_default {
            Some(start) => {
                if params[start..k].iter().any(|p| p.default.is_none()) {
                    return Err(CompileError {
                        message: "non-default argument follows default argument".into(),
                    });
                }
                (k - start) as u32
            }
            None => 0,
        };
        let info = match &self.scope {
            Some(s) => {
                let ci = s.children[self.next_child].clone();
                self.next_child += 1;
                ci
            }
            None => analyze_function(params, body, &[], false)?,
        };
        let frees = info.frees.clone();
        let mut child = new_code(
            name,
            info.locals.len() as u32,
            k as u32,
            info.cells.len() as u32,
            info.frees.len() as u32,
        );
        child.ndefaults = ndefaults;
        child.varargs = seen_star;
        child.kwargs = seen_dstar;
        child.param_names = params[..k].iter().map(|p| p.name.clone()).collect();
        let mut sub = Compiler {
            code: child,
            scope: Some(info),
            next_child: 0,
            loops: Vec::new(),
            finallies: Vec::new(),
            registry: self.registry.clone(),
            dir: self.dir.clone(),
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
        for d in decorators {
            self.expr(d)?;
        }
        if let Some(start) = first_default {
            for p in &params[start..k] {
                self.expr(p.default.as_ref().unwrap())?;
            }
        }
        self.emit_captures(&frees)?;
        self.emit(Op::MakeFunction, idx);
        for _ in decorators {
            self.emit(Op::Call, 1);
        }
        self.store(name);
        Ok(())
    }

    fn build_call_positionals(&mut self, args: &[Expr]) -> Result<(), CompileError> {
        self.emit(Op::BuildList, 0);
        let mut i = 0;
        while i < args.len() {
            if let Expr::Starred(inner) = &args[i] {
                self.expr(inner)?;
                self.emit(Op::ListExtend, 0);
                i += 1;
            } else {
                let start = i;
                while i < args.len() && !matches!(args[i], Expr::Starred(_)) {
                    i += 1;
                }
                for a in &args[start..i] {
                    self.expr(a)?;
                }
                self.emit(Op::BuildList, (i - start) as u32);
                self.emit(Op::ListExtend, 0);
            }
        }
        Ok(())
    }

    fn build_call_kwargs(&mut self, keywords: &[ast::Keyword]) -> Result<(), CompileError> {
        self.emit(Op::BuildDict, 0);
        let mut i = 0;
        while i < keywords.len() {
            if keywords[i].arg.is_none() {
                self.expr(&keywords[i].value)?;
                self.emit(Op::DictMerge, 0);
                i += 1;
            } else {
                let start = i;
                while i < keywords.len() && keywords[i].arg.is_some() {
                    i += 1;
                }
                for kw in &keywords[start..i] {
                    self.load_const(Const::Str(kw.arg.clone().unwrap()));
                    self.expr(&kw.value)?;
                }
                self.emit(Op::BuildDict, (i - start) as u32);
                self.emit(Op::DictMerge, 0);
            }
        }
        Ok(())
    }

    fn emit_captures(&mut self, frees: &[String]) -> Result<(), CompileError> {
        for f in frees {
            match self.scope.as_ref().and_then(|s| capture_slot(s, f)) {
                Some(i) => self.emit(Op::LoadClosure, i),
                None => {
                    return Err(CompileError {
                        message: format!("cannot capture '{f}'"),
                    });
                }
            }
        }
        Ok(())
    }

    fn classdef(
        &mut self,
        name: &str,
        bases: &[Expr],
        body: &[Stmt],
        decorators: &[Expr],
    ) -> Result<(), CompileError> {
        if bases.len() > 1 {
            return unsupported("multiple inheritance");
        }
        let info = match &self.scope {
            Some(s) => {
                let ci = s.children[self.next_child].clone();
                self.next_child += 1;
                ci
            }
            None => analyze_function(&[], body, &[], true)?,
        };
        let frees = info.frees.clone();
        let locals = info.locals.clone();
        let mut sub = Compiler {
            code: new_code(
                name,
                info.locals.len() as u32,
                0,
                info.cells.len() as u32,
                info.frees.len() as u32,
            ),
            scope: Some(info),
            next_child: 0,
            loops: Vec::new(),
            finallies: Vec::new(),
            registry: self.registry.clone(),
            dir: self.dir.clone(),
        };
        for s in body {
            sub.stmt(s)?;
        }
        let mut pairs = 0;
        for (slot, l) in locals.iter().enumerate() {
            if l.starts_with('<') || l.starts_with('.') {
                continue;
            }
            sub.load_const(Const::Str(l.clone()));
            sub.emit(Op::LoadLocal, slot as u32);
            pairs += 1;
        }
        sub.emit(Op::BuildDict, pairs);
        sub.emit(Op::Return, 0);
        let idx = self.code.codes.len() as u32;
        self.code.codes.push(sub.code);
        for d in decorators {
            self.expr(d)?;
        }
        self.emit_captures(&frees)?;
        self.emit(Op::MakeFunction, idx);
        self.emit(Op::Call, 0);
        match bases.first() {
            Some(b) => self.expr(b)?,
            None => self.load_const(Const::None),
        }
        self.load_const(Const::Str(name.to_string()));
        self.emit(Op::MakeClass, 0);
        for _ in decorators {
            self.emit(Op::Call, 1);
        }
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
                self.expr(value)?;
                let last = targets.len() - 1;
                for (i, t) in targets.iter().enumerate() {
                    if i < last {
                        self.emit(Op::DupTop, 0);
                    }
                    self.store_target(t)?;
                }
                Ok(())
            }
            Stmt::AugAssign { target, op, value } => {
                match target {
                    Expr::Name(_) | Expr::Attribute { .. } | Expr::Subscript { .. } => {}
                    _ => return unsupported("this augmented assignment target"),
                }
                self.expr(target)?;
                self.expr(value)?;
                let sel = bin_selector(*op)?;
                self.emit(Op::BinaryOp, sel | bytecode::BIN_AUG_FLAG);
                self.store_target(target)?;
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
            Stmt::FunctionDef {
                name,
                params,
                body,
                decorators,
            } => self.funcdef(name, params, body, decorators),
            Stmt::ClassDef {
                name,
                bases,
                body,
                decorators,
            } => self.classdef(name, bases, body, decorators),
            Stmt::Return(value) => {
                if self.scope.is_none() {
                    return unsupported("return outside a function");
                }
                match value {
                    Some(e) => self.expr(e)?,
                    None => self.load_const(Const::None),
                }
                self.emit_exit_finallies(0)?;
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
            Stmt::Global(_) => Ok(()),
            Stmt::Assert { test, msg } => {
                self.expr(test)?;
                let skip = self.emit_jump(Op::PopJumpIfTrue);
                let ae = self.name_idx("AssertionError");
                self.emit(Op::LoadName, ae);
                let argc = match msg {
                    Some(m) => {
                        self.expr(m)?;
                        1
                    }
                    None => 0,
                };
                self.emit(Op::Call, argc);
                self.emit(Op::Raise, 0);
                self.patch(skip);
                Ok(())
            }
            Stmt::Delete(targets) => {
                for t in targets {
                    self.delete_target(t)?;
                }
                Ok(())
            }
            Stmt::With { items, body } => self.with_stmt(items, body),
            Stmt::Import(aliases) => {
                for a in aliases {
                    let top = a.name.split('.').next().unwrap();
                    if !self.resolves_on_path(top) {
                        if let Some(builtin) = builtin_module(&a.name) {
                            let name = self.name_idx(builtin);
                            self.emit(Op::LoadName, name);
                            let bound = a.asname.as_deref().unwrap_or(top);
                            self.store(bound);
                            continue;
                        }
                    }
                    let ids = match self.load_dotted(&a.name) {
                        Ok(ids) => ids,
                        Err(LoadError::NotFound) => {
                            self.emit_import_missing(&a.name);
                            continue;
                        }
                        Err(LoadError::Hard(e)) => return Err(e),
                    };
                    for id in &ids {
                        self.emit(Op::Import, *id);
                        self.emit(Op::PopTop, 0);
                    }
                    match &a.asname {
                        Some(x) => {
                            self.emit(Op::Import, *ids.last().unwrap());
                            self.store(x);
                        }
                        None => {
                            let top = a.name.split('.').next().unwrap().to_string();
                            self.emit(Op::Import, ids[0]);
                            self.store(&top);
                        }
                    }
                }
                Ok(())
            }
            Stmt::ImportFrom {
                module,
                names,
                level,
            } => {
                if *level > 0 {
                    return self.import_from_relative(*level, module, names);
                }
                let top = module.split('.').next().unwrap();
                if !self.resolves_on_path(top) {
                    if let Some(builtin) = builtin_module(module) {
                        for a in names {
                            let m = self.name_idx(builtin);
                            self.emit(Op::LoadName, m);
                            let i = self.name_idx(&a.name);
                            self.emit(Op::LoadAttr, i);
                            let bound = a.asname.as_deref().unwrap_or(&a.name);
                            self.store(bound);
                        }
                        return Ok(());
                    }
                }
                let ids = match self.load_dotted(module) {
                    Ok(ids) => ids,
                    Err(LoadError::NotFound) => {
                        self.emit_import_missing(module);
                        return Ok(());
                    }
                    Err(LoadError::Hard(e)) => return Err(e),
                };
                for id in &ids {
                    self.emit(Op::Import, *id);
                    self.emit(Op::PopTop, 0);
                }
                let leaf_id = *ids.last().unwrap();
                for a in names {
                    match self.load_dotted(&format!("{module}.{}", a.name)) {
                        Ok(sub_ids) => {
                            for id in &sub_ids {
                                self.emit(Op::Import, *id);
                                self.emit(Op::PopTop, 0);
                            }
                        }
                        Err(LoadError::Hard(e)) => return Err(e),
                        Err(LoadError::NotFound) => {}
                    }
                    self.emit(Op::Import, leaf_id);
                    let i = self.name_idx(&a.name);
                    self.emit(Op::LoadAttr, i);
                    let bound = a.asname.as_deref().unwrap_or(&a.name);
                    self.store(bound);
                }
                Ok(())
            }
            Stmt::Pass => Ok(()),
            Stmt::Break => {
                if self.loops.is_empty() {
                    return Err(CompileError {
                        message: "'break' outside loop".into(),
                    });
                }
                self.emit_exit_finallies(self.loops.len())?;
                if self.loops.last().unwrap().is_for {
                    self.emit(Op::PopTop, 0);
                }
                let j = self.emit_jump(Op::Jump);
                self.loops.last_mut().unwrap().breaks.push(j);
                Ok(())
            }
            Stmt::Continue => {
                if self.loops.is_empty() {
                    return Err(CompileError {
                        message: "'continue' not properly in loop".into(),
                    });
                }
                self.emit_exit_finallies(self.loops.len())?;
                let head = self.loops.last().unwrap().head;
                self.emit(Op::Jump, head);
                Ok(())
            }
        }
    }

    fn emit_exit_finallies(&mut self, min_loops: usize) -> Result<(), CompileError> {
        let saved = self.finallies.clone();
        while let Some(top) = self.finallies.last() {
            if top.loops_at < min_loops {
                break;
            }
            let frame = self.finallies.pop().unwrap();
            for s in &frame.body {
                self.stmt(s)?;
            }
        }
        self.finallies = saved;
        Ok(())
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
            self.finallies.push(FinallyFrame {
                body: finalbody.to_vec(),
                loops_at: self.loops.len(),
            });
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
            self.finallies.pop();
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

    fn delete_target(&mut self, t: &Expr) -> Result<(), CompileError> {
        match t {
            Expr::Name(name) => {
                match resolve(self.scope.as_ref(), name) {
                    Slot::Local(i) => self.emit(Op::DeleteLocal, i),
                    Slot::Cell(i) | Slot::Free(i) => self.emit(Op::DeleteDeref, i),
                    Slot::Global => {
                        let i = self.name_idx(name);
                        self.emit(Op::DeleteName, i);
                    }
                }
                Ok(())
            }
            Expr::Subscript { value, index } => {
                self.expr(value)?;
                self.expr(index)?;
                self.emit(Op::DeleteSubscr, 0);
                Ok(())
            }
            Expr::Attribute { value, attr } => {
                self.expr(value)?;
                let i = self.name_idx(attr);
                self.emit(Op::DeleteAttr, i);
                Ok(())
            }
            _ => unsupported("this delete target"),
        }
    }

    fn with_stmt(&mut self, _items: &[WithItem], _body: &[Stmt]) -> Result<(), CompileError> {
        unsupported("with")
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
            Expr::Attribute { value, attr } => {
                self.expr(value)?;
                let i = self.name_idx(attr);
                self.emit(Op::StoreAttr, i);
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
        if let Some(c) = fold_expr(e) {
            self.load_const(c);
            return Ok(());
        }
        match e {
            Expr::Starred(_) => return unsupported("starred expression in this position"),
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
                let has_star = args.iter().any(|a| matches!(a, Expr::Starred(_)));
                if keywords.is_empty() && !has_star {
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
                } else {
                    if let Expr::Attribute { value, attr } = func.as_ref() {
                        self.expr(value)?;
                        let name = self.name_idx(attr);
                        self.emit(Op::LoadAttr, name);
                    } else {
                        self.expr(func)?;
                    }
                    self.build_call_positionals(args)?;
                    self.build_call_kwargs(keywords)?;
                    self.emit(Op::CallEx, 0);
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
                self.expr(left)?;
                self.expr(&comparators[0])?;
                if ops.len() == 1 {
                    self.emit(Op::CompareOp, cmp_selector(ops[0]));
                } else {
                    let last = ops.len() - 1;
                    let mut cleanup = Vec::new();
                    for (i, op) in ops.iter().enumerate() {
                        if i < last {
                            self.emit(Op::DupTop, 0);
                            self.emit(Op::RotThree, 0);
                            self.emit(Op::CompareOp, cmp_selector(*op));
                            cleanup.push(self.emit_jump(Op::JumpIfFalseOrPop));
                            self.expr(&comparators[i + 1])?;
                        } else {
                            self.emit(Op::CompareOp, cmp_selector(*op));
                        }
                    }
                    let to_end = self.emit_jump(Op::Jump);
                    for c in cleanup {
                        self.patch(c);
                    }
                    self.emit(Op::RotTwo, 0);
                    self.emit(Op::PopTop, 0);
                    self.patch(to_end);
                }
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
            Expr::Attribute { value, attr } => {
                self.expr(value)?;
                let i = self.name_idx(attr);
                self.emit(Op::LoadAttr, i);
            }
            Expr::GeneratorExp { .. } => return unsupported("generator expressions"),
            Expr::IfExp { test, body, orelse } => {
                self.expr(test)?;
                let to_else = self.emit_jump(Op::PopJumpIfFalse);
                self.expr(body)?;
                let to_end = self.emit_jump(Op::Jump);
                self.patch(to_else);
                self.expr(orelse)?;
                self.patch(to_end);
            }
            Expr::ListComp { .. } | Expr::DictComp { .. } | Expr::Lambda { .. } => {
                return Err(CompileError {
                    message: "comprehension or lambda survived desugaring".into(),
                });
            }
        }
        Ok(())
    }
}

fn cmp_selector(op: CmpOp) -> u32 {
    match op {
        CmpOp::Eq => bytecode::CMP_EQ,
        CmpOp::NotEq => bytecode::CMP_NE,
        CmpOp::Lt => bytecode::CMP_LT,
        CmpOp::LtE => bytecode::CMP_LE,
        CmpOp::Gt => bytecode::CMP_GT,
        CmpOp::GtE => bytecode::CMP_GE,
        CmpOp::In => bytecode::CMP_IN,
        CmpOp::NotIn => bytecode::CMP_NOT_IN,
        CmpOp::Is => bytecode::CMP_IS,
        CmpOp::IsNot => bytecode::CMP_IS_NOT,
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

fn int_const(i: i64) -> Const {
    if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
        Const::Int(i as i32)
    } else {
        Const::Float(i as f64)
    }
}

fn const_as_f64(c: &Const) -> Option<f64> {
    match c {
        Const::Int(i) => Some(*i as f64),
        Const::Float(f) => Some(*f),
        _ => None,
    }
}

fn const_truthy(c: &Const) -> bool {
    match c {
        Const::None => false,
        Const::Bool(b) => *b,
        Const::Int(i) => *i != 0,
        Const::Float(f) => *f != 0.0,
        Const::Str(s) => !s.is_empty(),
    }
}

fn fold_bin(op: BinOp, a: &Const, b: &Const) -> Option<Const> {
    if op == BinOp::Add {
        if let (Const::Str(x), Const::Str(y)) = (a, b) {
            return Some(Const::Str(format!("{x}{y}")));
        }
    }
    if let (Const::Int(x), Const::Int(y)) = (a, b) {
        let x = *x as i64;
        let y = *y as i64;
        match op {
            BinOp::Add => return Some(int_const(x + y)),
            BinOp::Sub => return Some(int_const(x - y)),
            BinOp::Mul => return Some(int_const(x * y)),
            BinOp::FloorDiv => {
                if y == 0 {
                    return None;
                }
                let mut q = x / y;
                if x % y != 0 && (x < 0) != (y < 0) {
                    q -= 1;
                }
                return Some(int_const(q));
            }
            BinOp::Mod => {
                if y == 0 {
                    return None;
                }
                let mut r = x % y;
                if r != 0 && (r < 0) != (y < 0) {
                    r += y;
                }
                return Some(int_const(r));
            }
            BinOp::Div => {}
            _ => return None,
        }
    }
    let (x, y) = (const_as_f64(a)?, const_as_f64(b)?);
    match op {
        BinOp::Add => Some(Const::Float(x + y)),
        BinOp::Sub => Some(Const::Float(x - y)),
        BinOp::Mul => Some(Const::Float(x * y)),
        BinOp::Div => {
            if y == 0.0 {
                return None;
            }
            Some(Const::Float(x / y))
        }
        BinOp::Mod => {
            if y == 0.0 {
                return None;
            }
            let mut r = x % y;
            if r != 0.0 && (r < 0.0) != (y < 0.0) {
                r += y;
            }
            Some(Const::Float(r))
        }
        BinOp::FloorDiv => {
            if y == 0.0 {
                return None;
            }
            Some(Const::Float((x / y).floor()))
        }
        _ => None,
    }
}

fn fold_unary(op: UnOp, v: &Const) -> Option<Const> {
    match op {
        UnOp::Neg => match v {
            Const::Int(i) => Some(int_const(-(*i as i64))),
            Const::Float(f) => Some(Const::Float(-*f)),
            _ => None,
        },
        UnOp::Pos => Some(v.clone()),
        UnOp::Not => Some(Const::Bool(!const_truthy(v))),
        UnOp::Invert => None,
    }
}

fn fold_expr(e: &Expr) -> Option<Const> {
    match e {
        Expr::Int { digits, radix } => {
            let n = i128::from_str_radix(digits, *radix).ok()?;
            if n < i32::MIN as i128 || n > i32::MAX as i128 {
                None
            } else {
                Some(Const::Int(n as i32))
            }
        }
        Expr::Float(f) => Some(Const::Float(*f)),
        Expr::Str(s) => Some(Const::Str(s.clone())),
        Expr::Bool(b) => Some(Const::Bool(*b)),
        Expr::None => Some(Const::None),
        Expr::Unary { op, operand } => fold_unary(*op, &fold_expr(operand)?),
        Expr::Bin { op, left, right } => fold_bin(*op, &fold_expr(left)?, &fold_expr(right)?),
        _ => None,
    }
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
    for module in &mut code.modules {
        assemble(module);
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
    fn lowers_precedence_into_bytecode() {
        let c = code("a + b * c\n");
        let bins: Vec<u32> = c
            .ops
            .iter()
            .filter(|i| i.op == Op::BinaryOp)
            .map(|i| i.arg)
            .collect();
        assert_eq!(bins, vec![bytecode::BIN_MUL, bytecode::BIN_ADD]);
    }

    #[test]
    fn constant_arithmetic_folds_to_one_load() {
        let c = code("1 + 2 * 3\n");
        assert!(!c.ops.iter().any(|i| i.op == Op::BinaryOp));
        assert_eq!(c.ops.iter().filter(|i| i.op == Op::LoadConst).count(), 1);
        assert_eq!(c.consts, vec![Const::Int(7)]);
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
        assert!(err("print(~5)\n").contains("~ operator"));
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
    fn class_lowers_to_a_namespace_function_and_make_class() {
        let c = code("class Point:\n    def norm(self):\n        return self.x\n");
        assert!(c.ops.iter().any(|i| i.op == Op::MakeClass));
        assert!(c.ops.iter().any(|i| i.op == Op::MakeFunction));
        let body = &c.codes[0];
        assert_eq!(body.name, "Point");
        let method = &body.codes[0];
        assert_eq!(method.name, "norm");
        assert_eq!(method.nparams, 1);
        assert!(method.ops.iter().any(|i| i.op == Op::LoadAttr));
        assert!(body.ops.iter().any(|i| i.op == Op::BuildDict));
    }

    #[test]
    fn attribute_load_and_store_emit_opcodes() {
        let c = code("p.x = 1\ny = p.x\n");
        assert!(c.ops.iter().any(|i| i.op == Op::StoreAttr));
        assert!(c.ops.iter().any(|i| i.op == Op::LoadAttr));
    }

    #[test]
    fn multiple_inheritance_is_rejected() {
        assert!(err("class C(A, B):\n    pass\n").contains("multiple inheritance"));
    }

    #[test]
    fn import_of_a_missing_module_defers_to_runtime() {
        let c = code("import no_such_module_xyz\n");
        assert!(c.ops.iter().any(|i| i.op == Op::ImportMissing));
        assert!(c.names.iter().any(|n| n == "no_such_module_xyz"));
        let c = code("from no_such_module_xyz import a\n");
        assert!(c.ops.iter().any(|i| i.op == Op::ImportMissing));
    }

    #[test]
    fn relative_import_without_a_source_dir_errors() {
        assert!(err("from . import x\n").contains("relative import"));
    }

    #[test]
    fn import_binds_names_as_locals_in_a_function() {
        use std::path::PathBuf;
        let dir = std::env::temp_dir().join(format!("gecko-comp-import-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("m.py"), "v = 1\n").unwrap();
        let module = parser::parse("import m\nx = m.v\n").unwrap();
        let c = super::compile_with_base(&module, Some(PathBuf::from(&dir))).unwrap();
        assert!(c.ops.iter().any(|i| i.op == Op::Import));
        assert_eq!(c.modules.len(), 1);
        assert_eq!(c.modules[0].name, "m");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dotted_import_registers_the_package_chain() {
        use std::path::PathBuf;
        let dir = std::env::temp_dir().join(format!("gecko-comp-dotted-{}", std::process::id()));
        let sub = dir.join("a").join("b");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.join("a").join("__init__.py"), "").unwrap();
        std::fs::write(sub.join("__init__.py"), "").unwrap();
        std::fs::write(sub.join("c.py"), "v = 1\n").unwrap();
        let module = parser::parse("import a.b.c\n").unwrap();
        let code = super::compile_with_base(&module, Some(PathBuf::from(&dir))).unwrap();
        let names: Vec<&str> = code.modules.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["a", "a.b", "a.b.c"]);
        assert_eq!(code.modules[0].parent_module, -1);
        assert_eq!(code.modules[1].parent_module, 0);
        assert_eq!(code.modules[2].parent_module, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn decorator_emits_a_call_after_make_function() {
        let c = code("@deco\ndef f():\n    pass\n");
        let make = c.ops.iter().position(|i| i.op == Op::MakeFunction).unwrap();
        let call = c.ops.iter().position(|i| i.op == Op::Call).unwrap();
        assert!(call > make);
        assert_eq!(c.ops[call].arg, 1);
        assert!(c.ops.iter().any(|i| i.op == Op::StoreName));
    }

    #[test]
    fn stacked_decorators_emit_one_call_each() {
        let c = code("@a\n@b\ndef f():\n    pass\n");
        let calls = c
            .ops
            .iter()
            .filter(|i| i.op == Op::Call && i.arg == 1)
            .count();
        assert_eq!(calls, 2);
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
    fn control_flow_through_finally_compiles() {
        for src in [
            "def f():\n    try:\n        return 1\n    finally:\n        pass\n",
            "for i in [1]:\n    try:\n        break\n    finally:\n        pass\n",
            "for i in [1]:\n    try:\n        continue\n    finally:\n        pass\n",
            "for i in [1]:\n    try:\n        for j in [1]:\n            break\n    finally:\n        pass\n",
        ] {
            assert!(compile(&parser::parse(src).unwrap()).is_ok());
        }
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
