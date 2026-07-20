#[derive(Debug, Clone, PartialEq)]
pub struct Code {
    pub name: String,
    pub consts: Vec<Const>,
    pub names: Vec<String>,
    pub ops: Vec<Instr>,
    pub excs: Vec<ExcEntry>,
    pub nlocals: u32,
    pub nparams: u32,
    pub ndefaults: u32,
    pub ncells: u32,
    pub nfrees: u32,
    pub param_names: Vec<String>,
    pub varargs: bool,
    pub kwargs: bool,
    pub codes: Vec<Code>,
    pub modules: Vec<Code>,
    pub parent_module: i32,
}

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
    LoadAttr = 34,
    StoreAttr = 35,
    MakeClass = 36,
    Import = 37,
    ImportMissing = 38,
    CallEx = 39,
    ListExtend = 40,
    DictMerge = 41,
}

pub const BIN_ADD: u32 = 0;
pub const BIN_SUB: u32 = 1;
pub const BIN_MUL: u32 = 2;
pub const BIN_DIV: u32 = 3;
pub const BIN_MOD: u32 = 4;
pub const BIN_FLOORDIV: u32 = 5;

pub const BIN_AUG_FLAG: u32 = 0x80;

const MAGIC: &[u8; 8] = b"GKBC0001";

pub const FROZEN_TRAILER: &[u8; 8] = b"GECKOFRZ";

pub fn read_frozen(path: &std::path::Path) -> Option<Code> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let size = f.seek(SeekFrom::End(0)).ok()?;
    if size < 16 {
        return None;
    }
    f.seek(SeekFrom::End(-16)).ok()?;
    let mut tail = [0u8; 16];
    f.read_exact(&mut tail).ok()?;
    if &tail[8..] != FROZEN_TRAILER {
        return None;
    }
    let plen = u64::from_le_bytes(tail[..8].try_into().unwrap());
    if plen == 0 || plen > size - 16 {
        return None;
    }
    f.seek(SeekFrom::End(-16 - plen as i64)).ok()?;
    let mut payload = vec![0u8; plen as usize];
    f.read_exact(&mut payload).ok()?;
    from_bytes(&payload).ok()
}

pub fn to_bytes(code: &Code) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    write_code(&mut out, code);
    out
}

fn w32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn wstr(out: &mut Vec<u8>, s: &str) {
    w32(out, s.len() as u32);
    out.extend_from_slice(s.as_bytes());
}

fn write_code(out: &mut Vec<u8>, c: &Code) {
    wstr(out, &c.name);
    w32(out, c.consts.len() as u32);
    for k in &c.consts {
        match k {
            Const::None => out.push(0),
            Const::Bool(b) => {
                out.push(1);
                out.push(*b as u8);
            }
            Const::Int(i) => {
                out.push(2);
                out.extend_from_slice(&i.to_le_bytes());
            }
            Const::Float(f) => {
                out.push(3);
                out.extend_from_slice(&f.to_le_bytes());
            }
            Const::Str(s) => {
                out.push(4);
                wstr(out, s);
            }
        }
    }
    w32(out, c.names.len() as u32);
    for n in &c.names {
        wstr(out, n);
    }
    w32(out, c.ops.len() as u32);
    for i in &c.ops {
        out.push(i.op as u8);
        w32(out, i.arg);
    }
    w32(out, c.excs.len() as u32);
    for e in &c.excs {
        w32(out, e.start);
        w32(out, e.end);
        w32(out, e.target);
        w32(out, e.depth);
    }
    w32(out, c.nlocals);
    w32(out, c.nparams);
    w32(out, c.ndefaults);
    w32(out, c.ncells);
    w32(out, c.nfrees);
    w32(out, c.param_names.len() as u32);
    for n in &c.param_names {
        wstr(out, n);
    }
    out.push(c.varargs as u8);
    out.push(c.kwargs as u8);
    w32(out, c.codes.len() as u32);
    for child in &c.codes {
        write_code(out, child);
    }
    w32(out, c.modules.len() as u32);
    for m in &c.modules {
        write_code(out, m);
    }
    w32(out, c.parent_module as u32);
}

pub fn from_bytes(data: &[u8]) -> Result<Code, String> {
    let mut r = Reader { data, pos: 0 };
    if r.take(8)? != MAGIC {
        return Err("not gecko bytecode, or an incompatible version".into());
    }
    let code = r.code()?;
    if r.pos != data.len() {
        return Err("trailing bytes after the code object".into());
    }
    Ok(code)
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        if self.pos + n > self.data.len() {
            return Err("truncated bytecode".into());
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn str(&mut self) -> Result<String, String> {
        let n = self.u32()? as usize;
        let bytes = self.take(n)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| "invalid string bytes".to_string())
    }

    fn code(&mut self) -> Result<Code, String> {
        let name = self.str()?;
        let mut consts = Vec::new();
        for _ in 0..self.u32()? {
            consts.push(match self.u8()? {
                0 => Const::None,
                1 => Const::Bool(self.u8()? != 0),
                2 => Const::Int(i32::from_le_bytes(self.take(4)?.try_into().unwrap())),
                3 => Const::Float(f64::from_le_bytes(self.take(8)?.try_into().unwrap())),
                4 => Const::Str(self.str()?),
                t => return Err(format!("bad const tag {t}")),
            });
        }
        let mut names = Vec::new();
        for _ in 0..self.u32()? {
            names.push(self.str()?);
        }
        let mut ops = Vec::new();
        for _ in 0..self.u32()? {
            let op = op_from_u8(self.u8()?)?;
            let arg = self.u32()?;
            ops.push(Instr { op, arg });
        }
        let mut excs = Vec::new();
        for _ in 0..self.u32()? {
            excs.push(ExcEntry {
                start: self.u32()?,
                end: self.u32()?,
                target: self.u32()?,
                depth: self.u32()?,
            });
        }
        let nlocals = self.u32()?;
        let nparams = self.u32()?;
        let ndefaults = self.u32()?;
        let ncells = self.u32()?;
        let nfrees = self.u32()?;
        let mut param_names = Vec::new();
        for _ in 0..self.u32()? {
            param_names.push(self.str()?);
        }
        let varargs = self.u8()? != 0;
        let kwargs = self.u8()? != 0;
        let mut codes = Vec::new();
        for _ in 0..self.u32()? {
            codes.push(self.code()?);
        }
        let mut modules = Vec::new();
        for _ in 0..self.u32()? {
            modules.push(self.code()?);
        }
        let parent_module = self.u32()? as i32;
        Ok(Code {
            name,
            consts,
            names,
            ops,
            excs,
            nlocals,
            nparams,
            ndefaults,
            ncells,
            nfrees,
            param_names,
            varargs,
            kwargs,
            codes,
            modules,
            parent_module,
        })
    }
}

fn op_from_u8(v: u8) -> Result<Op, String> {
    Ok(match v {
        0 => Op::LoadConst,
        1 => Op::LoadName,
        2 => Op::StoreName,
        3 => Op::LoadLocal,
        4 => Op::StoreLocal,
        5 => Op::PopTop,
        6 => Op::BinaryOp,
        7 => Op::Call,
        8 => Op::Return,
        9 => Op::Jump,
        10 => Op::PopJumpIfFalse,
        11 => Op::PopJumpIfTrue,
        12 => Op::JumpIfFalseOrPop,
        13 => Op::JumpIfTrueOrPop,
        14 => Op::CompareOp,
        15 => Op::UnaryNeg,
        16 => Op::UnaryNot,
        17 => Op::MakeFunction,
        18 => Op::BuildList,
        19 => Op::BuildDict,
        20 => Op::Subscr,
        21 => Op::StoreSubscr,
        22 => Op::GetIter,
        23 => Op::ForIter,
        24 => Op::CallMethod,
        25 => Op::ExtendedArg,
        26 => Op::LoadClosure,
        27 => Op::LoadDeref,
        28 => Op::StoreDeref,
        29 => Op::BuildTuple,
        30 => Op::UnpackSequence,
        31 => Op::Raise,
        32 => Op::ExcMatch,
        33 => Op::Reraise,
        34 => Op::LoadAttr,
        35 => Op::StoreAttr,
        36 => Op::MakeClass,
        37 => Op::Import,
        38 => Op::ImportMissing,
        39 => Op::CallEx,
        40 => Op::ListExtend,
        41 => Op::DictMerge,
        _ => return Err(format!("bad opcode {v}")),
    })
}

pub const CMP_EQ: u32 = 0;
pub const CMP_NE: u32 = 1;
pub const CMP_LT: u32 = 2;
pub const CMP_LE: u32 = 3;
pub const CMP_GT: u32 = 4;
pub const CMP_GE: u32 = 5;
pub const CMP_IN: u32 = 6;
pub const CMP_NOT_IN: u32 = 7;
pub const CMP_IS: u32 = 8;
pub const CMP_IS_NOT: u32 = 9;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_objects_roundtrip() {
        let inner = Code {
            name: "inner".into(),
            consts: vec![Const::None, Const::Bool(true), Const::Float(2.5)],
            names: vec!["print".into()],
            ops: vec![
                Instr {
                    op: Op::LoadDeref,
                    arg: 0,
                },
                Instr {
                    op: Op::Return,
                    arg: 0,
                },
            ],
            excs: Vec::new(),
            nlocals: 0,
            nparams: 0,
            ndefaults: 0,
            ncells: 0,
            nfrees: 1,
            param_names: Vec::new(),
            varargs: false,
            kwargs: false,
            codes: Vec::new(),
            modules: Vec::new(),
            parent_module: -1,
        };
        let submodule = Code {
            name: "helpers".into(),
            consts: vec![Const::Int(1)],
            names: vec!["value".into()],
            ops: vec![
                Instr {
                    op: Op::LoadConst,
                    arg: 0,
                },
                Instr {
                    op: Op::StoreName,
                    arg: 0,
                },
            ],
            excs: Vec::new(),
            nlocals: 0,
            nparams: 0,
            ndefaults: 0,
            ncells: 0,
            nfrees: 0,
            param_names: Vec::new(),
            varargs: false,
            kwargs: false,
            codes: Vec::new(),
            modules: Vec::new(),
            parent_module: 3,
        };
        let outer = Code {
            name: "<module>".into(),
            consts: vec![Const::Int(-7), Const::Str("hi \u{e9}".into())],
            names: vec!["x".into(), "y".into()],
            ops: vec![
                Instr {
                    op: Op::Import,
                    arg: 0,
                },
                Instr {
                    op: Op::MakeFunction,
                    arg: 0,
                },
                Instr {
                    op: Op::ExtendedArg,
                    arg: 1,
                },
                Instr {
                    op: Op::LoadConst,
                    arg: 44,
                },
            ],
            excs: vec![ExcEntry {
                start: 0,
                end: 2,
                target: 2,
                depth: 1,
            }],
            nlocals: 3,
            nparams: 1,
            ndefaults: 0,
            ncells: 2,
            nfrees: 0,
            param_names: Vec::new(),
            varargs: false,
            kwargs: false,
            codes: vec![inner],
            modules: vec![submodule],
            parent_module: -1,
        };
        let bytes = to_bytes(&outer);
        assert_eq!(from_bytes(&bytes).unwrap(), outer);
    }

    #[test]
    fn bad_input_is_rejected() {
        assert!(from_bytes(b"nonsense").is_err());
        assert!(from_bytes(b"GKBC0001").is_err());
        let mut good = to_bytes(&Code {
            name: "m".into(),
            consts: Vec::new(),
            names: Vec::new(),
            ops: Vec::new(),
            excs: Vec::new(),
            nlocals: 0,
            nparams: 0,
            ndefaults: 0,
            ncells: 0,
            nfrees: 0,
            param_names: Vec::new(),
            varargs: false,
            kwargs: false,
            codes: Vec::new(),
            modules: Vec::new(),
            parent_module: -1,
        });
        good.push(0);
        assert!(from_bytes(&good).is_err());
    }
}
