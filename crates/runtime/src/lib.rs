//! FFI bindings to the Gecko C runtime in native/.

use std::ffi::CString;
use std::os::raw::{c_char, c_int};

/// NaN-boxed value word. See docs/design/01-object-model.md.
pub type GkValue = u64;

pub enum GkHeap {}
pub enum GkVm {}
pub enum GkCode {}

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

pub const BIN_ADD: u8 = 0;
pub const BIN_SUB: u8 = 1;
pub const BIN_MUL: u8 = 2;
pub const BIN_DIV: u8 = 3;

unsafe extern "C" {
    pub fn gk_is_float(v: GkValue) -> c_int;
    pub fn gk_is_int(v: GkValue) -> c_int;
    pub fn gk_is_ptr(v: GkValue) -> c_int;
    pub fn gk_is_none(v: GkValue) -> c_int;
    pub fn gk_is_bool(v: GkValue) -> c_int;

    pub fn gk_from_float(d: f64) -> GkValue;
    pub fn gk_to_float(v: GkValue) -> f64;
    pub fn gk_from_int(i: i32) -> GkValue;
    pub fn gk_to_int(v: GkValue) -> i32;

    pub fn gk_none() -> GkValue;
    pub fn gk_bool(b: c_int) -> GkValue;
    pub fn gk_to_bool(v: GkValue) -> c_int;

    pub fn gk_from_ptr(p: *mut std::ffi::c_void) -> GkValue;
    pub fn gk_to_ptr(v: GkValue) -> *mut std::ffi::c_void;

    pub fn gk_heap_new() -> *mut GkHeap;
    pub fn gk_heap_destroy(h: *mut GkHeap);
    pub fn gk_str_new(h: *mut GkHeap, bytes: *const c_char, len: usize) -> GkValue;

    pub fn gk_code_new() -> *mut GkCode;
    pub fn gk_code_free(c: *mut GkCode);
    pub fn gk_code_add_const(c: *mut GkCode, v: GkValue) -> u32;
    pub fn gk_code_add_name(c: *mut GkCode, name: *const c_char) -> u32;
    pub fn gk_code_emit(c: *mut GkCode, op: u8, arg: u8);
    pub fn gk_code_set_nlocals(c: *mut GkCode, n: u32);

    pub fn gk_vm_new(h: *mut GkHeap) -> *mut GkVm;
    pub fn gk_vm_destroy(vm: *mut GkVm);
    pub fn gk_vm_register_builtins(vm: *mut GkVm);
    pub fn gk_vm_set_global(vm: *mut GkVm, name: *const c_char, v: GkValue);
    pub fn gk_vm_run(vm: *mut GkVm, code: *mut GkCode) -> GkValue;
    pub fn gk_vm_error(vm: *mut GkVm) -> c_int;
    pub fn gk_vm_output(vm: *mut GkVm, len: *mut usize) -> *const c_char;
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
            let v = unsafe { gk_from_float(d) };
            assert_eq!(unsafe { gk_is_float(v) }, 1, "is_float {d}");
            assert_eq!(unsafe { gk_is_int(v) }, 0, "not int {d}");
            assert_eq!(unsafe { gk_to_float(v) }, d, "roundtrip {d}");
        }
    }

    #[test]
    fn nan_roundtrips_as_nan() {
        let v = unsafe { gk_from_float(f64::NAN) };
        assert_eq!(unsafe { gk_is_float(v) }, 1);
        assert!(unsafe { gk_to_float(v) }.is_nan());
    }

    #[test]
    fn int_roundtrips() {
        for i in [0, 1, -1, i32::MAX, i32::MIN, 123456] {
            let v = unsafe { gk_from_int(i) };
            assert_eq!(unsafe { gk_is_int(v) }, 1, "is_int {i}");
            assert_eq!(unsafe { gk_is_float(v) }, 0, "not float {i}");
            assert_eq!(unsafe { gk_to_int(v) }, i, "roundtrip {i}");
        }
    }

    #[test]
    fn singletons_are_distinct() {
        let (n, t, f) = unsafe { (gk_none(), gk_bool(1), gk_bool(0)) };
        assert_eq!(unsafe { gk_is_none(n) }, 1);
        assert_eq!(unsafe { gk_is_bool(t) }, 1);
        assert_eq!(unsafe { gk_is_bool(f) }, 1);
        assert_eq!(unsafe { gk_to_bool(t) }, 1);
        assert_eq!(unsafe { gk_to_bool(f) }, 0);
        assert_ne!(n, t);
        assert_ne!(t, f);
        // None is not a bool, a bool is not None.
        assert_eq!(unsafe { gk_is_bool(n) }, 0);
        assert_eq!(unsafe { gk_is_none(t) }, 0);
    }

    #[test]
    fn pointer_roundtrips() {
        let mut boxed = Box::new(42u64);
        let p = (&mut *boxed) as *mut u64 as *mut std::ffi::c_void;
        let v = unsafe { gk_from_ptr(p) };
        assert_eq!(unsafe { gk_is_ptr(v) }, 1);
        assert_eq!(unsafe { gk_is_float(v) }, 0);
        assert_eq!(unsafe { gk_is_int(v) }, 0);
        assert_eq!(unsafe { gk_to_ptr(v) }, p);
    }
}

/// A Setae runtime instance. One isolate, meaning a heap and a VM, owning its
/// objects.
pub struct Vm {
    heap: *mut GkHeap,
    vm: *mut GkVm,
}

/// Result of running a code object.
pub struct Run {
    pub result: GkValue,
    pub output: String,
    pub error: bool,
}

impl Vm {
    pub fn new() -> Self {
        unsafe {
            let heap = gk_heap_new();
            let vm = gk_vm_new(heap);
            gk_vm_register_builtins(vm);
            Vm { heap, vm }
        }
    }

    pub fn run(&mut self, code: &bytecode::Code) -> Run {
        // Without EXTENDED_ARG an instruction argument is one byte, so jump
        // targets and pool indices cannot go past 255.
        if code.ops.iter().any(|i| i.arg > u8::MAX as u32) {
            return Run {
                result: unsafe { gk_none() },
                output: String::new(),
                error: true,
            };
        }
        unsafe {
            let gc = gk_code_new();
            for c in &code.consts {
                let v = match c {
                    bytecode::Const::None => gk_none(),
                    bytecode::Const::Bool(b) => gk_bool(*b as c_int),
                    bytecode::Const::Int(i) => gk_from_int(*i),
                    bytecode::Const::Float(f) => gk_from_float(*f),
                    bytecode::Const::Str(s) => {
                        gk_str_new(self.heap, s.as_ptr() as *const c_char, s.len())
                    }
                };
                gk_code_add_const(gc, v);
            }
            for name in &code.names {
                let cs = CString::new(name.as_str()).expect("name has no interior NUL");
                gk_code_add_name(gc, cs.as_ptr());
            }
            for instr in &code.ops {
                gk_code_emit(gc, instr.op as u8, instr.arg as u8);
            }
            gk_code_set_nlocals(gc, code.nlocals);

            let result = gk_vm_run(self.vm, gc);
            let error = gk_vm_error(self.vm) != 0;

            let mut len = 0usize;
            let ptr = gk_vm_output(self.vm, &mut len);
            let output = if ptr.is_null() || len == 0 {
                String::new()
            } else {
                let bytes = std::slice::from_raw_parts(ptr as *const u8, len);
                String::from_utf8_lossy(bytes).into_owned()
            };

            gk_code_free(gc);
            Run {
                result,
                output,
                error,
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
            gk_vm_destroy(self.vm);
            gk_heap_destroy(self.heap);
        }
    }
}

#[cfg(test)]
mod vm_tests {
    use super::*;

    #[test]
    fn runs_print_hello_world() {
        unsafe {
            let heap = gk_heap_new();
            let vm = gk_vm_new(heap);
            gk_vm_register_builtins(vm);

            let code = gk_code_new();
            let msg = "hello world";
            let s = gk_str_new(heap, msg.as_ptr() as *const c_char, msg.len());
            let c0 = gk_code_add_const(code, s);
            let name = gk_code_add_name(code, c"print".as_ptr());

            gk_code_emit(code, OP_LOAD_NAME, name as u8);
            gk_code_emit(code, OP_LOAD_CONST, c0 as u8);
            gk_code_emit(code, OP_CALL, 1);
            gk_code_emit(code, OP_POP_TOP, 0);

            gk_vm_run(vm, code);
            assert_eq!(gk_vm_error(vm), 0);

            let mut len = 0usize;
            let ptr = gk_vm_output(vm, &mut len);
            let out = std::slice::from_raw_parts(ptr as *const u8, len);
            assert_eq!(out, b"hello world\n");

            gk_code_free(code);
            gk_vm_destroy(vm);
            gk_heap_destroy(heap);
        }
    }

    #[test]
    fn evaluates_arithmetic() {
        unsafe {
            let heap = gk_heap_new();
            let vm = gk_vm_new(heap);

            let code = gk_code_new();
            let a = gk_code_add_const(code, gk_from_int(2));
            let b = gk_code_add_const(code, gk_from_int(3));
            gk_code_emit(code, OP_LOAD_CONST, a as u8);
            gk_code_emit(code, OP_LOAD_CONST, b as u8);
            gk_code_emit(code, OP_BINARY_OP, BIN_MUL);
            gk_code_emit(code, OP_RETURN, 0);

            let r = gk_vm_run(vm, code);
            assert_eq!(gk_vm_error(vm), 0);
            assert_eq!(gk_is_int(r), 1);
            assert_eq!(gk_to_int(r), 6);

            gk_code_free(code);
            gk_vm_destroy(vm);
            gk_heap_destroy(heap);
        }
    }
}
