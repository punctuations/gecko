#include "internal.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define STACK_MAX 1024

typedef struct {
    char *name;
    GkValue value;
} Global;

struct GkVM {
    GkHeap *heap;

    Global *globals;
    size_t nglobals;
    size_t globals_cap;

    char *out;
    size_t out_len;
    size_t out_cap;

    int error;
};

GkVM *gk_vm_new(GkHeap *h) {
    GkVM *vm = calloc(1, sizeof(GkVM));
    vm->heap = h;
    return vm;
}

void gk_vm_destroy(GkVM *vm) {
    if (vm == NULL) {
        return;
    }
    for (size_t i = 0; i < vm->nglobals; i++) {
        free(vm->globals[i].name);
    }
    free(vm->globals);
    free(vm->out);
    free(vm);
}

void gk_vm_set_global(GkVM *vm, const char *name, GkValue v) {
    for (size_t i = 0; i < vm->nglobals; i++) {
        if (strcmp(vm->globals[i].name, name) == 0) {
            vm->globals[i].value = v;
            return;
        }
    }
    if (vm->nglobals == vm->globals_cap) {
        vm->globals_cap = vm->globals_cap ? vm->globals_cap * 2 : 8;
        vm->globals = realloc(vm->globals, vm->globals_cap * sizeof(Global));
    }
    size_t n = strlen(name) + 1;
    vm->globals[vm->nglobals].name = malloc(n);
    memcpy(vm->globals[vm->nglobals].name, name, n);
    vm->globals[vm->nglobals].value = v;
    vm->nglobals++;
}

static int global_lookup(GkVM *vm, const char *name, GkValue *out) {
    for (size_t i = 0; i < vm->nglobals; i++) {
        if (strcmp(vm->globals[i].name, name) == 0) {
            *out = vm->globals[i].value;
            return 1;
        }
    }
    return 0;
}

int gk_vm_error(GkVM *vm) {
    return vm->error;
}

const char *gk_vm_output(GkVM *vm, size_t *len) {
    if (len != NULL) {
        *len = vm->out_len;
    }
    return vm->out;
}

void gk_vm_append_output(GkVM *vm, const char *bytes, size_t len) {
    if (vm->out_len + len > vm->out_cap) {
        while (vm->out_cap < vm->out_len + len) {
            vm->out_cap = vm->out_cap ? vm->out_cap * 2 : 64;
        }
        vm->out = realloc(vm->out, vm->out_cap);
    }
    memcpy(vm->out + vm->out_len, bytes, len);
    vm->out_len += len;
}

GkHeap *gk_vm_heap(GkVM *vm) {
    return vm->heap;
}

static double as_number(GkValue v) {
    return gk_is_int(v) ? (double)gk_to_int(v) : gk_to_float(v);
}

static GkValue binary_op(GkVM *vm, GkBinOp op, GkValue a, GkValue b) {
    int numeric = (gk_is_int(a) || gk_is_float(a)) && (gk_is_int(b) || gk_is_float(b));
    if (!numeric) {
        vm->error = 1;
        return gk_none();
    }
    if (gk_is_int(a) && gk_is_int(b) && op != BIN_DIV) {
        int64_t x = gk_to_int(a);
        int64_t y = gk_to_int(b);
        int64_t r = op == BIN_ADD ? x + y : op == BIN_SUB ? x - y : x * y;
        if (r >= INT32_MIN && r <= INT32_MAX) {
            return gk_from_int((int32_t)r);
        }
        return gk_from_float((double)r);
    }
    double x = as_number(a);
    double y = as_number(b);
    double r = op == BIN_ADD   ? x + y
               : op == BIN_SUB ? x - y
               : op == BIN_MUL ? x * y
                               : x / y;
    return gk_from_float(r);
}

static int truthy(GkValue v) {
    if (gk_is_none(v)) {
        return 0;
    }
    if (gk_is_bool(v)) {
        return gk_to_bool(v);
    }
    if (gk_is_int(v)) {
        return gk_to_int(v) != 0;
    }
    if (gk_is_float(v)) {
        return gk_to_float(v) != 0.0;
    }
    if (gk_is_str(v)) {
        return gk_str_len(v) != 0;
    }
    return 1;
}

static int str_eq(GkValue a, GkValue b) {
    size_t n = gk_str_len(a);
    return n == gk_str_len(b) && memcmp(gk_str_data(a), gk_str_data(b), n) == 0;
}

static GkValue compare(GkVM *vm, GkCmpOp op, GkValue a, GkValue b) {
    int an = gk_is_int(a) || gk_is_float(a);
    int bn = gk_is_int(b) || gk_is_float(b);
    if (an && bn) {
        double x = as_number(a);
        double y = as_number(b);
        int r = op == CMP_EQ   ? x == y
                : op == CMP_NE ? x != y
                : op == CMP_LT ? x < y
                : op == CMP_LE ? x <= y
                : op == CMP_GT ? x > y
                               : x >= y;
        return gk_bool(r);
    }
    if (gk_is_str(a) && gk_is_str(b) && (op == CMP_EQ || op == CMP_NE)) {
        int eq = str_eq(a, b);
        return gk_bool(op == CMP_EQ ? eq : !eq);
    }
    if (op == CMP_EQ || op == CMP_NE) {
        int eq = a == b; /* compare identity, which covers None and bool */
        return gk_bool(op == CMP_EQ ? eq : !eq);
    }
    vm->error = 1;
    return gk_none();
}

static GkValue unary_neg(GkVM *vm, GkValue a) {
    if (gk_is_int(a)) {
        int64_t r = -(int64_t)gk_to_int(a);
        if (r >= INT32_MIN && r <= INT32_MAX) {
            return gk_from_int((int32_t)r);
        }
        return gk_from_float((double)r);
    }
    if (gk_is_float(a)) {
        return gk_from_float(-gk_to_float(a));
    }
    vm->error = 1;
    return gk_none();
}

static GkValue call_value(GkVM *vm, GkValue callee, GkValue *args, int nargs) {
    if (gk_obj_type(callee) == GK_T_BUILTIN) {
        GkBuiltin *b = gk_to_ptr(callee);
        return b->fn(vm, args, nargs);
    }
    vm->error = 1;
    return gk_none();
}

GkValue gk_vm_run(GkVM *vm, GkCode *code) {
    const GkValue *consts = gk_code_consts(code);
    uint32_t ncode;
    const uint8_t *bytes = gk_code_bytes(code, &ncode);
    uint32_t nlocals = gk_code_nlocals(code);

    GkValue *locals = nlocals ? calloc(nlocals, sizeof(GkValue)) : NULL;
    GkValue stack[STACK_MAX];
    int sp = 0;

    GkValue result = gk_none();
    uint32_t ip = 0;
    while (ip < ncode && !vm->error) {
        uint8_t op = bytes[ip];
        uint8_t arg = bytes[ip + 1];
        ip += 2;
        switch (op) {
        case OP_LOAD_CONST:
            stack[sp++] = consts[arg];
            break;
        case OP_LOAD_NAME: {
            GkValue v;
            if (!global_lookup(vm, gk_code_name(code, arg), &v)) {
                vm->error = 1;
                break;
            }
            stack[sp++] = v;
            break;
        }
        case OP_STORE_NAME:
            gk_vm_set_global(vm, gk_code_name(code, arg), stack[--sp]);
            break;
        case OP_LOAD_LOCAL:
            stack[sp++] = locals[arg];
            break;
        case OP_STORE_LOCAL:
            locals[arg] = stack[--sp];
            break;
        case OP_POP_TOP:
            sp--;
            break;
        case OP_BINARY_OP: {
            GkValue b = stack[--sp];
            GkValue a = stack[--sp];
            stack[sp++] = binary_op(vm, (GkBinOp)arg, a, b);
            break;
        }
        case OP_CALL: {
            int nargs = arg;
            GkValue *args = &stack[sp - nargs];
            GkValue callee = stack[sp - nargs - 1];
            GkValue r = call_value(vm, callee, args, nargs);
            sp -= nargs + 1;
            stack[sp++] = r;
            break;
        }
        case OP_RETURN:
            result = stack[--sp];
            ip = ncode;
            break;
        case OP_JUMP:
            ip = (uint32_t)arg * 2;
            break;
        case OP_POP_JUMP_IF_FALSE:
            if (!truthy(stack[--sp])) {
                ip = (uint32_t)arg * 2;
            }
            break;
        case OP_POP_JUMP_IF_TRUE:
            if (truthy(stack[--sp])) {
                ip = (uint32_t)arg * 2;
            }
            break;
        case OP_JUMP_IF_FALSE_OR_POP:
            if (!truthy(stack[sp - 1])) {
                ip = (uint32_t)arg * 2;
            } else {
                sp--;
            }
            break;
        case OP_JUMP_IF_TRUE_OR_POP:
            if (truthy(stack[sp - 1])) {
                ip = (uint32_t)arg * 2;
            } else {
                sp--;
            }
            break;
        case OP_COMPARE_OP: {
            GkValue b = stack[--sp];
            GkValue a = stack[--sp];
            stack[sp++] = compare(vm, (GkCmpOp)arg, a, b);
            break;
        }
        case OP_UNARY_NEG:
            stack[sp - 1] = unary_neg(vm, stack[sp - 1]);
            break;
        case OP_UNARY_NOT:
            stack[sp - 1] = gk_bool(!truthy(stack[sp - 1]));
            break;
        default:
            vm->error = 1;
            break;
        }
    }

    free(locals);
    return result;
}
