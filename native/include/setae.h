#ifndef SETAE_H
#define SETAE_H

#include <stddef.h>
#include <stdint.h>

/* NaN-boxed. Encoding in docs/design/01-object-model.md. */
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
    SETAE_T_EXCTYPE,
    SETAE_T_EXC,
    SETAE_T_CLASS,
    SETAE_T_INSTANCE,
    SETAE_T_BOUND,
    SETAE_T_SUBJECT,
    SETAE_T_STOP,
    SETAE_T_GEN,
} SetaeType;

/* One-word header; the payload follows it in memory. */
typedef struct SetaeObject {
    uint32_t type;
    uint32_t gc;
} SetaeObject;

typedef struct SetaeStr {
    SetaeObject obj;
    uint32_t len;
    char data[]; /* UTF-8, not NUL-terminated */
} SetaeStr;

struct SetaeVM;

typedef SetaeValue (*SetaeCFunc)(struct SetaeVM *vm, SetaeValue *args, int nargs);

typedef struct SetaeBuiltin {
    SetaeObject obj;
    SetaeCFunc fn;
    const char *name;
} SetaeBuiltin;

/* The value's type, or -1 when it is immediate (int, float, bool, None). */
int setae_obj_type(SetaeValue v);
int setae_is_str(SetaeValue v);
size_t setae_str_len(SetaeValue v);
const char *setae_str_data(SetaeValue v);

/* Owns every object it allocates and frees them all on destroy. */
typedef struct SetaeHeap SetaeHeap;

SetaeHeap *setae_heap_new(void);
void setae_heap_destroy(SetaeHeap *h);
size_t setae_heap_live(const SetaeHeap *h);
void setae_heap_set_limit(SetaeHeap *h, size_t max_objects);
SetaeValue setae_str_new(SetaeHeap *h, const char *bytes, size_t len);
SetaeValue setae_builtin_new(SetaeHeap *h, SetaeCFunc fn, const char *name);

/* Wordcode: one opcode byte plus one arg byte. OP_EXTENDED_ARG prefixes shift
   in the high bits of a wider arg. Jump args are instruction indices, not byte
   offsets. */
typedef enum {
    OP_LOAD_CONST,
    OP_LOAD_NAME,
    OP_STORE_NAME,
    OP_LOAD_LOCAL,
    OP_STORE_LOCAL,
    OP_POP_TOP,
    OP_BINARY_OP,
    OP_CALL,
    OP_RETURN,
    OP_JUMP,
    OP_POP_JUMP_IF_FALSE,
    OP_POP_JUMP_IF_TRUE,
    OP_JUMP_IF_FALSE_OR_POP,
    OP_JUMP_IF_TRUE_OR_POP,
    OP_COMPARE_OP,
    OP_UNARY_NEG,
    OP_UNARY_NOT,
    OP_MAKE_FUNCTION,
    OP_BUILD_LIST,
    OP_BUILD_DICT,
    OP_SUBSCR,
    OP_STORE_SUBSCR,
    OP_GET_ITER,
    OP_FOR_ITER,
    OP_CALL_METHOD, /* arg: name index << 8 | argument count */
    OP_EXTENDED_ARG,
    OP_LOAD_CLOSURE,
    OP_LOAD_DEREF,
    OP_STORE_DEREF,
    OP_BUILD_TUPLE,
    OP_UNPACK_SEQUENCE,
    OP_RAISE,
    OP_EXC_MATCH,
    OP_RERAISE,
    OP_LOAD_ATTR,
    OP_STORE_ATTR,
    OP_MAKE_CLASS,
    OP_IMPORT,
    OP_IMPORT_MISSING,
    OP_CALL_EX,
    OP_LIST_EXTEND,
    OP_DICT_MERGE,
    OP_DUP_TOP,
    OP_DELETE_NAME,
    OP_DELETE_SUBSCR,
    OP_DELETE_ATTR,
    OP_DELETE_LOCAL,
    OP_ROT_TWO,
    OP_ROT_THREE,
    OP_DELETE_DEREF,
    OP_FORMAT_VALUE,
    OP_YIELD_VALUE,
    OP_AWAIT,
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
    CMP_IS,
    CMP_IS_NOT,
} SetaeCmpOp;

typedef struct SetaeCode SetaeCode;

SetaeCode *setae_code_new(void);
void setae_code_free(SetaeCode *c);
SetaeCode *setae_code_new_child(SetaeCode *parent);
SetaeCode *setae_code_new_module(SetaeCode *parent);
void setae_code_set_module_parent(SetaeCode *c, int32_t parent);
uint32_t setae_code_add_const(SetaeCode *c, SetaeValue v);
uint32_t setae_code_add_name(SetaeCode *c, const char *name);
void setae_code_emit(SetaeCode *c, uint8_t op, uint8_t arg);
void setae_code_set_nlocals(SetaeCode *c, uint32_t n);
void setae_code_set_nparams(SetaeCode *c, uint32_t n);
void setae_code_set_ndefaults(SetaeCode *c, uint32_t n);
void setae_code_add_param_name(SetaeCode *c, const char *name);
void setae_code_set_variadic(SetaeCode *c, uint8_t varargs, uint8_t kwargs);
void setae_code_set_generator(SetaeCode *c, uint8_t generator);
void setae_code_set_coroutine(SetaeCode *c, uint8_t coroutine);
void setae_code_set_ncells(SetaeCode *c, uint32_t n);
void setae_code_set_nfrees(SetaeCode *c, uint32_t n);
void setae_code_add_exc(SetaeCode *c, uint32_t start, uint32_t end, uint32_t target,
                        uint32_t depth);
void setae_code_set_name(SetaeCode *c, const char *name);

typedef struct SetaeVM SetaeVM;

typedef struct SetaeMsg SetaeMsg;
SetaeMsg *setae_msg_read(SetaeVM *vm, SetaeValue v);
SetaeValue setae_msg_write(SetaeVM *vm, const SetaeMsg *m);
void setae_msg_free(SetaeMsg *m);

SetaeValue setae_subject_new(SetaeHeap *h, void *mailbox);
SetaeValue setae_stop_new(SetaeHeap *h);
void *setae_subject_mailbox(SetaeValue v);
void setae_set_subject_drop(void (*fn)(void *));
void setae_subject_drop_handle(void *mailbox);
void setae_set_subject_clone(void *(*fn)(void *));
void setae_set_subject_send(void (*fn)(void *, SetaeMsg *));
int setae_subject_send_value(SetaeVM *vm, SetaeValue subject, SetaeValue arg);
void setae_set_subject_call(SetaeValue (*fn)(SetaeVM *, SetaeValue, SetaeValue, SetaeValue));
SetaeValue setae_subject_call_value(SetaeVM *vm, SetaeValue subject, SetaeValue build,
                                    SetaeValue timeout);
void setae_vm_push_tmp(SetaeVM *vm, SetaeValue v);
void setae_vm_pop_tmp(SetaeVM *vm);

uint8_t *setae_code_serialize(const SetaeCode *c, size_t *len_out);
const SetaeCode *setae_func_code(SetaeValue func);
void setae_bytes_free(uint8_t *p);
uint32_t setae_tuple_len(SetaeValue tv);
SetaeValue setae_tuple_get(SetaeValue tv, uint32_t i);

SetaeVM *setae_vm_new(SetaeHeap *h);
void setae_vm_destroy(SetaeVM *vm);
void setae_vm_register_builtins(SetaeVM *vm);
void setae_vm_register_builtin(SetaeVM *vm, const char *name, SetaeValue v);
void setae_vm_set_global(SetaeVM *vm, const char *name, SetaeValue v);
typedef SetaeValue (*SetaeSandboxHook)(SetaeVM *vm, const char *src, size_t len,
                                       uint64_t steps, size_t mem, uint64_t millis);

SetaeValue setae_vm_run(SetaeVM *vm, SetaeCode *code);
SetaeValue setae_call(SetaeVM *vm, SetaeValue callee, SetaeValue *args, int nargs);
void setae_vm_clear_error(SetaeVM *vm);
void setae_gecko_actor_register(SetaeVM *vm, const char *name, SetaeValue value);
SetaeValue setae_gecko_actor_module(SetaeVM *vm);
void setae_vm_set_step_limit(SetaeVM *vm, uint64_t limit);
void setae_vm_set_time_limit(SetaeVM *vm, uint64_t millis);
void setae_vm_set_sandbox_hook(SetaeVM *vm, SetaeSandboxHook hook);
SetaeHeap *setae_vm_heap(SetaeVM *vm);
void setae_vm_raise_str(SetaeVM *vm, const char *kind, const char *msg);
void setae_gc_collect(SetaeVM *vm);
int setae_vm_error(SetaeVM *vm);
const char *setae_vm_error_msg(SetaeVM *vm);
const char *setae_vm_output(SetaeVM *vm, size_t *len);

#endif
