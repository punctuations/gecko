use runtime::{SetaeValue, SetaeVm};
use std::ffi::{CString, c_char};

pub extern "C" fn hook(
    vm: *mut SetaeVm,
    src: *const c_char,
    len: usize,
    steps: u64,
    mem: usize,
    millis: u64,
) -> SetaeValue {
    let source = {
        let bytes = unsafe { std::slice::from_raw_parts(src as *const u8, len) };
        match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => return raise(vm, "source is not valid UTF-8"),
        }
    };
    let module = match parser::parse(&source) {
        Ok(m) => m,
        Err(e) => return raise(vm, &format!("SyntaxError: {}", e.message)),
    };
    let code = match compiler::compile(&module) {
        Ok(c) => c,
        Err(e) => return raise(vm, &format!("CompileError: {}", e.message)),
    };
    let mut sub = runtime::Vm::new();
    if steps > 0 {
        sub.set_step_limit(steps);
    }
    if mem > 0 {
        sub.set_memory_limit(mem);
    }
    if millis > 0 {
        sub.set_time_limit(millis);
    }
    let run = sub.run(&code);
    if run.error {
        let msg = if run.message.is_empty() {
            "sandboxed program failed".to_string()
        } else {
            run.message.clone()
        };
        return raise(vm, &msg);
    }
    unsafe {
        let heap = runtime::setae_vm_heap(vm);
        runtime::setae_str_new(heap, run.output.as_ptr() as *const c_char, run.output.len())
    }
}

fn raise(vm: *mut SetaeVm, msg: &str) -> SetaeValue {
    let kind = CString::new("SandboxError").unwrap();
    let msg = CString::new(msg.replace('\0', "")).unwrap();
    unsafe {
        runtime::setae_vm_raise_str(vm, kind.as_ptr(), msg.as_ptr());
        runtime::setae_none()
    }
}
