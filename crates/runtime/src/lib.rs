use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

pub type SetaeValue = u64;

pub enum SetaeHeap {}
pub enum SetaeVm {}
pub enum SetaeCode {}

pub const OP_LOAD_CONST: u8 = 0;
pub const OP_LOAD_NAME: u8 = 1;
pub const OP_STORE_NAME: u8 = 2;
pub const OP_LOAD_LOCAL: u8 = 3;
pub const OP_STORE_LOCAL: u8 = 4;
pub const OP_POP_TOP: u8 = 5;
pub const OP_BINARY_OP: u8 = 6;
pub const OP_CALL: u8 = 7;
pub const OP_RETURN: u8 = 8;
pub const OP_JUMP: u8 = 9;
pub const OP_POP_JUMP_IF_FALSE: u8 = 10;
pub const OP_POP_JUMP_IF_TRUE: u8 = 11;
pub const OP_JUMP_IF_FALSE_OR_POP: u8 = 12;
pub const OP_JUMP_IF_TRUE_OR_POP: u8 = 13;
pub const OP_COMPARE_OP: u8 = 14;
pub const OP_UNARY_NEG: u8 = 15;
pub const OP_UNARY_NOT: u8 = 16;
pub const OP_MAKE_FUNCTION: u8 = 17;
pub const OP_BUILD_LIST: u8 = 18;
pub const OP_BUILD_DICT: u8 = 19;
pub const OP_SUBSCR: u8 = 20;
pub const OP_STORE_SUBSCR: u8 = 21;
pub const OP_GET_ITER: u8 = 22;
pub const OP_FOR_ITER: u8 = 23;
pub const OP_CALL_METHOD: u8 = 24;
pub const OP_EXTENDED_ARG: u8 = 25;
pub const OP_LOAD_CLOSURE: u8 = 26;
pub const OP_LOAD_DEREF: u8 = 27;
pub const OP_STORE_DEREF: u8 = 28;

pub const BIN_ADD: u8 = 0;
pub const BIN_SUB: u8 = 1;
pub const BIN_MUL: u8 = 2;
pub const BIN_DIV: u8 = 3;
pub const BIN_MOD: u8 = 4;
pub const BIN_FLOORDIV: u8 = 5;

unsafe extern "C" {
    pub fn setae_is_float(v: SetaeValue) -> c_int;
    pub fn setae_is_int(v: SetaeValue) -> c_int;
    pub fn setae_is_ptr(v: SetaeValue) -> c_int;
    pub fn setae_is_none(v: SetaeValue) -> c_int;
    pub fn setae_is_bool(v: SetaeValue) -> c_int;

    pub fn setae_from_float(d: f64) -> SetaeValue;
    pub fn setae_to_float(v: SetaeValue) -> f64;
    pub fn setae_from_int(i: i32) -> SetaeValue;
    pub fn setae_to_int(v: SetaeValue) -> i32;

    pub fn setae_none() -> SetaeValue;
    pub fn setae_bool(b: c_int) -> SetaeValue;
    pub fn setae_to_bool(v: SetaeValue) -> c_int;

    pub fn setae_from_ptr(p: *mut std::ffi::c_void) -> SetaeValue;
    pub fn setae_to_ptr(v: SetaeValue) -> *mut std::ffi::c_void;

    pub fn setae_heap_new() -> *mut SetaeHeap;
    pub fn setae_heap_destroy(h: *mut SetaeHeap);
    pub fn setae_heap_live(h: *const SetaeHeap) -> usize;
    pub fn setae_gc_collect(vm: *mut SetaeVm);
    pub fn setae_str_new(h: *mut SetaeHeap, bytes: *const c_char, len: usize) -> SetaeValue;

    pub fn setae_code_new() -> *mut SetaeCode;
    pub fn setae_code_free(c: *mut SetaeCode);
    pub fn setae_code_new_child(parent: *mut SetaeCode) -> *mut SetaeCode;
    pub fn setae_code_add_const(c: *mut SetaeCode, v: SetaeValue) -> u32;
    pub fn setae_code_add_name(c: *mut SetaeCode, name: *const c_char) -> u32;
    pub fn setae_code_emit(c: *mut SetaeCode, op: u8, arg: u8);
    pub fn setae_code_set_nlocals(c: *mut SetaeCode, n: u32);
    pub fn setae_code_set_nparams(c: *mut SetaeCode, n: u32);
    pub fn setae_code_set_ncells(c: *mut SetaeCode, n: u32);
    pub fn setae_code_set_nfrees(c: *mut SetaeCode, n: u32);
    pub fn setae_code_add_exc(c: *mut SetaeCode, start: u32, end: u32, target: u32, depth: u32);
    pub fn setae_code_set_name(c: *mut SetaeCode, name: *const c_char);

    pub fn setae_vm_new(h: *mut SetaeHeap) -> *mut SetaeVm;
    pub fn setae_vm_destroy(vm: *mut SetaeVm);
    pub fn setae_vm_register_builtins(vm: *mut SetaeVm);
    pub fn setae_vm_set_global(vm: *mut SetaeVm, name: *const c_char, v: SetaeValue);
    pub fn setae_vm_run(vm: *mut SetaeVm, code: *mut SetaeCode) -> SetaeValue;
    pub fn setae_vm_error(vm: *mut SetaeVm) -> c_int;
    pub fn setae_vm_error_msg(vm: *mut SetaeVm) -> *const c_char;
    pub fn setae_vm_output(vm: *mut SetaeVm, len: *mut usize) -> *const c_char;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_roundtrips() {
        for d in [
            0.0,
            -0.0,
            1.0,
            -1.5,
            1e300,
            5e-324,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            let v = unsafe { setae_from_float(d) };
            assert_eq!(unsafe { setae_is_float(v) }, 1, "is_float {d}");
            assert_eq!(unsafe { setae_is_int(v) }, 0, "not int {d}");
            assert_eq!(unsafe { setae_to_float(v) }, d, "roundtrip {d}");
        }
    }

    #[test]
    fn nan_roundtrips_as_nan() {
        let v = unsafe { setae_from_float(f64::NAN) };
        assert_eq!(unsafe { setae_is_float(v) }, 1);
        assert!(unsafe { setae_to_float(v) }.is_nan());
    }

    #[test]
    fn int_roundtrips() {
        for i in [0, 1, -1, i32::MAX, i32::MIN, 123456] {
            let v = unsafe { setae_from_int(i) };
            assert_eq!(unsafe { setae_is_int(v) }, 1, "is_int {i}");
            assert_eq!(unsafe { setae_is_float(v) }, 0, "not float {i}");
            assert_eq!(unsafe { setae_to_int(v) }, i, "roundtrip {i}");
        }
    }

    #[test]
    fn singletons_are_distinct() {
        let (n, t, f) = unsafe { (setae_none(), setae_bool(1), setae_bool(0)) };
        assert_eq!(unsafe { setae_is_none(n) }, 1);
        assert_eq!(unsafe { setae_is_bool(t) }, 1);
        assert_eq!(unsafe { setae_is_bool(f) }, 1);
        assert_eq!(unsafe { setae_to_bool(t) }, 1);
        assert_eq!(unsafe { setae_to_bool(f) }, 0);
        assert_ne!(n, t);
        assert_ne!(t, f);
        assert_eq!(unsafe { setae_is_bool(n) }, 0);
        assert_eq!(unsafe { setae_is_none(t) }, 0);
    }

    #[test]
    fn pointer_roundtrips() {
        let mut boxed = Box::new(42u64);
        let p = (&mut *boxed) as *mut u64 as *mut std::ffi::c_void;
        let v = unsafe { setae_from_ptr(p) };
        assert_eq!(unsafe { setae_is_ptr(v) }, 1);
        assert_eq!(unsafe { setae_is_float(v) }, 0);
        assert_eq!(unsafe { setae_is_int(v) }, 0);
        assert_eq!(unsafe { setae_to_ptr(v) }, p);
    }
}

pub struct Vm {
    heap: *mut SetaeHeap,
    vm: *mut SetaeVm,
    codes: Vec<*mut SetaeCode>,
}

pub struct Run {
    pub result: SetaeValue,
    pub output: String,
    pub error: bool,
    pub message: String,
}

fn args_fit(code: &bytecode::Code) -> bool {
    code.ops.iter().all(|i| i.arg <= u8::MAX as u32) && code.codes.iter().all(args_fit)
}

impl Vm {
    pub fn new() -> Self {
        unsafe {
            let heap = setae_heap_new();
            let vm = setae_vm_new(heap);
            setae_vm_register_builtins(vm);
            Vm {
                heap,
                vm,
                codes: Vec::new(),
            }
        }
    }

    pub fn run(&mut self, code: &bytecode::Code) -> Run {
        if !args_fit(code) {
            return Run {
                result: unsafe { setae_none() },
                output: String::new(),
                error: true,
                message: "argument does not fit one byte".into(),
            };
        }
        unsafe {
            let gc = setae_code_new();
            self.lower(gc, code);
            self.codes.push(gc);

            let result = setae_vm_run(self.vm, gc);
            let error = setae_vm_error(self.vm) != 0;
            let message = if error {
                CStr::from_ptr(setae_vm_error_msg(self.vm))
                    .to_string_lossy()
                    .into_owned()
            } else {
                String::new()
            };

            let mut len = 0usize;
            let ptr = setae_vm_output(self.vm, &mut len);
            let output = if ptr.is_null() || len == 0 {
                String::new()
            } else {
                let bytes = std::slice::from_raw_parts(ptr as *const u8, len);
                String::from_utf8_lossy(bytes).into_owned()
            };

            Run {
                result,
                output,
                error,
                message,
            }
        }
    }

    pub fn heap_live(&self) -> usize {
        unsafe { setae_heap_live(self.heap) }
    }

    pub fn collect(&mut self) {
        unsafe { setae_gc_collect(self.vm) }
    }

    unsafe fn lower(&mut self, gc: *mut SetaeCode, code: &bytecode::Code) {
        unsafe {
            for c in &code.consts {
                let v = match c {
                    bytecode::Const::None => setae_none(),
                    bytecode::Const::Bool(b) => setae_bool(*b as c_int),
                    bytecode::Const::Int(i) => setae_from_int(*i),
                    bytecode::Const::Float(f) => setae_from_float(*f),
                    bytecode::Const::Str(s) => {
                        setae_str_new(self.heap, s.as_ptr() as *const c_char, s.len())
                    }
                };
                setae_code_add_const(gc, v);
            }
            for name in &code.names {
                let cs = CString::new(name.as_str()).expect("name has no interior NUL");
                setae_code_add_name(gc, cs.as_ptr());
            }
            for instr in &code.ops {
                setae_code_emit(gc, instr.op as u8, instr.arg as u8);
            }
            for e in &code.excs {
                setae_code_add_exc(gc, e.start, e.end, e.target, e.depth);
            }
            setae_code_set_nlocals(gc, code.nlocals);
            setae_code_set_nparams(gc, code.nparams);
            setae_code_set_ncells(gc, code.ncells);
            setae_code_set_nfrees(gc, code.nfrees);
            let cs = CString::new(code.name.as_str()).expect("name has no interior NUL");
            setae_code_set_name(gc, cs.as_ptr());
            for child in &code.codes {
                let cgc = setae_code_new_child(gc);
                self.lower(cgc, child);
            }
        }
    }
}

impl Default for Vm {
    fn default() -> Self {
        Vm::new()
    }
}

impl Drop for Vm {
    fn drop(&mut self) {
        unsafe {
            for gc in &self.codes {
                setae_code_free(*gc);
            }
            setae_vm_destroy(self.vm);
            setae_heap_destroy(self.heap);
        }
    }
}

#[cfg(test)]
mod machine_tests {
    use super::*;
    use bytecode::{Code, Const, Instr, Op};

    fn blank(nlocals: u32, nparams: u32, ncells: u32, nfrees: u32) -> Code {
        Code {
            name: "test".into(),
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

    fn ins(op: Op, arg: u32) -> Instr {
        Instr { op, arg }
    }

    fn int_result(run: &Run) -> i32 {
        assert!(!run.error, "{}", run.message);
        assert_eq!(unsafe { setae_is_int(run.result) }, 1);
        unsafe { setae_to_int(run.result) }
    }

    #[test]
    fn calls_a_function_through_make_function() {
        let mut add = blank(2, 2, 0, 0);
        add.ops = vec![
            ins(Op::LoadLocal, 0),
            ins(Op::LoadLocal, 1),
            ins(Op::BinaryOp, bytecode::BIN_ADD),
            ins(Op::Return, 0),
        ];
        let mut m = blank(0, 0, 0, 0);
        m.consts = vec![Const::Int(2), Const::Int(40)];
        m.codes = vec![add];
        m.ops = vec![
            ins(Op::MakeFunction, 0),
            ins(Op::LoadConst, 0),
            ins(Op::LoadConst, 1),
            ins(Op::Call, 2),
            ins(Op::Return, 0),
        ];
        let mut vm = Vm::new();
        let run = vm.run(&m);
        assert_eq!(int_result(&run), 42);
    }

    #[test]
    fn a_callee_writes_back_through_a_captured_cell() {
        let mut inner = blank(0, 0, 0, 1);
        inner.consts = vec![Const::Int(9)];
        inner.ops = vec![
            ins(Op::LoadConst, 0),
            ins(Op::StoreDeref, 0),
            ins(Op::LoadConst, 0),
            ins(Op::Return, 0),
        ];
        let mut outer = blank(0, 0, 1, 0);
        outer.codes = vec![inner];
        outer.ops = vec![
            ins(Op::LoadClosure, 0),
            ins(Op::MakeFunction, 0),
            ins(Op::Call, 0),
            ins(Op::PopTop, 0),
            ins(Op::LoadDeref, 0),
            ins(Op::Return, 0),
        ];
        let mut m = blank(0, 0, 0, 0);
        m.codes = vec![outer];
        m.ops = vec![
            ins(Op::MakeFunction, 0),
            ins(Op::Call, 0),
            ins(Op::Return, 0),
        ];
        let mut vm = Vm::new();
        let run = vm.run(&m);
        assert_eq!(int_result(&run), 9);
    }

    #[test]
    fn cells_flow_from_store_deref_to_a_capturing_callee() {
        let mut inner = blank(0, 0, 0, 1);
        inner.ops = vec![ins(Op::LoadDeref, 0), ins(Op::Return, 0)];
        let mut outer = blank(0, 0, 1, 0);
        outer.consts = vec![Const::Int(5)];
        outer.codes = vec![inner];
        outer.ops = vec![
            ins(Op::LoadConst, 0),
            ins(Op::StoreDeref, 0),
            ins(Op::LoadClosure, 0),
            ins(Op::MakeFunction, 0),
            ins(Op::Call, 0),
            ins(Op::Return, 0),
        ];
        let mut m = blank(0, 0, 0, 0);
        m.codes = vec![outer];
        m.ops = vec![
            ins(Op::MakeFunction, 0),
            ins(Op::Call, 0),
            ins(Op::Return, 0),
        ];
        let mut vm = Vm::new();
        let run = vm.run(&m);
        assert_eq!(int_result(&run), 5);
    }

    #[test]
    fn builds_a_list_and_subscripts_it() {
        let mut m = blank(0, 0, 0, 0);
        m.consts = vec![Const::Int(10), Const::Int(20), Const::Int(1)];
        m.ops = vec![
            ins(Op::LoadConst, 0),
            ins(Op::LoadConst, 1),
            ins(Op::BuildList, 2),
            ins(Op::LoadConst, 2),
            ins(Op::Subscr, 0),
            ins(Op::Return, 0),
        ];
        let mut vm = Vm::new();
        let run = vm.run(&m);
        assert_eq!(int_result(&run), 20);
    }

    #[test]
    fn for_iter_walks_a_list() {
        let mut m = blank(2, 0, 0, 0);
        m.consts = vec![Const::Int(0), Const::Int(3), Const::Int(4), Const::Int(5)];
        m.ops = vec![
            ins(Op::LoadConst, 0),
            ins(Op::StoreLocal, 0),
            ins(Op::LoadConst, 1),
            ins(Op::LoadConst, 2),
            ins(Op::LoadConst, 3),
            ins(Op::BuildList, 3),
            ins(Op::GetIter, 0),
            ins(Op::ForIter, 14),
            ins(Op::StoreLocal, 1),
            ins(Op::LoadLocal, 0),
            ins(Op::LoadLocal, 1),
            ins(Op::BinaryOp, bytecode::BIN_ADD),
            ins(Op::StoreLocal, 0),
            ins(Op::Jump, 7),
            ins(Op::LoadLocal, 0),
            ins(Op::Return, 0),
        ];
        let mut vm = Vm::new();
        let run = vm.run(&m);
        assert_eq!(int_result(&run), 12);
    }

    #[test]
    fn tuples_build_and_unpack_in_order() {
        let mut m = blank(2, 0, 0, 0);
        m.consts = vec![Const::Int(7), Const::Int(8)];
        m.ops = vec![
            ins(Op::LoadConst, 0),
            ins(Op::LoadConst, 1),
            ins(Op::BuildTuple, 2),
            ins(Op::UnpackSequence, 2),
            ins(Op::StoreLocal, 0),
            ins(Op::StoreLocal, 1),
            ins(Op::LoadLocal, 0),
            ins(Op::LoadLocal, 1),
            ins(Op::BinaryOp, bytecode::BIN_SUB),
            ins(Op::Return, 0),
        ];
        let mut vm = Vm::new();
        let run = vm.run(&m);
        assert_eq!(int_result(&run), -1);
    }

    #[test]
    fn extended_arg_reaches_a_high_const() {
        let mut m = blank(0, 0, 0, 0);
        for i in 0..300 {
            m.consts.push(Const::Int(i));
        }
        m.ops = vec![
            ins(Op::ExtendedArg, 1),
            ins(Op::LoadConst, 0x2b),
            ins(Op::Return, 0),
        ];
        let mut vm = Vm::new();
        let run = vm.run(&m);
        assert_eq!(int_result(&run), 299);
    }

    #[test]
    fn the_exception_table_unwinds_and_recovers() {
        let mut m = blank(0, 0, 0, 0);
        m.consts = vec![Const::Int(1), Const::Int(0), Const::Int(7)];
        m.ops = vec![
            ins(Op::LoadConst, 0),
            ins(Op::LoadConst, 1),
            ins(Op::BinaryOp, bytecode::BIN_DIV),
            ins(Op::Return, 0),
            ins(Op::PopTop, 0),
            ins(Op::LoadConst, 2),
            ins(Op::Return, 0),
        ];
        m.excs = vec![bytecode::ExcEntry {
            start: 0,
            end: 4,
            target: 4,
            depth: 0,
        }];
        let mut vm = Vm::new();
        let run = vm.run(&m);
        assert_eq!(int_result(&run), 7);
    }

    #[test]
    fn calling_an_int_reports_a_type_error() {
        let mut m = blank(0, 0, 0, 0);
        m.consts = vec![Const::Int(1)];
        m.ops = vec![
            ins(Op::LoadConst, 0),
            ins(Op::LoadConst, 0),
            ins(Op::Call, 1),
        ];
        let mut vm = Vm::new();
        let run = vm.run(&m);
        assert!(run.error);
        assert!(run.message.contains("not callable"), "{}", run.message);
    }

    #[test]
    fn a_hot_loop_of_garbage_gets_collected() {
        let mut m = blank(1, 0, 0, 0);
        m.consts = vec![
            Const::Int(3000),
            Const::Str("a".into()),
            Const::Str("b".into()),
            Const::Int(1),
        ];
        m.ops = vec![
            ins(Op::LoadConst, 0),
            ins(Op::StoreLocal, 0),
            ins(Op::LoadLocal, 0),
            ins(Op::PopJumpIfFalse, 13),
            ins(Op::LoadConst, 1),
            ins(Op::LoadConst, 2),
            ins(Op::BinaryOp, bytecode::BIN_ADD),
            ins(Op::PopTop, 0),
            ins(Op::LoadLocal, 0),
            ins(Op::LoadConst, 3),
            ins(Op::BinaryOp, bytecode::BIN_SUB),
            ins(Op::StoreLocal, 0),
            ins(Op::Jump, 2),
        ];
        let mut vm = Vm::new();
        let run = vm.run(&m);
        assert!(!run.error, "{}", run.message);
        assert!(
            vm.heap_live() < 1500,
            "heap has {} live objects",
            vm.heap_live()
        );
    }
}

#[cfg(test)]
mod vm_tests {
    use super::*;

    #[test]
    fn runs_print_hello_world() {
        unsafe {
            let heap = setae_heap_new();
            let vm = setae_vm_new(heap);
            setae_vm_register_builtins(vm);

            let code = setae_code_new();
            let msg = "hello world";
            let s = setae_str_new(heap, msg.as_ptr() as *const c_char, msg.len());
            let c0 = setae_code_add_const(code, s);
            let name = setae_code_add_name(code, c"print".as_ptr());

            setae_code_emit(code, OP_LOAD_NAME, name as u8);
            setae_code_emit(code, OP_LOAD_CONST, c0 as u8);
            setae_code_emit(code, OP_CALL, 1);
            setae_code_emit(code, OP_POP_TOP, 0);

            setae_vm_run(vm, code);
            assert_eq!(setae_vm_error(vm), 0);

            let mut len = 0usize;
            let ptr = setae_vm_output(vm, &mut len);
            let out = std::slice::from_raw_parts(ptr as *const u8, len);
            assert_eq!(out, b"hello world\n");

            setae_code_free(code);
            setae_vm_destroy(vm);
            setae_heap_destroy(heap);
        }
    }

    #[test]
    fn evaluates_arithmetic() {
        unsafe {
            let heap = setae_heap_new();
            let vm = setae_vm_new(heap);

            let code = setae_code_new();
            let a = setae_code_add_const(code, setae_from_int(2));
            let b = setae_code_add_const(code, setae_from_int(3));
            setae_code_emit(code, OP_LOAD_CONST, a as u8);
            setae_code_emit(code, OP_LOAD_CONST, b as u8);
            setae_code_emit(code, OP_BINARY_OP, BIN_MUL);
            setae_code_emit(code, OP_RETURN, 0);

            let r = setae_vm_run(vm, code);
            assert_eq!(setae_vm_error(vm), 0);
            assert_eq!(setae_is_int(r), 1);
            assert_eq!(setae_to_int(r), 6);

            setae_code_free(code);
            setae_vm_destroy(vm);
            setae_heap_destroy(heap);
        }
    }
}
