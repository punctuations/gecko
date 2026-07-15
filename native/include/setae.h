#ifndef SETAE_H
#define SETAE_H

#include <stddef.h>
#include <stdint.h>

/* NaN-boxed value word. The encoding is in docs/design/01-object-model.md. */
typedef uint64_t SetaeValue;

#define SETAE_NUMBER_TAG    0xffff000000000000ULL
#define SETAE_DOUBLE_OFFSET 0x0001000000000000ULL

#define SETAE_VAL_NONE  0x02ULL
#define SETAE_VAL_FALSE 0x04ULL
#define SETAE_VAL_TRUE  0x06ULL

int setae_is_float(SetaeValue v);
int setae_is_int(SetaeValue v); /* fixnum, not a heap bignum */
int setae_is_ptr(SetaeValue v);
int setae_is_none(SetaeValue v);
int setae_is_bool(SetaeValue v);

SetaeValue setae_from_float(double d);
double setae_to_float(SetaeValue v);
SetaeValue setae_from_int(int32_t i);
int32_t setae_to_int(SetaeValue v);

SetaeValue setae_none(void);
SetaeValue setae_bool(int b);
int setae_to_bool(SetaeValue v);

SetaeValue setae_from_ptr(void *p);
void *setae_to_ptr(SetaeValue v);

typedef enum {
    SETAE_T_BIGINT = 1,
    SETAE_T_STR,
    SETAE_T_LIST,
    SETAE_T_DICT,
    SETAE_T_TUPLE,
    SETAE_T_FUNCTION,
    SETAE_T_CODE,
    SETAE_T_MODULE,
    SETAE_T_BUILTIN,
    SETAE_T_RANGE,
    SETAE_T_ITER,
    SETAE_T_CELL,
} SetaeType;

/* Header of every heap object. The payload follows it in memory. */
typedef struct SetaeObject {
    uint32_t type; /* a SetaeType */
    uint32_t gc;   /* mark, color, and flags */
} SetaeObject;

typedef struct SetaeStr {
    SetaeObject obj;
    uint32_t len;
    char data[]; /* UTF-8 bytes, not NUL-terminated */
} SetaeStr;

struct SetaeVM;

typedef SetaeValue (*SetaeCFunc)(struct SetaeVM *vm, SetaeValue *args, int nargs);

typedef struct SetaeBuiltin {
    SetaeObject obj;
    SetaeCFunc fn;
    const char *name;
} SetaeBuiltin;

/* The type of a value, or -1 if it is immediate, so an int, float, bool, or
   None. */
int setae_obj_type(SetaeValue v);
int setae_is_str(SetaeValue v);
size_t setae_str_len(SetaeValue v);
const char *setae_str_data(SetaeValue v);

/* Owns every object it allocates and frees them all on destroy. Mark-sweep
   tracing comes later, see docs/design/03-gc.md. */
typedef struct SetaeHeap SetaeHeap;

SetaeHeap *setae_heap_new(void);
void setae_heap_destroy(SetaeHeap *h);
size_t setae_heap_live(const SetaeHeap *h);
SetaeValue setae_str_new(SetaeHeap *h, const char *bytes, size_t len);
SetaeValue setae_builtin_new(SetaeHeap *h, SetaeCFunc fn, const char *name);

/* Bytecode. Opcodes are 1 byte plus a 1-byte argument, with OP_EXTENDED_ARG
   shifting in high bits for wider arguments. */
typedef enum {
    OP_LOAD_CONST,  /* arg: const index */
    OP_LOAD_NAME,   /* arg: name index, resolved against globals */
    OP_STORE_NAME,  /* arg: name index */
    OP_LOAD_LOCAL,  /* arg: local slot */
    OP_STORE_LOCAL, /* arg: local slot */
    OP_POP_TOP,
    OP_BINARY_OP, /* arg: SetaeBinOp */
    OP_CALL,      /* arg: argument count */
    OP_RETURN,
    OP_JUMP,                 /* arg: target instruction index */
    OP_POP_JUMP_IF_FALSE,    /* arg: target, pops the tested value */
    OP_POP_JUMP_IF_TRUE,     /* arg: target, pops the tested value */
    OP_JUMP_IF_FALSE_OR_POP, /* arg: target, keeps the value if it jumps */
    OP_JUMP_IF_TRUE_OR_POP,  /* arg: target, keeps the value if it jumps */
    OP_COMPARE_OP,           /* arg: SetaeCmpOp */
    OP_UNARY_NEG,
    OP_UNARY_NOT,
    OP_MAKE_FUNCTION,
    OP_BUILD_LIST,
    OP_BUILD_DICT,
    OP_SUBSCR,
    OP_STORE_SUBSCR,
    OP_GET_ITER,
    OP_FOR_ITER,
    OP_CALL_METHOD,
    OP_EXTENDED_ARG,
    OP_LOAD_CLOSURE,
    OP_LOAD_DEREF,
    OP_STORE_DEREF,
    OP_BUILD_TUPLE,
    OP_UNPACK_SEQUENCE,
} SetaeOp;

typedef enum {
    BIN_ADD,
    BIN_SUB,
    BIN_MUL,
    BIN_DIV,
    BIN_MOD,
    BIN_FLOORDIV,
} SetaeBinOp;

typedef enum {
    CMP_EQ,
    CMP_NE,
    CMP_LT,
    CMP_LE,
    CMP_GT,
    CMP_GE,
    CMP_IN,
    CMP_NOT_IN,
} SetaeCmpOp;

typedef struct SetaeCode SetaeCode;

SetaeCode *setae_code_new(void);
void setae_code_free(SetaeCode *c);
SetaeCode *setae_code_new_child(SetaeCode *parent);
uint32_t setae_code_add_const(SetaeCode *c, SetaeValue v);
uint32_t setae_code_add_name(SetaeCode *c, const char *name);
void setae_code_emit(SetaeCode *c, uint8_t op, uint8_t arg);
void setae_code_set_nlocals(SetaeCode *c, uint32_t n);
void setae_code_set_nparams(SetaeCode *c, uint32_t n);
void setae_code_set_ncells(SetaeCode *c, uint32_t n);
void setae_code_set_nfrees(SetaeCode *c, uint32_t n);
void setae_code_set_name(SetaeCode *c, const char *name);

/* Setae: the bytecode VM. See docs/design/02-bytecode.md. */
typedef struct SetaeVM SetaeVM;

SetaeVM *setae_vm_new(SetaeHeap *h);
void setae_vm_destroy(SetaeVM *vm);
void setae_vm_register_builtins(SetaeVM *vm);
void setae_vm_set_global(SetaeVM *vm, const char *name, SetaeValue v);
SetaeValue setae_vm_run(SetaeVM *vm, SetaeCode *code);
void setae_gc_collect(SetaeVM *vm);
int setae_vm_error(SetaeVM *vm);
const char *setae_vm_error_msg(SetaeVM *vm);
const char *setae_vm_output(SetaeVM *vm, size_t *len);

#endif /* SETAE_H */
