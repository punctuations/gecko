#ifndef GECKO_H
#define GECKO_H

#include <stddef.h>
#include <stdint.h>

/* NaN-boxed value word. The encoding is in docs/design/01-object-model.md. */
typedef uint64_t GkValue;

#define GK_NUMBER_TAG    0xffff000000000000ULL
#define GK_DOUBLE_OFFSET 0x0001000000000000ULL

#define GK_VAL_NONE  0x02ULL
#define GK_VAL_FALSE 0x04ULL
#define GK_VAL_TRUE  0x06ULL

int gk_is_float(GkValue v);
int gk_is_int(GkValue v); /* fixnum, not a heap bignum */
int gk_is_ptr(GkValue v);
int gk_is_none(GkValue v);
int gk_is_bool(GkValue v);

GkValue gk_from_float(double d);
double gk_to_float(GkValue v);
GkValue gk_from_int(int32_t i);
int32_t gk_to_int(GkValue v);

GkValue gk_none(void);
GkValue gk_bool(int b);
int gk_to_bool(GkValue v);

GkValue gk_from_ptr(void *p);
void *gk_to_ptr(GkValue v);

typedef enum {
    GK_T_BIGINT = 1,
    GK_T_STR,
    GK_T_LIST,
    GK_T_DICT,
    GK_T_TUPLE,
    GK_T_FUNCTION,
    GK_T_CODE,
    GK_T_MODULE,
    GK_T_BUILTIN,
} GkType;

/* Header of every heap object. The payload follows it in memory. */
typedef struct GkObject {
    uint32_t type; /* a GkType */
    uint32_t gc;   /* mark, color, and flags */
} GkObject;

typedef struct GkStr {
    GkObject obj;
    uint32_t len;
    char data[]; /* UTF-8 bytes, not NUL-terminated */
} GkStr;

struct GkVM;

typedef GkValue (*GkCFunc)(struct GkVM *vm, GkValue *args, int nargs);

typedef struct GkBuiltin {
    GkObject obj;
    GkCFunc fn;
    const char *name;
} GkBuiltin;

/* The type of a value, or -1 if it is immediate, so an int, float, bool, or
   None. */
int gk_obj_type(GkValue v);
int gk_is_str(GkValue v);
size_t gk_str_len(GkValue v);
const char *gk_str_data(GkValue v);

/* Owns every object it allocates and frees them all on destroy. Mark-sweep
   tracing comes later, see docs/design/03-gc.md. */
typedef struct GkHeap GkHeap;

GkHeap *gk_heap_new(void);
void gk_heap_destroy(GkHeap *h);
GkValue gk_str_new(GkHeap *h, const char *bytes, size_t len);
GkValue gk_builtin_new(GkHeap *h, GkCFunc fn, const char *name);

/* Bytecode. Opcodes are 1 byte plus a 1-byte argument. */
typedef enum {
    OP_LOAD_CONST,  /* arg: const index */
    OP_LOAD_NAME,   /* arg: name index, resolved against globals */
    OP_STORE_NAME,  /* arg: name index */
    OP_LOAD_LOCAL,  /* arg: local slot */
    OP_STORE_LOCAL, /* arg: local slot */
    OP_POP_TOP,
    OP_BINARY_OP, /* arg: GkBinOp */
    OP_CALL,      /* arg: argument count */
    OP_RETURN,
    OP_JUMP,                 /* arg: target instruction index */
    OP_POP_JUMP_IF_FALSE,    /* arg: target, pops the tested value */
    OP_POP_JUMP_IF_TRUE,     /* arg: target, pops the tested value */
    OP_JUMP_IF_FALSE_OR_POP, /* arg: target, keeps the value if it jumps */
    OP_JUMP_IF_TRUE_OR_POP,  /* arg: target, keeps the value if it jumps */
    OP_COMPARE_OP,           /* arg: GkCmpOp */
    OP_UNARY_NEG,
    OP_UNARY_NOT,
} GkOp;

typedef enum {
    BIN_ADD,
    BIN_SUB,
    BIN_MUL,
    BIN_DIV,
} GkBinOp;

typedef enum {
    CMP_EQ,
    CMP_NE,
    CMP_LT,
    CMP_LE,
    CMP_GT,
    CMP_GE,
} GkCmpOp;

typedef struct GkCode GkCode;

GkCode *gk_code_new(void);
void gk_code_free(GkCode *c);
uint32_t gk_code_add_const(GkCode *c, GkValue v);
uint32_t gk_code_add_name(GkCode *c, const char *name);
void gk_code_emit(GkCode *c, uint8_t op, uint8_t arg);
void gk_code_set_nlocals(GkCode *c, uint32_t n);

/* Setae: the bytecode VM. See docs/design/02-bytecode.md. */
typedef struct GkVM GkVM;

GkVM *gk_vm_new(GkHeap *h);
void gk_vm_destroy(GkVM *vm);
void gk_vm_register_builtins(GkVM *vm);
void gk_vm_set_global(GkVM *vm, const char *name, GkValue v);
GkValue gk_vm_run(GkVM *vm, GkCode *code);
int gk_vm_error(GkVM *vm);
const char *gk_vm_output(GkVM *vm, size_t *len);

#endif /* GECKO_H */
