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
- COMPARE_OP (arg selects <, <=, ==, !=, >, >=, in, not in)
- UNARY_NEG, UNARY_NOT
- JUMP, POP_JUMP_IF_FALSE, POP_JUMP_IF_TRUE
- JUMP_IF_FALSE_OR_POP, JUMP_IF_TRUE_OR_POP
- CALL, RETURN
- MAKE_FUNCTION (arg is a child code index; pops the child's captured cells)
- LOAD_CLOSURE (pushes a cell), LOAD_DEREF, STORE_DEREF (cell contents)
- BUILD_LIST, BUILD_TUPLE, BUILD_DICT (arg is the element or pair count)
- UNPACK_SEQUENCE (arg is the target count; pushes elements in reverse)
- SUBSCR, STORE_SUBSCR
- GET_ITER, FOR_ITER (arg is the jump target once exhausted)
- CALL_METHOD (arg packs a name index in the high bits and the argument count
  in the low byte)
- EXTENDED_ARG

The two OR_POP forms give `and` and `or` their value-preserving semantics
without a DUP_TOP.

A function call runs the callee's code object in a new frame. The VM implements
frames as C recursion with a depth cap. Falling off the end of a function
returns None via a compiler-emitted LOAD_CONST and RETURN.

Planned: LOAD_GLOBAL, STORE_GLOBAL, DUP_TOP, the ** BINARY_OP selector.
Attribute access beyond method calls arrives with classes (v0.0.2).

## Dispatch

Setae uses computed goto, a dispatch table of label addresses, where the C
compiler supports it, and falls back to a switch. The opcode numbering keeps
that table dense.

## Serialization

v0.0.1 keeps code objects in memory only. An on-disk cache format, compiled
`.gecko` modules, waits for the packaging phase in v0.0.3.

## Open

- Superinstructions and inline caches wait for v0.0.5, but leave opcode space
  for them.
- The exception table format, for zero-cost try/except in v0.0.2.
