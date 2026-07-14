//! Lowers the AST to Setae bytecode.
//!
//! Only the subset the VM can run is lowered. Anything else is rejected here
//! rather than compiled into something wrong.

use ast::{BinOp, BoolOp, CmpOp, Expr, Module, Stmt, UnOp};
use bytecode::{Code, Const, Instr, Op};

#[derive(Debug, Clone, PartialEq)]
pub struct CompileError {
    pub message: String,
}

pub fn compile(module: &Module) -> Result<Code, CompileError> {
    let mut c = Compiler {
        code: Code {
            consts: Vec::new(),
            names: Vec::new(),
            ops: Vec::new(),
            nlocals: 0,
        },
    };
    for stmt in &module.body {
        c.stmt(stmt)?;
    }
    Ok(c.code)
}

struct Compiler {
    code: Code,
}

fn unsupported<T>(what: &str) -> Result<T, CompileError> {
    Err(CompileError {
        message: format!("{what} not supported yet"),
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
                let Expr::Name(name) = &targets[0] else {
                    return unsupported("this assignment target");
                };
                self.expr(value)?;
                let i = self.name_idx(name);
                self.emit(Op::StoreName, i);
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
            Stmt::FunctionDef { .. } => unsupported("function definitions"),
            Stmt::Return(_) => unsupported("return outside a function"),
            Stmt::For { .. } => unsupported("for loops"),
            Stmt::AugAssign { .. } => unsupported("augmented assignment"),
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
            Expr::Name(name) => {
                let i = self.name_idx(name);
                self.emit(Op::LoadName, i);
            }
            Expr::Call {
                func,
                args,
                keywords,
            } => {
                if !keywords.is_empty() {
                    return unsupported("keyword arguments");
                }
                self.expr(func)?;
                for a in args {
                    self.expr(a)?;
                }
                self.emit(Op::Call, args.len() as u32);
            }
            Expr::Bin { op, left, right } => {
                let sel = match op {
                    BinOp::Add => bytecode::BIN_ADD,
                    BinOp::Sub => bytecode::BIN_SUB,
                    BinOp::Mul => bytecode::BIN_MUL,
                    BinOp::Div => bytecode::BIN_DIV,
                    _ => return unsupported("this operator"),
                };
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
            _ => return unsupported("this expression"),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::compile;
    use bytecode::{Const, Op};

    fn code(src: &str) -> bytecode::Code {
        compile(&parser::parse(src).unwrap()).unwrap()
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
        let err = compile(&parser::parse("def f():\n    pass\n").unwrap()).unwrap_err();
        assert!(err.message.contains("function definitions"));
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
}
