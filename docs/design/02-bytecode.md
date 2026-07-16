# Bytecode

## Machine

Setae is a stack-based VM. Operands and results live on an operand stack, one
per frame. This is close to Python semantics and keeps the compiler simple. A
register-based rewrite is a possible optimization for v0.0.5 and is out of scope
now.

## Code object

The Rust compiler emits one code object per function and one per module body:

- consts: the constant pool, holding the values LOAD_CONST refers to.
- names: the table of global and attribute names.
- code: the instruction bytes.
- nlocals: the number of local slots.
- stacksize: the deepest the operand stack gets, computed by the compiler.
- ncells, nfrees: closure slots. A frame lays out locals, then cells it owns,
  then cells it captured, then the operand stack.

A code object is a value, see 01-object-model.md, and is the unit the VM runs.

## Instruction encoding

Wordcode: each instruction is 1 byte of opcode plus 1 byte of argument. Fixed
2-byte units keep decoding branch-free and cache-friendly. An argument wider
than 8 bits gets one or more preceding EXTENDED_ARG instructions that shift in
the high bits. The compiler materializes them in an assembly pass: widening an
instruction shifts every later jump target, so the pass reruns until the widths
stop changing, then emits.

A jump argument is an index into the 2-byte instruction units. An EXTENDED_ARG
prefix counts as an instruction of its own.

## Instruction set

Implemented:

- LOAD_CONST, LOAD_NAME, STORE_NAME, LOAD_LOCAL, STORE_LOCAL
- POP_TOP
- BINARY_OP (arg selects +, -, *, /, %, //)
- COMPARE_OP (arg selects <, <=, ==, !=, >, >=, in, not in, is, is not; is
  compares by identity, which for gecko values is bit equality)
- UNARY_NEG, UNARY_NOT
- JUMP, POP_JUMP_IF_FALSE, POP_JUMP_IF_TRUE
- JUMP_IF_FALSE_OR_POP, JUMP_IF_TRUE_OR_POP
- CALL, RETURN
- MAKE_FUNCTION (arg is a child code index; pops the child's default values then
  its captured cells)
- LOAD_CLOSURE (pushes a cell), LOAD_DEREF, STORE_DEREF (cell contents)
- BUILD_LIST, BUILD_TUPLE, BUILD_DICT (arg is the element or pair count)
- UNPACK_SEQUENCE (arg is the target count; pushes elements in reverse)
- SUBSCR, STORE_SUBSCR
- GET_ITER, FOR_ITER (arg is the jump target once exhausted)
- CALL_METHOD (arg packs a name index in the high bits and the argument count
  in the low byte)
- EXTENDED_ARG
- RAISE, RERAISE, EXC_MATCH
- LOAD_ATTR, STORE_ATTR (arg is a name index)
- MAKE_CLASS (pops the namespace dict, base, and name; a base that is a builtin
  exception type produces a new exception type of that name)
- IMPORT (arg is a module index; pushes the module object)
- IMPORT_MISSING (arg is a name index; raises ImportError for a module the
  compiler could not resolve)

The two OR_POP forms give `and` and `or` their value-preserving semantics
without a DUP_TOP.

A function call runs the callee's code object in a new frame. The VM implements
frames as C recursion with a depth cap. Falling off the end of a function
returns None via a compiler-emitted LOAD_CONST and RETURN.

Default arguments are evaluated once where the def appears and stored on the
function object. A call that passes fewer arguments than parameters fills the
trailing gap from those stored defaults, so the accepted count runs from
params minus defaults up to params.

A class body compiles to its own code object. MAKE_FUNCTION and CALL run it to
produce a namespace dict, which MAKE_CLASS turns into a class object with its
name and optional single base. Instantiating a class allocates an instance and
runs __init__ with the instance prepended as self. Looking up a method binds it
to its instance.

## Modules

Imports resolve at compile time. The compiler searches an ordered path for the
module: the importing file's directory, then each directory in the GECKO_PATH
environment variable, then the site-packages directory under GECKO_HOME (or
~/.gecko when GECKO_HOME is unset). A name resolves to `name.py` or to a package
directory `name/__init__.py`. The compiler reads and compiles the file into a module code
object, registered on the root code object with a global index, and lowers an
import to IMPORT plus the name bindings. A frozen binary therefore carries its
imports with no file access at run time.

A dotted name like `a.b.c` resolves segment by segment. The first segment
searches the path; each later segment resolves inside its parent package's
directory, and every segment but the last must be a package. Each prefix
becomes its own module code object carrying its parent's index. IMPORT links a
submodule into its parent's namespace, so `a.b` becomes an attribute of `a`.

A relative import (`from . import x`, `from ..pkg import y`) resolves against
the importing module's own directory, one level up per leading dot past the
first. It never touches the search path.

A module the compiler cannot find is not a compile error. The compiler lowers it
to IMPORT_MISSING, which raises ImportError at run time naming the module. This
keeps a program that imports a CPython-only module in a branch gecko never takes
from failing to compile. A module that is found but fails to parse or compile is
still a hard compile error.

Each module runs in its own namespace. A frame carries the module it belongs
to, and a function carries the module it was defined in, so LOAD_NAME and
STORE_NAME target that module's namespace dict rather than the main script's
globals. Builtins live in a separate table that every module falls back to. The
main script has no module and uses a flat global table, so a program with no
imports resolves names exactly as before. IMPORT runs a module once and caches
the result, so cyclic imports get the partially built module already in the
cache.

Planned: LOAD_GLOBAL, STORE_GLOBAL, DUP_TOP, the ** BINARY_OP selector.

## Exception table

Each code object carries a table of entries (start, end, target, depth), all in
instruction units. When an instruction in [start, end) raises, the VM cuts the
operand stack back to depth, pushes the exception object, and jumps to target.
Entries are scanned in order and the first match wins. An inner try finishes
compiling before its enclosing try, so inner entries come first and shadow the
outer ones. A frame with no matching entry returns to its caller, which repeats
the search at its CALL instruction. A try block costs nothing when no exception
is raised.

depth is the number of enclosing for-loop iterators inside the frame, which is
everything living on the operand stack at a statement boundary. finally blocks
are compiled twice, once inline for the normal path and once as a handler that
reraises.

## Dispatch

Setae uses computed goto, a dispatch table of label addresses, where the C
compiler supports it, and falls back to a switch. The opcode numbering keeps
that table dense.

## Serialization

Code objects serialize to a small length-prefixed binary format with a
versioned magic. `gecko build` uses it to freeze a program: the serialized
module is appended behind a trailer to a copy of gecko-runner, a stub binary
holding only the VM and the bytecode reader, and the stub checks its own tail
at startup. An on-disk cache format for imported modules waits for the
packaging phase in v0.0.3.

## Open

- Superinstructions and inline caches wait for v0.0.5, but leave opcode space
  for them.
