//! Setae bytecode: the compiler's output and the runtime's input.
//!
//! `Op` discriminants match the C `GkOp` enum in native/include/gecko.h.

#[derive(Debug, Clone, PartialEq)]
pub struct Code {
    pub consts: Vec<Const>,
    pub names: Vec<String>,
    pub ops: Vec<Instr>,
    pub nlocals: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Const {
    None,
    Bool(bool),
    Int(i32),
    Float(f64),
    Str(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Instr {
    pub op: Op,
    pub arg: u32,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    LoadConst = 0,
    LoadName = 1,
    StoreName = 2,
    LoadLocal = 3,
    StoreLocal = 4,
    PopTop = 5,
    BinaryOp = 6,
    Call = 7,
    Return = 8,
    Jump = 9,
    PopJumpIfFalse = 10,
    PopJumpIfTrue = 11,
    JumpIfFalseOrPop = 12,
    JumpIfTrueOrPop = 13,
    CompareOp = 14,
    UnaryNeg = 15,
    UnaryNot = 16,
}

/// `BinaryOp` argument selectors, matching the C `GkBinOp` enum.
pub const BIN_ADD: u32 = 0;
pub const BIN_SUB: u32 = 1;
pub const BIN_MUL: u32 = 2;
pub const BIN_DIV: u32 = 3;

/// `CompareOp` argument selectors, matching the C `GkCmpOp` enum.
pub const CMP_EQ: u32 = 0;
pub const CMP_NE: u32 = 1;
pub const CMP_LT: u32 = 2;
pub const CMP_LE: u32 = 3;
pub const CMP_GT: u32 = 4;
pub const CMP_GE: u32 = 5;
