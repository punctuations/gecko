//! Setae bytecode: the compiler's output and the runtime's input.
//!
//! `Op` discriminants match the C `SetaeOp` enum in native/include/setae.h.

#[derive(Debug, Clone, PartialEq)]
pub struct Code {
    pub name: String,
    pub consts: Vec<Const>,
    pub names: Vec<String>,
    pub ops: Vec<Instr>,
    pub excs: Vec<ExcEntry>,
    pub nlocals: u32,
    pub nparams: u32,
    pub ncells: u32,
    pub nfrees: u32,
    pub codes: Vec<Code>,
}

/// Exception table entry. Instruction units in [start, end) are protected: an
/// error there cuts the operand stack to depth, pushes the exception, and
/// jumps to target. First matching entry wins, so inner ranges come first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExcEntry {
    pub start: u32,
    pub end: u32,
    pub target: u32,
    pub depth: u32,
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
    MakeFunction = 17,
    BuildList = 18,
    BuildDict = 19,
    Subscr = 20,
    StoreSubscr = 21,
    GetIter = 22,
    ForIter = 23,
    CallMethod = 24,
    ExtendedArg = 25,
    LoadClosure = 26,
    LoadDeref = 27,
    StoreDeref = 28,
    BuildTuple = 29,
    UnpackSequence = 30,
    Raise = 31,
    ExcMatch = 32,
    Reraise = 33,
}

/// `BinaryOp` argument selectors, matching the C `SetaeBinOp` enum.
pub const BIN_ADD: u32 = 0;
pub const BIN_SUB: u32 = 1;
pub const BIN_MUL: u32 = 2;
pub const BIN_DIV: u32 = 3;
pub const BIN_MOD: u32 = 4;
pub const BIN_FLOORDIV: u32 = 5;

/// Set on a `BinaryOp` argument when it came from an augmented assignment, so
/// a type error reports `+=` rather than `+`.
pub const BIN_AUG_FLAG: u32 = 0x80;

/// `CompareOp` argument selectors, matching the C `SetaeCmpOp` enum.
pub const CMP_EQ: u32 = 0;
pub const CMP_NE: u32 = 1;
pub const CMP_LT: u32 = 2;
pub const CMP_LE: u32 = 3;
pub const CMP_GT: u32 = 4;
pub const CMP_GE: u32 = 5;
pub const CMP_IN: u32 = 6;
pub const CMP_NOT_IN: u32 = 7;
