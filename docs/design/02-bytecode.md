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
- cellvars, freevars: closure slots. Empty until v0.0.2.

A code object is a value, see 01-object-model.md, and is the unit the VM runs.

## Instruction encoding

Wordcode: each instruction is 1 byte of opcode plus 1 byte of argument. Fixed
2-byte units keep decoding branch-free and cache-friendly. An argument wider
than 8 bits needs a preceding EXTENDED_ARG to shift in the high bits. That is
not implemented, so arguments and jump targets are capped at 255, and the
runtime rejects code that exceeds the cap rather than truncating it.

Jump arguments are instruction indices, not byte offsets.

## Instruction set

Implemented:

- LOAD_CONST, LOAD_NAME, STORE_NAME, LOAD_LOCAL, STORE_LOCAL
- POP_TOP
- BINARY_OP (arg selects +, -, *, /)
- COMPARE_OP (arg selects <, <=, ==, !=, >, >=)
- UNARY_NEG, UNARY_NOT
- JUMP, POP_JUMP_IF_FALSE, POP_JUMP_IF_TRUE
- JUMP_IF_FALSE_OR_POP, JUMP_IF_TRUE_OR_POP
- CALL, RETURN

The two OR_POP forms give `and` and `or` their value-preserving semantics
without a DUP_TOP.

Planned: LOAD_GLOBAL, STORE_GLOBAL, DUP_TOP, the remaining BINARY_OP selectors
(//, %, **), BUILD_LIST, BUILD_TUPLE, BUILD_DICT, MAKE_FUNCTION. Attribute
access, subscripting, and iteration opcodes arrive with the features that need
them (v0.0.2).

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
