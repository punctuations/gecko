use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

pub type SetaeValue = u64;

pub enum SetaeHeap {}
pub enum SetaeVm {}
pub enum SetaeCode {}
pub enum SetaeMsg {}

pub type SandboxHook =
    extern "C" fn(*mut SetaeVm, *const c_char, usize, u64, usize, u64) -> SetaeValue;

pub type HostFn = extern "C" fn(*mut SetaeVm, *mut SetaeValue, c_int) -> SetaeValue;

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
    pub fn setae_heap_set_limit(h: *mut SetaeHeap, max_objects: usize);
    pub fn setae_gc_collect(vm: *mut SetaeVm);
    pub fn setae_vm_set_step_limit(vm: *mut SetaeVm, limit: u64);
    pub fn setae_vm_set_time_limit(vm: *mut SetaeVm, millis: u64);
    pub fn setae_vm_set_sandbox_hook(vm: *mut SetaeVm, hook: SandboxHook);
    pub fn setae_vm_heap(vm: *mut SetaeVm) -> *mut SetaeHeap;
    pub fn setae_vm_raise_str(vm: *mut SetaeVm, kind: *const c_char, msg: *const c_char);
    pub fn setae_str_new(h: *mut SetaeHeap, bytes: *const c_char, len: usize) -> SetaeValue;
    pub fn setae_int_from_decimal(
        h: *mut SetaeHeap,
        s: *const c_char,
        n: usize,
        neg: c_int,
    ) -> SetaeValue;

    pub fn setae_msg_read(vm: *mut SetaeVm, v: SetaeValue) -> *mut SetaeMsg;
    pub fn setae_msg_write(vm: *mut SetaeVm, m: *const SetaeMsg) -> SetaeValue;
    pub fn setae_msg_free(m: *mut SetaeMsg);
    pub fn setae_subject_new(h: *mut SetaeHeap, mailbox: *mut std::ffi::c_void) -> SetaeValue;
    pub fn setae_stop_new(h: *mut SetaeHeap) -> SetaeValue;
    pub fn setae_subject_mailbox(v: SetaeValue) -> *mut std::ffi::c_void;
    pub fn setae_set_subject_drop(f: extern "C" fn(*mut std::ffi::c_void));
    pub fn setae_code_serialize(c: *const SetaeCode, len_out: *mut usize) -> *mut u8;
    pub fn setae_func_code(func: SetaeValue) -> *const SetaeCode;
    pub fn setae_bytes_free(p: *mut u8);
    pub fn setae_set_subject_clone(
        f: extern "C" fn(*mut std::ffi::c_void) -> *mut std::ffi::c_void,
    );
    pub fn setae_set_subject_send(f: extern "C" fn(*mut std::ffi::c_void, *mut SetaeMsg));
    pub fn setae_set_subject_call(
        f: extern "C" fn(*mut SetaeVm, SetaeValue, SetaeValue, SetaeValue) -> SetaeValue,
    );
    pub fn setae_subject_send_value(
        vm: *mut SetaeVm,
        subject: SetaeValue,
        arg: SetaeValue,
    ) -> c_int;
    pub fn setae_vm_push_tmp(vm: *mut SetaeVm, v: SetaeValue);
    pub fn setae_vm_pop_tmp(vm: *mut SetaeVm);
    pub fn setae_call(
        vm: *mut SetaeVm,
        callee: SetaeValue,
        args: *mut SetaeValue,
        nargs: c_int,
    ) -> SetaeValue;
    pub fn setae_vm_clear_error(vm: *mut SetaeVm);
    pub fn setae_gecko_actor_register(vm: *mut SetaeVm, name: *const c_char, value: SetaeValue);
    pub fn setae_gecko_actor_module(vm: *mut SetaeVm) -> SetaeValue;
    pub fn setae_obj_type(v: SetaeValue) -> c_int;
    pub fn setae_tuple_len(v: SetaeValue) -> u32;
    pub fn setae_tuple_get(v: SetaeValue, i: u32) -> SetaeValue;
    pub fn setae_value_eq(a: SetaeValue, b: SetaeValue) -> c_int;
    pub fn setae_list_new(h: *mut SetaeHeap, cap: u32) -> SetaeValue;
    pub fn setae_list_append(lv: SetaeValue, v: SetaeValue);
    pub fn setae_list_len(lv: SetaeValue) -> u32;
    pub fn setae_list_get(lv: SetaeValue, i: u32) -> SetaeValue;
    pub fn setae_dict_new(h: *mut SetaeHeap) -> SetaeValue;
    pub fn setae_dict_put(dv: SetaeValue, k: SetaeValue, v: SetaeValue);
    pub fn setae_tuple_new(h: *mut SetaeHeap, items: *const SetaeValue, len: u32) -> SetaeValue;

    pub fn setae_code_new() -> *mut SetaeCode;
    pub fn setae_code_free(c: *mut SetaeCode);
    pub fn setae_code_new_child(parent: *mut SetaeCode) -> *mut SetaeCode;
    pub fn setae_code_new_module(parent: *mut SetaeCode) -> *mut SetaeCode;
    pub fn setae_code_set_module_parent(c: *mut SetaeCode, parent: i32);
    pub fn setae_code_add_const(c: *mut SetaeCode, v: SetaeValue) -> u32;
    pub fn setae_code_add_name(c: *mut SetaeCode, name: *const c_char) -> u32;
    pub fn setae_code_emit(c: *mut SetaeCode, op: u8, arg: u8);
    pub fn setae_code_set_nlocals(c: *mut SetaeCode, n: u32);
    pub fn setae_code_set_nparams(c: *mut SetaeCode, n: u32);
    pub fn setae_code_set_ndefaults(c: *mut SetaeCode, n: u32);
    pub fn setae_code_set_ncells(c: *mut SetaeCode, n: u32);
    pub fn setae_code_set_nfrees(c: *mut SetaeCode, n: u32);
    pub fn setae_code_add_param_name(c: *mut SetaeCode, name: *const c_char);
    pub fn setae_code_set_variadic(c: *mut SetaeCode, varargs: u8, kwargs: u8);
    pub fn setae_code_set_generator(c: *mut SetaeCode, generator: u8);
    pub fn setae_code_set_coroutine(c: *mut SetaeCode, coroutine: u8);
    pub fn setae_code_add_exc(c: *mut SetaeCode, start: u32, end: u32, target: u32, depth: u32);
    pub fn setae_code_set_name(c: *mut SetaeCode, name: *const c_char);

    pub fn setae_vm_new(h: *mut SetaeHeap) -> *mut SetaeVm;
    pub fn setae_vm_destroy(vm: *mut SetaeVm);
    pub fn setae_vm_register_builtins(vm: *mut SetaeVm);
    pub fn setae_vm_register_builtin(vm: *mut SetaeVm, name: *const c_char, v: SetaeValue);
    pub fn setae_builtin_new(h: *mut SetaeHeap, f: HostFn, name: *const c_char) -> SetaeValue;
    pub fn setae_vm_set_global(vm: *mut SetaeVm, name: *const c_char, v: SetaeValue);
    pub fn setae_vm_run(vm: *mut SetaeVm, code: *mut SetaeCode) -> SetaeValue;
    pub fn setae_vm_error(vm: *mut SetaeVm) -> c_int;
    pub fn setae_vm_error_msg(vm: *mut SetaeVm) -> *const c_char;
    pub fn setae_vm_output(vm: *mut SetaeVm, len: *mut usize) -> *const c_char;
}

use std::sync::mpsc::{Receiver, Sender, channel};

struct Envelope {
    msg: *mut SetaeMsg,
    reply: Option<Sender<Envelope>>,
    err: Option<String>,
}
unsafe impl Send for Envelope {}

impl Envelope {
    fn value(msg: *mut SetaeMsg) -> Self {
        Envelope {
            msg,
            reply: None,
            err: None,
        }
    }
}

extern "C" fn subject_drop(mailbox: *mut std::ffi::c_void) {
    if !mailbox.is_null() {
        unsafe { drop(Box::from_raw(mailbox as *mut Sender<Envelope>)) };
    }
}

extern "C" fn subject_clone(mailbox: *mut std::ffi::c_void) -> *mut std::ffi::c_void {
    let sender = unsafe { &*(mailbox as *const Sender<Envelope>) };
    Box::into_raw(Box::new(sender.clone())) as *mut std::ffi::c_void
}

extern "C" fn subject_send(mailbox: *mut std::ffi::c_void, msg: *mut SetaeMsg) {
    let sender = unsafe { &*(mailbox as *const Sender<Envelope>) };
    if sender.send(Envelope::value(msg)).is_err() {
        unsafe { setae_msg_free(msg) };
    }
}

extern "C" fn subject_call(
    vm: *mut SetaeVm,
    subject: SetaeValue,
    build: SetaeValue,
    timeout: SetaeValue,
) -> SetaeValue {
    unsafe {
        let millis = if setae_is_int(timeout) != 0 {
            setae_to_int(timeout).max(0) as u64
        } else {
            0
        };
        let heap = setae_vm_heap(vm);
        let (tx, rx) = channel::<Envelope>();
        let reply = setae_subject_new(
            heap,
            Box::into_raw(Box::new(tx.clone())) as *mut std::ffi::c_void,
        );

        setae_vm_push_tmp(vm, reply);
        let mut build_args = [reply];
        let message = setae_call(vm, build, build_args.as_mut_ptr(), 1);
        if setae_vm_error(vm) != 0 {
            setae_vm_pop_tmp(vm);
            return setae_none();
        }
        setae_vm_push_tmp(vm, message);
        let msg = setae_msg_read(vm, message);
        setae_vm_pop_tmp(vm);
        setae_vm_pop_tmp(vm);
        if msg.is_null() {
            return setae_none();
        }

        let actor_sender = &*(setae_subject_mailbox(subject) as *const Sender<Envelope>);
        let inbound = Envelope {
            msg,
            reply: Some(tx),
            err: None,
        };
        if actor_sender.send(inbound).is_err() {
            setae_msg_free(msg);
            let k = CString::new("RuntimeError").unwrap();
            let m = CString::new("the actor is no longer running").unwrap();
            setae_vm_raise_str(vm, k.as_ptr(), m.as_ptr());
            return setae_none();
        }

        match rx.recv_timeout(std::time::Duration::from_millis(millis)) {
            Ok(env) => {
                if let Some(text) = env.err {
                    let k = CString::new("RuntimeError").unwrap();
                    let m = CString::new(text)
                        .unwrap_or_else(|_| CString::new("actor handler failed").unwrap());
                    setae_vm_raise_str(vm, k.as_ptr(), m.as_ptr());
                    setae_none()
                } else {
                    let r = setae_msg_write(vm, env.msg);
                    setae_msg_free(env.msg);
                    r
                }
            }
            Err(_) => {
                let k = CString::new("TimeoutError").unwrap();
                let m = CString::new("actor did not reply within the timeout").unwrap();
                setae_vm_raise_str(vm, k.as_ptr(), m.as_ptr());
                setae_none()
            }
        }
    }
}

const T_LIST: c_int = 3;
const T_TUPLE: c_int = 5;
const T_FUNCTION: c_int = 6;
const T_STOP: c_int = 19;

extern "C" fn actor_stop(vm: *mut SetaeVm, _args: *mut SetaeValue, _argc: c_int) -> SetaeValue {
    unsafe { setae_stop_new(setae_vm_heap(vm)) }
}

unsafe fn set_root(vm: *mut SetaeVm, name: &str, v: SetaeValue) {
    let c = CString::new(name).expect("root name has no interior NUL");
    unsafe { setae_vm_set_global(vm, c.as_ptr(), v) };
}

extern "C" fn actor_spawn(vm: *mut SetaeVm, args: *mut SetaeValue, argc: c_int) -> SetaeValue {
    unsafe {
        if argc != 2 && argc != 3 {
            let k = CString::new("TypeError").unwrap();
            let m = CString::new("spawn() takes 2 or 3 arguments").unwrap();
            setae_vm_raise_str(vm, k.as_ptr(), m.as_ptr());
            return setae_none();
        }
        let argv = std::slice::from_raw_parts(args, argc as usize);
        let state = argv[0];
        let handle = argv[1];
        if setae_obj_type(handle) != T_FUNCTION {
            let k = CString::new("TypeError").unwrap();
            let m = CString::new("spawn() handler must be a function").unwrap();
            setae_vm_raise_str(vm, k.as_ptr(), m.as_ptr());
            return setae_none();
        }

        let code = setae_func_code(handle);
        let mut len = 0usize;
        let ptr = setae_code_serialize(code, &mut len);
        let bytes = std::slice::from_raw_parts(ptr, len).to_vec();
        setae_bytes_free(ptr);

        let heap = setae_vm_heap(vm);
        let extras = setae_list_new(heap, 0);
        if argc == 3 {
            let e = argv[2];
            match setae_obj_type(e) {
                T_LIST => {
                    for i in 0..setae_list_len(e) {
                        setae_list_append(extras, setae_list_get(e, i));
                    }
                }
                T_TUPLE => {
                    for i in 0..setae_tuple_len(e) {
                        setae_list_append(extras, setae_tuple_get(e, i));
                    }
                }
                _ => {
                    let k = CString::new("TypeError").unwrap();
                    let m = CString::new("spawn() args must be a list or tuple").unwrap();
                    setae_vm_raise_str(vm, k.as_ptr(), m.as_ptr());
                    return setae_none();
                }
            }
        }
        let pack = setae_list_new(heap, 2);
        setae_list_append(pack, state);
        setae_list_append(pack, extras);

        let init = setae_msg_read(vm, pack);
        if init.is_null() {
            return setae_none();
        }

        let (tx, rx) = channel::<Envelope>();
        let boxed = Box::into_raw(Box::new(tx)) as *mut std::ffi::c_void;
        let subject = setae_subject_new(heap, boxed);

        let init = Envelope::value(init);
        std::thread::spawn(move || actor_main(bytes, init, rx));
        subject
    }
}

fn actor_main(bytes: Vec<u8>, init: Envelope, rx: Receiver<Envelope>) {
    let handler = match bytecode::from_bytes(&bytes) {
        Ok(c) => c,
        Err(_) => return,
    };
    let wrapper = bytecode::Code {
        name: String::new(),
        consts: Vec::new(),
        names: Vec::new(),
        ops: vec![
            bytecode::Instr {
                op: bytecode::Op::MakeFunction,
                arg: 0,
            },
            bytecode::Instr {
                op: bytecode::Op::Return,
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
        generator: false,
        coroutine: false,
        codes: vec![handler],
        modules: Vec::new(),
        parent_module: -1,
    };

    let mut child = Vm::new();
    child.enable_actors();
    let run = child.run(&wrapper);
    if run.error {
        return;
    }
    let handle = run.result;

    unsafe {
        set_root(child.vm, "actor", setae_gecko_actor_module(child.vm));
        set_root(child.vm, "$handle", handle);
        let pack = setae_msg_write(child.vm, init.msg);
        setae_msg_free(init.msg);
        let mut state = setae_list_get(pack, 0);
        let extras = setae_list_get(pack, 1);
        set_root(child.vm, "$state", state);
        set_root(child.vm, "$extras", extras);
        let extra_items: Vec<SetaeValue> = (0..setae_list_len(extras))
            .map(|i| setae_list_get(extras, i))
            .collect();

        while let Ok(env) = rx.recv() {
            let message = setae_msg_write(child.vm, env.msg);
            setae_msg_free(env.msg);
            set_root(child.vm, "$msg", message);

            let mut call_args = Vec::with_capacity(2 + extra_items.len());
            call_args.push(state);
            call_args.push(message);
            call_args.extend_from_slice(&extra_items);
            let next = setae_call(
                child.vm,
                handle,
                call_args.as_mut_ptr(),
                call_args.len() as c_int,
            );
            if setae_vm_error(child.vm) != 0 {
                let text = CStr::from_ptr(setae_vm_error_msg(child.vm))
                    .to_string_lossy()
                    .into_owned();
                setae_vm_clear_error(child.vm);
                if let Some(reply) = env.reply {
                    let _ = reply.send(Envelope {
                        msg: std::ptr::null_mut(),
                        reply: None,
                        err: Some(text),
                    });
                }
                break;
            }
            if setae_obj_type(next) == T_STOP {
                break;
            }
            state = next;
            set_root(child.vm, "$state", state);
        }
    }
}

pub struct Mailbox {
    rx: Receiver<Envelope>,
}

impl Mailbox {
    pub fn recv(&self, vm: &Vm) -> Option<SetaeValue> {
        let env = self.rx.recv().ok()?;
        if env.msg.is_null() {
            return None;
        }
        let v = unsafe { setae_msg_write(vm.vm, env.msg) };
        unsafe { setae_msg_free(env.msg) };
        Some(v)
    }

    pub fn try_recv(&self, vm: &Vm) -> Option<SetaeValue> {
        let env = self.rx.try_recv().ok()?;
        if env.msg.is_null() {
            return None;
        }
        let v = unsafe { setae_msg_write(vm.vm, env.msg) };
        unsafe { setae_msg_free(env.msg) };
        Some(v)
    }
}

impl Vm {
    pub fn mailbox(&self) -> (SetaeValue, Mailbox) {
        let (tx, rx) = channel::<Envelope>();
        let boxed = Box::into_raw(Box::new(tx)) as *mut std::ffi::c_void;
        let subject = unsafe { setae_subject_new(self.heap, boxed) };
        (subject, Mailbox { rx })
    }

    pub fn send(&self, subject: SetaeValue, value: SetaeValue) -> bool {
        let msg = unsafe { setae_msg_read(self.vm, value) };
        if msg.is_null() {
            return false;
        }
        let sender = unsafe { &*(setae_subject_mailbox(subject) as *const Sender<Envelope>) };
        if sender.send(Envelope::value(msg)).is_err() {
            unsafe { setae_msg_free(msg) };
            return false;
        }
        true
    }
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
    code.ops.iter().all(|i| i.arg <= u8::MAX as u32)
        && code.codes.iter().all(args_fit)
        && code.modules.iter().all(args_fit)
}

impl Vm {
    pub fn new() -> Self {
        unsafe {
            let heap = setae_heap_new();
            let vm = setae_vm_new(heap);
            setae_vm_register_builtins(vm);
            setae_set_subject_drop(subject_drop);
            setae_set_subject_clone(subject_clone);
            setae_set_subject_send(subject_send);
            setae_set_subject_call(subject_call);
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

    pub fn set_step_limit(&mut self, limit: u64) {
        unsafe { setae_vm_set_step_limit(self.vm, limit) }
    }

    pub fn set_time_limit(&mut self, millis: u64) {
        unsafe { setae_vm_set_time_limit(self.vm, millis) }
    }

    pub fn set_memory_limit(&mut self, max_objects: usize) {
        unsafe { setae_heap_set_limit(self.heap, max_objects) }
    }

    pub fn set_sandbox_hook(&mut self, hook: SandboxHook) {
        unsafe { setae_vm_set_sandbox_hook(self.vm, hook) }
    }

    pub fn enable_actors(&mut self) {
        let spawn = CString::new("spawn").expect("name has no interior NUL");
        let stop = CString::new("stop").expect("name has no interior NUL");
        unsafe {
            let b = setae_builtin_new(self.heap, actor_spawn, spawn.as_ptr());
            setae_gecko_actor_register(self.vm, spawn.as_ptr(), b);
            let s = setae_builtin_new(self.heap, actor_stop, stop.as_ptr());
            setae_gecko_actor_register(self.vm, stop.as_ptr(), s);
        }
        std::mem::forget(spawn);
        std::mem::forget(stop);
    }

    pub fn register_fn(&mut self, name: &str, f: HostFn) {
        let cname = CString::new(name).expect("name has no interior NUL");
        unsafe {
            let b = setae_builtin_new(self.heap, f, cname.as_ptr());
            setae_vm_register_builtin(self.vm, cname.as_ptr(), b);
        }
        std::mem::forget(cname);
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
                    bytecode::Const::BigInt(s) => {
                        setae_int_from_decimal(self.heap, s.as_ptr() as *const c_char, s.len(), 0)
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
            setae_code_set_ndefaults(gc, code.ndefaults);
            setae_code_set_ncells(gc, code.ncells);
            setae_code_set_nfrees(gc, code.nfrees);
            for name in &code.param_names {
                let cs = CString::new(name.as_str()).expect("param name has no interior NUL");
                setae_code_add_param_name(gc, cs.as_ptr());
            }
            setae_code_set_variadic(gc, code.varargs as u8, code.kwargs as u8);
            setae_code_set_generator(gc, code.generator as u8);
            setae_code_set_coroutine(gc, code.coroutine as u8);
            setae_code_set_module_parent(gc, code.parent_module);
            let cs = CString::new(code.name.as_str()).expect("name has no interior NUL");
            setae_code_set_name(gc, cs.as_ptr());
            for child in &code.codes {
                let cgc = setae_code_new_child(gc);
                self.lower(cgc, child);
            }
            for module in &code.modules {
                let mgc = setae_code_new_module(gc);
                self.lower(mgc, module);
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
            ndefaults: 0,
            ncells,
            nfrees,
            param_names: Vec::new(),
            varargs: false,
            kwargs: false,
            generator: false,
            coroutine: false,
            codes: Vec::new(),
            modules: Vec::new(),
            parent_module: -1,
        }
    }

    fn ins(op: Op, arg: u32) -> Instr {
        Instr { op, arg }
    }

    #[test]
    fn a_lowered_code_reserializes_to_the_original_bytes() {
        let mut add = blank(3, 2, 0, 0);
        add.name = "add".into();
        add.param_names = vec!["a".into(), "b".into(), "rest".into()];
        add.varargs = true;
        add.consts = vec![Const::Str("hi".into()), Const::Float(1.5)];
        add.names = vec!["print".into()];
        add.ops = vec![
            ins(Op::LoadLocal, 0),
            ins(Op::LoadLocal, 1),
            ins(Op::BinaryOp, bytecode::BIN_ADD),
            ins(Op::Return, 0),
        ];
        let mut m = blank(0, 0, 0, 0);
        m.name = String::new();
        m.consts = vec![Const::Int(2), Const::None, Const::Bool(true)];
        m.codes = vec![add];
        m.ops = vec![ins(Op::MakeFunction, 0), ins(Op::Return, 0)];

        let want = bytecode::to_bytes(&m);
        let mut vm = Vm::new();
        unsafe {
            let gc = setae_code_new();
            vm.lower(gc, &m);
            let mut len = 0usize;
            let ptr = setae_code_serialize(gc, &mut len);
            let got = std::slice::from_raw_parts(ptr, len).to_vec();
            setae_bytes_free(ptr);
            setae_code_free(gc);
            assert_eq!(got, want, "C serialization must match bytecode::to_bytes");
        }
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
    fn import_runs_a_module_and_reads_its_attribute() {
        let mut module = blank(0, 0, 0, 0);
        module.name = "helpers".into();
        module.consts = vec![Const::Int(42)];
        module.names = vec!["value".into()];
        module.ops = vec![ins(Op::LoadConst, 0), ins(Op::StoreName, 0)];

        let mut root = blank(0, 0, 0, 0);
        root.names = vec!["value".into()];
        root.modules = vec![module];
        root.ops = vec![ins(Op::Import, 0), ins(Op::LoadAttr, 0), ins(Op::Return, 0)];
        let mut vm = Vm::new();
        let run = vm.run(&root);
        assert_eq!(int_result(&run), 42);
    }

    #[test]
    fn a_module_is_cached_across_imports() {
        let mut module = blank(1, 0, 0, 0);
        module.name = "counter".into();
        module.consts = vec![Const::Int(1)];
        module.names = vec!["n".into()];
        module.ops = vec![ins(Op::LoadConst, 0), ins(Op::StoreName, 0)];

        let mut root = blank(0, 0, 0, 0);
        root.names = vec!["n".into()];
        root.modules = vec![module];
        root.ops = vec![
            ins(Op::Import, 0),
            ins(Op::LoadAttr, 0),
            ins(Op::Import, 0),
            ins(Op::LoadAttr, 0),
            ins(Op::BinaryOp, bytecode::BIN_ADD),
            ins(Op::Return, 0),
        ];
        let mut vm = Vm::new();
        let run = vm.run(&root);
        assert_eq!(int_result(&run), 2);
    }

    #[test]
    fn a_step_limit_interrupts_an_infinite_loop() {
        let mut m = blank(0, 0, 0, 0);
        m.consts = vec![Const::Bool(true)];
        m.ops = vec![
            ins(Op::LoadConst, 0),
            ins(Op::PopJumpIfFalse, 3),
            ins(Op::Jump, 0),
        ];
        let mut vm = Vm::new();
        vm.set_step_limit(1000);
        let run = vm.run(&m);
        assert!(run.error);
        assert!(run.message.contains("step limit"), "{}", run.message);
    }

    #[test]
    fn a_time_limit_interrupts_an_infinite_loop() {
        let mut m = blank(0, 0, 0, 0);
        m.consts = vec![Const::Bool(true)];
        m.ops = vec![
            ins(Op::LoadConst, 0),
            ins(Op::PopJumpIfFalse, 3),
            ins(Op::Jump, 0),
        ];
        let mut vm = Vm::new();
        vm.set_time_limit(20);
        let start = std::time::Instant::now();
        let run = vm.run(&m);
        assert!(run.error);
        assert!(run.message.contains("time limit"), "{}", run.message);
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn transfer_copies_nested_values_across_heaps() {
        unsafe {
            let a = Vm::new();
            let b = Vm::new();
            let inner = setae_list_new(a.heap, 0);
            setae_list_append(inner, setae_from_int(2));
            setae_list_append(inner, setae_from_int(3));
            let d = setae_dict_new(a.heap);
            setae_dict_put(
                d,
                setae_str_new(a.heap, c"k".as_ptr(), 1),
                setae_from_int(4),
            );
            let top = setae_list_new(a.heap, 0);
            setae_list_append(top, setae_from_int(1));
            setae_list_append(top, setae_str_new(a.heap, c"hi".as_ptr(), 2));
            setae_list_append(top, inner);
            setae_list_append(top, d);
            let msg = setae_msg_read(a.vm, top);
            assert!(!msg.is_null(), "read should succeed");
            let copy = setae_msg_write(b.vm, msg);
            setae_msg_free(msg);
            assert_eq!(
                setae_value_eq(top, copy),
                1,
                "copy equals the original by value"
            );
        }
    }

    #[test]
    fn transfer_preserves_a_cycle() {
        unsafe {
            let a = Vm::new();
            let b = Vm::new();
            let l = setae_list_new(a.heap, 0);
            setae_list_append(l, setae_from_int(7));
            setae_list_append(l, l);
            let msg = setae_msg_read(a.vm, l);
            assert!(!msg.is_null());
            let copy = setae_msg_write(b.vm, msg);
            setae_msg_free(msg);
            assert_eq!(setae_list_len(copy), 2);
            assert_eq!(setae_to_int(setae_list_get(copy, 0)), 7);
            assert_eq!(setae_list_get(copy, 1), copy, "the copy references itself");
        }
    }

    #[test]
    fn a_mailbox_carries_a_value_between_heaps() {
        unsafe {
            let a = Vm::new();
            let b = Vm::new();
            let (subject, mailbox) = b.mailbox();
            let v = setae_list_new(a.heap, 0);
            setae_list_append(v, setae_from_int(1));
            setae_list_append(v, setae_from_int(2));
            setae_list_append(v, setae_from_int(3));
            assert!(a.send(subject, v), "send should succeed");
            let got = mailbox.recv(&b).expect("a message arrives");
            assert_eq!(
                setae_value_eq(v, got),
                1,
                "the received value equals what was sent"
            );
        }
    }

    #[test]
    fn a_subject_travels_inside_a_message() {
        let b = Vm::new();
        let (subject, mailbox) = b.mailbox();
        assert!(b.send(subject, subject), "a subject is sendable by handle");
        let got = mailbox.recv(&b).expect("the subject arrives");
        assert_eq!(
            unsafe { setae_obj_type(got) },
            18,
            "the received value is a subject"
        );
    }

    #[test]
    fn an_actor_processes_messages_and_reports_back() {
        let src = "def handle(state, message, report):\n    total = state + message\n    report.send(total)\n    return total\n";
        let module = parser::parse(src).expect("parse");
        let code = compiler::compile_with_base(&module, None).expect("compile");
        let bytes = bytecode::to_bytes(&code.codes[0]);

        let driver = Vm::new();
        let (report_subject, report_mailbox) = driver.mailbox();
        unsafe {
            let (actor_tx, actor_rx) = channel::<Envelope>();
            let boxed = Box::into_raw(Box::new(actor_tx)) as *mut std::ffi::c_void;
            let actor_subject = setae_subject_new(driver.heap, boxed);

            let extras = setae_list_new(driver.heap, 0);
            setae_list_append(extras, report_subject);
            let pack = setae_list_new(driver.heap, 2);
            setae_list_append(pack, setae_from_int(0));
            setae_list_append(pack, extras);
            let init = setae_msg_read(driver.vm, pack);
            assert!(!init.is_null(), "init pack is sendable");
            let init = Envelope::value(init);

            std::thread::spawn(move || actor_main(bytes, init, actor_rx));

            assert!(driver.send(actor_subject, setae_from_int(10)));
            assert!(driver.send(actor_subject, setae_from_int(5)));

            let r1 = report_mailbox.recv(&driver).expect("first report");
            let r2 = report_mailbox.recv(&driver).expect("second report");
            assert_eq!(setae_to_int(r1), 10, "0 + 10");
            assert_eq!(setae_to_int(r2), 15, "10 + 5");
        }
    }

    #[test]
    fn a_step_interrupt_is_not_catchable() {
        let mut m = blank(0, 0, 0, 0);
        m.consts = vec![Const::Bool(true), Const::Int(0)];
        m.ops = vec![
            ins(Op::LoadConst, 0),
            ins(Op::PopJumpIfFalse, 3),
            ins(Op::Jump, 0),
            ins(Op::PopTop, 0),
            ins(Op::LoadConst, 1),
            ins(Op::Return, 0),
        ];
        m.excs = vec![bytecode::ExcEntry {
            start: 0,
            end: 3,
            target: 3,
            depth: 0,
        }];
        let mut vm = Vm::new();
        vm.set_step_limit(1000);
        let run = vm.run(&m);
        assert!(run.error, "an interrupt must not be caught by try/except");
        assert!(run.message.contains("step limit"));
    }

    #[test]
    fn a_memory_limit_stops_runaway_allocation() {
        let mut m = blank(1, 0, 0, 0);
        m.consts = vec![Const::Str("x".into()), Const::Str("y".into())];
        m.ops = vec![
            ins(Op::LoadConst, 0),
            ins(Op::LoadConst, 1),
            ins(Op::BinaryOp, bytecode::BIN_ADD),
            ins(Op::StoreLocal, 0),
            ins(Op::Jump, 0),
        ];
        let mut vm = Vm::new();
        vm.set_memory_limit(vm.heap_live() + 200);
        vm.set_step_limit(1_000_000);
        let run = vm.run(&m);
        assert!(run.error);
        assert!(run.message.contains("MemoryError"), "{}", run.message);
    }

    extern "C" fn host_double(
        _vm: *mut SetaeVm,
        args: *mut SetaeValue,
        nargs: c_int,
    ) -> SetaeValue {
        unsafe {
            if nargs != 1 || setae_is_int(*args) == 0 {
                return setae_none();
            }
            setae_from_int(setae_to_int(*args) * 2)
        }
    }

    #[test]
    fn a_host_function_is_callable_from_bytecode() {
        let mut m = blank(0, 0, 0, 0);
        m.consts = vec![Const::Int(21)];
        m.names = vec!["double".into()];
        m.ops = vec![
            ins(Op::LoadName, 0),
            ins(Op::LoadConst, 0),
            ins(Op::Call, 1),
            ins(Op::Return, 0),
        ];
        let mut vm = Vm::new();
        vm.register_fn("double", host_double);
        let run = vm.run(&m);
        assert_eq!(int_result(&run), 42);
    }

    #[test]
    fn separate_vms_have_isolated_state() {
        let mut a = blank(0, 0, 0, 0);
        a.consts = vec![Const::Int(10)];
        a.names = vec!["x".into()];
        a.ops = vec![
            ins(Op::LoadConst, 0),
            ins(Op::StoreName, 0),
            ins(Op::LoadName, 0),
            ins(Op::Return, 0),
        ];
        let mut b = blank(0, 0, 0, 0);
        b.names = vec!["x".into()];
        b.ops = vec![ins(Op::LoadName, 0), ins(Op::Return, 0)];

        let mut vm_a = Vm::new();
        assert_eq!(int_result(&vm_a.run(&a)), 10);
        let mut vm_b = Vm::new();
        let run_b = vm_b.run(&b);
        assert!(run_b.error, "x set in vm_a must not leak into vm_b");
        assert!(run_b.message.contains("NameError"));
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
    fn make_class_instance_and_attributes_round_trip() {
        let mut m = blank(1, 0, 0, 0);
        m.consts = vec![Const::None, Const::Str("C".into()), Const::Int(42)];
        m.names = vec!["v".into()];
        m.ops = vec![
            ins(Op::BuildDict, 0),
            ins(Op::LoadConst, 0),
            ins(Op::LoadConst, 1),
            ins(Op::MakeClass, 0),
            ins(Op::Call, 0),
            ins(Op::StoreLocal, 0),
            ins(Op::LoadConst, 2),
            ins(Op::LoadLocal, 0),
            ins(Op::StoreAttr, 0),
            ins(Op::LoadLocal, 0),
            ins(Op::LoadAttr, 0),
            ins(Op::Return, 0),
        ];
        let mut vm = Vm::new();
        let run = vm.run(&m);
        assert_eq!(int_result(&run), 42);
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
