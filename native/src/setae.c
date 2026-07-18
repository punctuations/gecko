#define _POSIX_C_SOURCE 200809L

#include "internal.h"

#include <math.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define STACK_MAX 1024
#define MAX_DEPTH 500
#define FRAME_POOL_MAX 256

static SetaeValue *frame_alloc(SetaeVM *vm, uint32_t need, uint32_t *out_cap) {
    if (vm->frame_pool_n > 0 && vm->frame_pool_caps[vm->frame_pool_n - 1] >= need) {
        vm->frame_pool_n--;
        *out_cap = vm->frame_pool_caps[vm->frame_pool_n];
        return vm->frame_pool[vm->frame_pool_n];
    }
    *out_cap = need;
    return malloc(need * sizeof(SetaeValue));
}

static void frame_release(SetaeVM *vm, SetaeValue *buf, uint32_t cap) {
    if (vm->frame_pool_n >= FRAME_POOL_MAX) {
        free(buf);
        return;
    }
    if (vm->frame_pool_n == vm->frame_pool_cap) {
        vm->frame_pool_cap = vm->frame_pool_cap ? vm->frame_pool_cap * 2 : 16;
        vm->frame_pool = realloc(vm->frame_pool, vm->frame_pool_cap * sizeof(SetaeValue *));
        vm->frame_pool_caps =
            realloc(vm->frame_pool_caps, vm->frame_pool_cap * sizeof(uint32_t));
    }
    vm->frame_pool[vm->frame_pool_n] = buf;
    vm->frame_pool_caps[vm->frame_pool_n] = cap;
    vm->frame_pool_n++;
}

SetaeVM *setae_vm_new(SetaeHeap *h) {
    SetaeVM *vm = calloc(1, sizeof(SetaeVM));
    vm->heap = h;
    setae_heap_bind(h, vm);
    return vm;
}

void setae_vm_destroy(SetaeVM *vm) {
    if (vm == NULL) {
        return;
    }
    for (size_t i = 0; i < vm->nglobals; i++) {
        free(vm->globals[i].name);
    }
    free(vm->globals);
    free(vm->globals_index);
    for (size_t i = 0; i < vm->nbuiltins; i++) {
        free(vm->builtins[i].name);
    }
    free(vm->builtins);
    free(vm->builtins_index);
    free(vm->module_cache);
    free(vm->codes);
    free(vm->out);
    for (size_t i = 0; i < vm->frame_pool_n; i++) {
        free(vm->frame_pool[i]);
    }
    free(vm->frame_pool);
    free(vm->frame_pool_caps);
    free(vm);
}

static void attach_code(SetaeVM *vm, const SetaeCode *code) {
    for (size_t i = 0; i < vm->ncodes; i++) {
        if (vm->codes[i] == code) {
            return;
        }
    }
    if (vm->ncodes == vm->codes_cap) {
        vm->codes_cap = vm->codes_cap ? vm->codes_cap * 2 : 4;
        vm->codes = realloc(vm->codes, vm->codes_cap * sizeof(SetaeCode *));
    }
    vm->codes[vm->ncodes++] = code;
}

#define TAB_EMPTY 0xFFFFFFFFu

static int64_t tab_find(const SetaeGlobal *arr, size_t n, const uint32_t *index,
                        uint32_t cap, const char *name) {
    if (index == NULL) {
        for (size_t i = 0; i < n; i++) {
            if (strcmp(arr[i].name, name) == 0) {
                return (int64_t)i;
            }
        }
        return -1;
    }
    uint64_t mask = cap - 1;
    uint64_t slot = setae_hash_bytes(name, strlen(name)) & mask;
    while (index[slot] != TAB_EMPTY) {
        uint32_t e = index[slot];
        if (strcmp(arr[e].name, name) == 0) {
            return (int64_t)e;
        }
        slot = (slot + 1) & mask;
    }
    return -1;
}

static void tab_index_add(const SetaeGlobal *arr, size_t n, uint32_t **pindex,
                          uint32_t *pcap, uint32_t entry) {
    if (*pindex == NULL) {
        if (n <= 8) {
            return;
        }
        *pcap = 16;
        while (*pcap < n * 2) {
            *pcap <<= 1;
        }
    } else if ((uint64_t)n * 3 >= (uint64_t)*pcap * 2) {
        *pcap <<= 1;
    } else {
        uint64_t mask = *pcap - 1;
        uint64_t slot = setae_hash_bytes(arr[entry].name, strlen(arr[entry].name)) & mask;
        while ((*pindex)[slot] != TAB_EMPTY) {
            slot = (slot + 1) & mask;
        }
        (*pindex)[slot] = entry;
        return;
    }
    free(*pindex);
    *pindex = malloc(*pcap * sizeof(uint32_t));
    for (uint32_t i = 0; i < *pcap; i++) {
        (*pindex)[i] = TAB_EMPTY;
    }
    uint64_t mask = *pcap - 1;
    for (size_t e = 0; e < n; e++) {
        uint64_t slot = setae_hash_bytes(arr[e].name, strlen(arr[e].name)) & mask;
        while ((*pindex)[slot] != TAB_EMPTY) {
            slot = (slot + 1) & mask;
        }
        (*pindex)[slot] = (uint32_t)e;
    }
}

void setae_vm_set_global(SetaeVM *vm, const char *name, SetaeValue v) {
    int64_t found = tab_find(vm->globals, vm->nglobals, vm->globals_index,
                             vm->globals_index_cap, name);
    if (found >= 0) {
        vm->globals[found].value = v;
        return;
    }
    if (vm->nglobals == vm->globals_cap) {
        vm->globals_cap = vm->globals_cap ? vm->globals_cap * 2 : 8;
        vm->globals = realloc(vm->globals, vm->globals_cap * sizeof(SetaeGlobal));
    }
    size_t n = strlen(name) + 1;
    vm->globals[vm->nglobals].name = malloc(n);
    memcpy(vm->globals[vm->nglobals].name, name, n);
    vm->globals[vm->nglobals].value = v;
    vm->nglobals++;
    tab_index_add(vm->globals, vm->nglobals, &vm->globals_index, &vm->globals_index_cap,
                  (uint32_t)(vm->nglobals - 1));
}

void setae_vm_register_builtin(SetaeVM *vm, const char *name, SetaeValue v) {
    if (vm->nbuiltins == vm->builtins_cap) {
        vm->builtins_cap = vm->builtins_cap ? vm->builtins_cap * 2 : 16;
        vm->builtins = realloc(vm->builtins, vm->builtins_cap * sizeof(SetaeGlobal));
    }
    size_t n = strlen(name) + 1;
    vm->builtins[vm->nbuiltins].name = malloc(n);
    memcpy(vm->builtins[vm->nbuiltins].name, name, n);
    vm->builtins[vm->nbuiltins].value = v;
    vm->nbuiltins++;
    tab_index_add(vm->builtins, vm->nbuiltins, &vm->builtins_index, &vm->builtins_index_cap,
                  (uint32_t)(vm->nbuiltins - 1));
}

int setae_vm_error(SetaeVM *vm) {
    return vm->error;
}

const char *setae_vm_error_msg(SetaeVM *vm) {
    return vm->errmsg;
}

void setae_vm_raise(SetaeVM *vm, const char *kind, const char *fmt, ...) {
    if (vm->error) {
        return;
    }
    char msg[112];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(msg, sizeof(msg), fmt, ap);
    va_end(ap);
    SetaeValue message = setae_none();
    if (msg[0] != '\0') {
        message = setae_str_new(vm->heap, msg, strlen(msg));
    }
    setae_vm_push_tmp(vm, message);
    vm->exc = setae_exc_new(vm->heap, kind, message);
    setae_vm_pop_tmp(vm);
    vm->error = 1;
    if (msg[0] != '\0') {
        snprintf(vm->errmsg, sizeof(vm->errmsg), "%s: %s", kind, msg);
    } else {
        snprintf(vm->errmsg, sizeof(vm->errmsg), "%s", kind);
    }
}

void setae_vm_oom(SetaeVM *vm) {
    if (vm->error) {
        return;
    }
    vm->error = 1;
    if (vm->oom != 0) {
        vm->exc = vm->oom;
    }
    snprintf(vm->errmsg, sizeof(vm->errmsg), "MemoryError: heap object limit exceeded");
}

void setae_vm_set_step_limit(SetaeVM *vm, uint64_t limit) {
    vm->step_limit = limit;
}

static uint64_t monotonic_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}

void setae_vm_set_time_limit(SetaeVM *vm, uint64_t millis) {
    vm->time_limit_ns = millis * 1000000ull;
}

void setae_vm_set_sandbox_hook(SetaeVM *vm, SetaeSandboxHook hook) {
    vm->sandbox_hook = hook;
}

void setae_vm_raise_str(SetaeVM *vm, const char *kind, const char *msg) {
    setae_vm_raise(vm, kind, "%s", msg);
}

static void raise_pending(SetaeVM *vm, SetaeValue exc) {
    SetaeExc *e = setae_to_ptr(exc);
    vm->exc = exc;
    vm->error = 1;
    if (setae_is_str(e->message)) {
        snprintf(vm->errmsg, sizeof(vm->errmsg), "%s: %.*s", e->kind,
                 (int)setae_str_len(e->message), setae_str_data(e->message));
    } else {
        snprintf(vm->errmsg, sizeof(vm->errmsg), "%s", e->kind);
    }
}

static void raise_value(SetaeVM *vm, SetaeValue v) {
    int t = setae_obj_type(v);
    if (t == SETAE_T_EXC) {
        raise_pending(vm, v);
        return;
    }
    if (t == SETAE_T_EXCTYPE) {
        SetaeExcType *et = setae_to_ptr(v);
        raise_pending(vm, setae_exc_new(vm->heap, et->name, setae_none()));
        return;
    }
    setae_vm_raise(vm, "TypeError", "exceptions must derive from BaseException");
}

static int exc_matches(SetaeVM *vm, SetaeValue exc, SetaeValue type) {
    if (setae_obj_type(type) == SETAE_T_TUPLE) {
        SetaeTuple *t = setae_to_ptr(type);
        for (uint32_t i = 0; i < t->len; i++) {
            if (exc_matches(vm, exc, t->items[i])) {
                return 1;
            }
            if (vm->error) {
                return 0;
            }
        }
        return 0;
    }
    if (setae_obj_type(type) != SETAE_T_EXCTYPE) {
        setae_vm_raise(
            vm, "TypeError",
            "catching classes that do not inherit from BaseException is not allowed");
        return 0;
    }
    SetaeExcType *et = setae_to_ptr(type);
    SetaeExc *e = setae_to_ptr(exc);
    return strcmp(et->name, "Exception") == 0 || strcmp(et->name, e->kind) == 0;
}

const char *setae_vm_output(SetaeVM *vm, size_t *len) {
    if (len != NULL) {
        *len = vm->out_len;
    }
    return vm->out;
}

void setae_vm_append_output(SetaeVM *vm, const char *bytes, size_t len) {
    if (vm->out_len + len > vm->out_cap) {
        while (vm->out_cap < vm->out_len + len) {
            vm->out_cap = vm->out_cap ? vm->out_cap * 2 : 64;
        }
        vm->out = realloc(vm->out, vm->out_cap);
    }
    memcpy(vm->out + vm->out_len, bytes, len);
    vm->out_len += len;
}

SetaeHeap *setae_vm_heap(SetaeVM *vm) {
    return vm->heap;
}

void setae_vm_push_tmp(SetaeVM *vm, SetaeValue v) {
    vm->tmp_roots[vm->ntmp++] = v;
}

void setae_vm_pop_tmp(SetaeVM *vm) {
    vm->ntmp--;
}

static double as_number(SetaeValue v) {
    return setae_is_int(v) ? (double)setae_to_int(v) : setae_to_float(v);
}

static SetaeValue from_i64(int64_t i) {
    if (i >= INT32_MIN && i <= INT32_MAX) {
        return setae_from_int((int32_t)i);
    }
    return setae_from_float((double)i);
}

static int hashable(SetaeValue v) {
    int t = setae_obj_type(v);
    if (t == SETAE_T_LIST || t == SETAE_T_DICT) {
        return 0;
    }
    if (t == SETAE_T_TUPLE) {
        SetaeTuple *tup = setae_to_ptr(v);
        for (uint32_t i = 0; i < tup->len; i++) {
            if (!hashable(tup->items[i])) {
                return 0;
            }
        }
    }
    return 1;
}

static int64_t dict_find(const SetaeDict *d, SetaeValue key) {
    if (d->index != NULL) {
        return setae_dict_index_get(d, key);
    }
    for (uint32_t i = 0; i < d->len; i++) {
        if (setae_value_eq(d->entries[i].key, key)) {
            return (int64_t)i;
        }
    }
    return -1;
}

static void dict_set(SetaeDict *d, SetaeValue key, SetaeValue value) {
    int64_t i = dict_find(d, key);
    if (i >= 0) {
        d->entries[i].value = value;
    } else {
        setae_dict_push(d, key, value);
    }
}

static int64_t dict_find_cstr(const SetaeDict *d, const char *name) {
    size_t n = strlen(name);
    if (d->index != NULL) {
        return setae_dict_index_get_cstr(d, name, n);
    }
    for (uint32_t i = 0; i < d->len; i++) {
        SetaeValue k = d->entries[i].key;
        if (setae_is_str(k) && setae_str_len(k) == n &&
            memcmp(setae_str_data(k), name, n) == 0) {
            return (int64_t)i;
        }
    }
    return -1;
}

static int class_lookup(SetaeValue cls, const char *name, SetaeValue *out) {
    while (setae_obj_type(cls) == SETAE_T_CLASS) {
        SetaeClass *c = setae_to_ptr(cls);
        SetaeDict *d = setae_to_ptr(c->dict);
        int64_t i = dict_find_cstr(d, name);
        if (i >= 0) {
            *out = d->entries[i].value;
            return 1;
        }
        cls = c->base;
    }
    return 0;
}

static void attr_error(SetaeVM *vm, SetaeValue obj, const char *name) {
    if (setae_obj_type(obj) == SETAE_T_INSTANCE) {
        SetaeInstance *inst = setae_to_ptr(obj);
        SetaeClass *c = setae_to_ptr(inst->cls);
        setae_vm_raise(vm, "AttributeError", "'%.*s' object has no attribute '%s'",
                       (int)setae_str_len(c->name), setae_str_data(c->name), name);
    } else {
        setae_vm_raise(vm, "AttributeError", "'%s' object has no attribute '%s'",
                       setae_type_name(obj), name);
    }
}

static SetaeValue load_attr(SetaeVM *vm, SetaeValue obj, const char *name) {
    int t = setae_obj_type(obj);
    if (t == SETAE_T_INSTANCE) {
        SetaeInstance *inst = setae_to_ptr(obj);
        SetaeValue found;
        if (setae_instance_get(inst, name, &found)) {
            return found;
        }
        SetaeValue v;
        if (class_lookup(inst->cls, name, &v)) {
            if (setae_obj_type(v) == SETAE_T_FUNCTION) {
                return setae_bound_new(vm->heap, v, obj);
            }
            return v;
        }
        attr_error(vm, obj, name);
        return setae_none();
    }
    if (t == SETAE_T_CLASS) {
        SetaeValue v;
        if (class_lookup(obj, name, &v)) {
            return v;
        }
        SetaeClass *c = setae_to_ptr(obj);
        setae_vm_raise(vm, "AttributeError", "type object '%.*s' has no attribute '%s'",
                       (int)setae_str_len(c->name), setae_str_data(c->name), name);
        return setae_none();
    }
    if (t == SETAE_T_MODULE) {
        SetaeModule *m = setae_to_ptr(obj);
        SetaeDict *d = setae_to_ptr(m->dict);
        int64_t i = dict_find_cstr(d, name);
        if (i >= 0) {
            return d->entries[i].value;
        }
        setae_vm_raise(vm, "AttributeError", "module '%.*s' has no attribute '%s'",
                       (int)setae_str_len(m->name), setae_str_data(m->name), name);
        return setae_none();
    }
    attr_error(vm, obj, name);
    return setae_none();
}

static const char *bin_symbol(SetaeBinOp op, int aug) {
    static const char *const plain[] = {"+", "-", "*", "/", "%", "//"};
    static const char *const augmented[] = {"+=", "-=", "*=", "/=", "%=", "//="};
    return aug ? augmented[op] : plain[op];
}

static SetaeValue binary_op(SetaeVM *vm, SetaeBinOp op, int aug, SetaeValue a,
                            SetaeValue b) {
    if (op == BIN_ADD && setae_is_str(a) && setae_is_str(b)) {
        size_t na = setae_str_len(a);
        size_t nb = setae_str_len(b);
        char *buf = malloc(na + nb);
        memcpy(buf, setae_str_data(a), na);
        memcpy(buf + na, setae_str_data(b), nb);
        SetaeValue r = setae_str_new(vm->heap, buf, na + nb);
        free(buf);
        return r;
    }
    if (op == BIN_ADD && setae_obj_type(a) == SETAE_T_TUPLE &&
        setae_obj_type(b) == SETAE_T_TUPLE) {
        SetaeTuple *ta = setae_to_ptr(a);
        SetaeTuple *tb = setae_to_ptr(b);
        SetaeValue rv = setae_tuple_new(vm->heap, NULL, ta->len + tb->len);
        SetaeTuple *r = setae_to_ptr(rv);
        memcpy(r->items, ta->items, ta->len * sizeof(SetaeValue));
        memcpy(r->items + ta->len, tb->items, tb->len * sizeof(SetaeValue));
        return rv;
    }
    if (op == BIN_ADD && setae_obj_type(a) == SETAE_T_LIST &&
        setae_obj_type(b) == SETAE_T_LIST) {
        SetaeList *la = setae_to_ptr(a);
        SetaeList *lb = setae_to_ptr(b);
        SetaeValue rv = setae_list_new(vm->heap, la->len + lb->len);
        SetaeList *r = setae_to_ptr(rv);
        for (uint32_t i = 0; i < la->len; i++) {
            setae_list_push(r, la->items[i]);
        }
        for (uint32_t i = 0; i < lb->len; i++) {
            setae_list_push(r, lb->items[i]);
        }
        return rv;
    }
    int numeric =
        (setae_is_int(a) || setae_is_float(a)) && (setae_is_int(b) || setae_is_float(b));
    if (!numeric) {
        setae_vm_raise(vm, "TypeError", "unsupported operand type(s) for %s: '%s' and '%s'",
                       bin_symbol(op, aug), setae_type_name(a), setae_type_name(b));
        return setae_none();
    }
    if (setae_is_int(a) && setae_is_int(b) && op != BIN_DIV) {
        int64_t x = setae_to_int(a);
        int64_t y = setae_to_int(b);
        if (op == BIN_MOD || op == BIN_FLOORDIV) {
            if (y == 0) {
                setae_vm_raise(vm, "ZeroDivisionError", "integer division or modulo by zero");
                return setae_none();
            }
            if (op == BIN_MOD) {
                int64_t r = x % y;
                if (r != 0 && (r < 0) != (y < 0)) {
                    r += y;
                }
                return from_i64(r);
            }
            int64_t q = x / y;
            if (x % y != 0 && (x < 0) != (y < 0)) {
                q--;
            }
            return from_i64(q);
        }
        int64_t r = op == BIN_ADD ? x + y : op == BIN_SUB ? x - y : x * y;
        return from_i64(r);
    }
    double x = as_number(a);
    double y = as_number(b);
    switch (op) {
    case BIN_ADD:
        return setae_from_float(x + y);
    case BIN_SUB:
        return setae_from_float(x - y);
    case BIN_MUL:
        return setae_from_float(x * y);
    case BIN_DIV:
        if (y == 0.0) {
            setae_vm_raise(vm, "ZeroDivisionError", "division by zero");
            return setae_none();
        }
        return setae_from_float(x / y);
    case BIN_MOD: {
        if (y == 0.0) {
            setae_vm_raise(vm, "ZeroDivisionError", "float modulo");
            return setae_none();
        }
        double r = fmod(x, y);
        if (r != 0.0 && (r < 0.0) != (y < 0.0)) {
            r += y;
        }
        return setae_from_float(r);
    }
    default:
        if (y == 0.0) {
            setae_vm_raise(vm, "ZeroDivisionError", "float floor division by zero");
            return setae_none();
        }
        return setae_from_float(floor(x / y));
    }
}

static int truthy(SetaeValue v) {
    if (setae_is_none(v)) {
        return 0;
    }
    if (setae_is_bool(v)) {
        return setae_to_bool(v);
    }
    if (setae_is_int(v)) {
        return setae_to_int(v) != 0;
    }
    if (setae_is_float(v)) {
        return setae_to_float(v) != 0.0;
    }
    switch (setae_obj_type(v)) {
    case SETAE_T_STR:
        return setae_str_len(v) != 0;
    case SETAE_T_LIST:
        return ((SetaeList *)setae_to_ptr(v))->len != 0;
    case SETAE_T_TUPLE:
        return ((SetaeTuple *)setae_to_ptr(v))->len != 0;
    case SETAE_T_DICT:
        return ((SetaeDict *)setae_to_ptr(v))->len != 0;
    case SETAE_T_RANGE:
        return setae_range_len(setae_to_ptr(v)) != 0;
    default:
        return 1;
    }
}

static int str_order(SetaeValue a, SetaeValue b) {
    size_t na = setae_str_len(a);
    size_t nb = setae_str_len(b);
    size_t n = na < nb ? na : nb;
    int c = memcmp(setae_str_data(a), setae_str_data(b), n);
    if (c != 0) {
        return c;
    }
    return na < nb ? -1 : na > nb ? 1 : 0;
}

static int contains(SetaeVM *vm, SetaeValue container, SetaeValue x) {
    switch (setae_obj_type(container)) {
    case SETAE_T_LIST: {
        SetaeList *l = setae_to_ptr(container);
        for (uint32_t i = 0; i < l->len; i++) {
            if (setae_value_eq(l->items[i], x)) {
                return 1;
            }
        }
        return 0;
    }
    case SETAE_T_TUPLE: {
        SetaeTuple *t = setae_to_ptr(container);
        for (uint32_t i = 0; i < t->len; i++) {
            if (setae_value_eq(t->items[i], x)) {
                return 1;
            }
        }
        return 0;
    }
    case SETAE_T_DICT:
        return dict_find(setae_to_ptr(container), x) >= 0;
    case SETAE_T_STR: {
        if (!setae_is_str(x)) {
            setae_vm_raise(
                vm, "TypeError", "'in <string>' requires string as left operand, not %s",
                setae_type_name(x));
            return 0;
        }
        size_t nh = setae_str_len(container);
        size_t nn = setae_str_len(x);
        if (nn > nh) {
            return 0;
        }
        const char *h = setae_str_data(container);
        const char *nd = setae_str_data(x);
        for (size_t i = 0; i + nn <= nh; i++) {
            if (memcmp(h + i, nd, nn) == 0) {
                return 1;
            }
        }
        return 0;
    }
    case SETAE_T_RANGE: {
        if (!setae_is_int(x)) {
            return 0;
        }
        SetaeRange *r = setae_to_ptr(container);
        int64_t v = setae_to_int(x);
        if (r->step > 0 && (v < r->start || v >= r->stop)) {
            return 0;
        }
        if (r->step < 0 && (v > r->start || v <= r->stop)) {
            return 0;
        }
        return (v - r->start) % r->step == 0;
    }
    default:
        setae_vm_raise(vm, "TypeError", "argument of type '%s' is not iterable",
                       setae_type_name(container));
        return 0;
    }
}

static SetaeValue compare(SetaeVM *vm, SetaeCmpOp op, SetaeValue a, SetaeValue b) {
    if (op == CMP_IS || op == CMP_IS_NOT) {
        int same = a == b;
        return setae_bool(op == CMP_IS ? same : !same);
    }
    if (op == CMP_IN || op == CMP_NOT_IN) {
        int r = contains(vm, b, a);
        if (vm->error) {
            return setae_none();
        }
        return setae_bool(op == CMP_IN ? r : !r);
    }
    if (op == CMP_EQ || op == CMP_NE) {
        int eq = setae_value_eq(a, b);
        return setae_bool(op == CMP_EQ ? eq : !eq);
    }
    int an = setae_is_int(a) || setae_is_float(a);
    int bn = setae_is_int(b) || setae_is_float(b);
    int c;
    if (an && bn) {
        double x = as_number(a);
        double y = as_number(b);
        c = x < y ? -1 : x > y ? 1 : 0;
    } else if (setae_is_str(a) && setae_is_str(b)) {
        c = str_order(a, b);
    } else {
        setae_vm_raise(vm, "TypeError", "comparison not supported between '%s' and '%s'",
                       setae_type_name(a), setae_type_name(b));
        return setae_none();
    }
    int r = op == CMP_LT ? c < 0 : op == CMP_LE ? c <= 0 : op == CMP_GT ? c > 0 : c >= 0;
    return setae_bool(r);
}

static SetaeValue unary_neg(SetaeVM *vm, SetaeValue a) {
    if (setae_is_int(a)) {
        return from_i64(-(int64_t)setae_to_int(a));
    }
    if (setae_is_float(a)) {
        return setae_from_float(-setae_to_float(a));
    }
    setae_vm_raise(vm, "TypeError", "bad operand type for unary -: '%s'",
                   setae_type_name(a));
    return setae_none();
}

static SetaeValue str_char_at(SetaeVM *vm, SetaeValue s, size_t cp) {
    const char *p = setae_str_data(s);
    size_t n = setae_str_len(s);
    size_t count = 0;
    for (size_t i = 0; i < n;) {
        size_t charlen = 1;
        unsigned char c = (unsigned char)p[i];
        if (c >= 0xf0) {
            charlen = 4;
        } else if (c >= 0xe0) {
            charlen = 3;
        } else if (c >= 0xc0) {
            charlen = 2;
        }
        if (count == cp) {
            return setae_str_new(vm->heap, p + i, charlen);
        }
        count++;
        i += charlen;
    }
    setae_vm_raise(vm, "IndexError", "string index out of range");
    return setae_none();
}

static SetaeValue subscript(SetaeVM *vm, SetaeValue obj, SetaeValue idx) {
    switch (setae_obj_type(obj)) {
    case SETAE_T_LIST: {
        SetaeList *l = setae_to_ptr(obj);
        if (!setae_is_int(idx)) {
            setae_vm_raise(vm, "TypeError", "list indices must be integers, not %s",
                           setae_type_name(idx));
            return setae_none();
        }
        int64_t i = setae_to_int(idx);
        if (i < 0) {
            i += l->len;
        }
        if (i < 0 || i >= (int64_t)l->len) {
            setae_vm_raise(vm, "IndexError", "list index out of range");
            return setae_none();
        }
        return l->items[i];
    }
    case SETAE_T_TUPLE: {
        SetaeTuple *t = setae_to_ptr(obj);
        if (!setae_is_int(idx)) {
            setae_vm_raise(vm, "TypeError", "tuple indices must be integers, not %s",
                           setae_type_name(idx));
            return setae_none();
        }
        int64_t i = setae_to_int(idx);
        if (i < 0) {
            i += t->len;
        }
        if (i < 0 || i >= (int64_t)t->len) {
            setae_vm_raise(vm, "IndexError", "tuple index out of range");
            return setae_none();
        }
        return t->items[i];
    }
    case SETAE_T_DICT: {
        SetaeDict *d = setae_to_ptr(obj);
        if (!hashable(idx)) {
            setae_vm_raise(vm, "TypeError", "unhashable type: '%s'", setae_type_name(idx));
            return setae_none();
        }
        int64_t i = dict_find(d, idx);
        if (i < 0) {
            setae_vm_raise(vm, "KeyError", "");
            return setae_none();
        }
        return d->entries[i].value;
    }
    case SETAE_T_STR: {
        if (!setae_is_int(idx)) {
            setae_vm_raise(vm, "TypeError", "string indices must be integers, not %s",
                           setae_type_name(idx));
            return setae_none();
        }
        int64_t i = setae_to_int(idx);
        int64_t n = (int64_t)setae_str_count(obj);
        if (i < 0) {
            i += n;
        }
        if (i < 0 || i >= n) {
            setae_vm_raise(vm, "IndexError", "string index out of range");
            return setae_none();
        }
        return str_char_at(vm, obj, (size_t)i);
    }
    case SETAE_T_RANGE: {
        SetaeRange *r = setae_to_ptr(obj);
        if (!setae_is_int(idx)) {
            setae_vm_raise(vm, "TypeError", "range indices must be integers, not %s",
                           setae_type_name(idx));
            return setae_none();
        }
        int64_t i = setae_to_int(idx);
        int64_t n = setae_range_len(r);
        if (i < 0) {
            i += n;
        }
        if (i < 0 || i >= n) {
            setae_vm_raise(vm, "IndexError", "range object index out of range");
            return setae_none();
        }
        return from_i64(r->start + i * r->step);
    }
    default:
        setae_vm_raise(vm, "TypeError", "'%s' object is not subscriptable",
                       setae_type_name(obj));
        return setae_none();
    }
}

static void store_subscript(SetaeVM *vm, SetaeValue obj, SetaeValue idx, SetaeValue val) {
    switch (setae_obj_type(obj)) {
    case SETAE_T_LIST: {
        SetaeList *l = setae_to_ptr(obj);
        if (!setae_is_int(idx)) {
            setae_vm_raise(vm, "TypeError", "list indices must be integers, not %s",
                           setae_type_name(idx));
            return;
        }
        int64_t i = setae_to_int(idx);
        if (i < 0) {
            i += l->len;
        }
        if (i < 0 || i >= (int64_t)l->len) {
            setae_vm_raise(vm, "IndexError", "list assignment index out of range");
            return;
        }
        l->items[i] = val;
        return;
    }
    case SETAE_T_DICT: {
        if (!hashable(idx)) {
            setae_vm_raise(vm, "TypeError", "unhashable type: '%s'", setae_type_name(idx));
            return;
        }
        dict_set(setae_to_ptr(obj), idx, val);
        return;
    }
    default:
        setae_vm_raise(vm, "TypeError", "'%s' object does not support item assignment",
                       setae_type_name(obj));
    }
}

static int iter_next(SetaeVM *vm, SetaeIter *it, SetaeValue *out) {
    switch (setae_obj_type(it->target)) {
    case SETAE_T_LIST: {
        SetaeList *l = setae_to_ptr(it->target);
        if (it->index >= l->len) {
            return 0;
        }
        *out = l->items[it->index++];
        return 1;
    }
    case SETAE_T_TUPLE: {
        SetaeTuple *t = setae_to_ptr(it->target);
        if (it->index >= t->len) {
            return 0;
        }
        *out = t->items[it->index++];
        return 1;
    }
    case SETAE_T_DICT: {
        SetaeDict *d = setae_to_ptr(it->target);
        if (it->index >= d->len) {
            return 0;
        }
        *out = d->entries[it->index++].key;
        return 1;
    }
    case SETAE_T_STR: {
        size_t n = setae_str_len(it->target);
        if (it->index >= n) {
            return 0;
        }
        const char *p = setae_str_data(it->target);
        size_t charlen = 1;
        unsigned char c = (unsigned char)p[it->index];
        if (c >= 0xf0) {
            charlen = 4;
        } else if (c >= 0xe0) {
            charlen = 3;
        } else if (c >= 0xc0) {
            charlen = 2;
        }
        *out = setae_str_new(vm->heap, p + it->index, charlen);
        it->index += charlen;
        return 1;
    }
    default: {
        SetaeRange *r = setae_to_ptr(it->target);
        if ((int64_t)it->index >= setae_range_len(r)) {
            return 0;
        }
        *out = from_i64(r->start + (int64_t)it->index * r->step);
        it->index++;
        return 1;
    }
    }
}

static SetaeValue run_code(SetaeVM *vm, const SetaeCode *code, SetaeValue *args,
                           int nargs, const SetaeValue *captured,
                           const SetaeValue *defaults, uint32_t ndefaults,
                           SetaeValue module);

static SetaeValue call_value(SetaeVM *vm, SetaeValue callee, SetaeValue *args,
                             int nargs) {
    int t = setae_obj_type(callee);
    if (t == SETAE_T_BUILTIN) {
        SetaeBuiltin *b = setae_to_ptr(callee);
        return b->fn(vm, args, nargs);
    }
    if (t == SETAE_T_FUNCTION) {
        SetaeFunc *f = setae_to_ptr(callee);
        uint32_t nparams = setae_code_nparams(f->code);
        uint32_t required = nparams - f->ndefaults;
        if ((uint32_t)nargs < required || (uint32_t)nargs > nparams) {
            setae_vm_raise(vm, "TypeError", "%s() takes %u positional arguments but %d were given",
                           setae_code_fname(f->code), nparams, nargs);
            return setae_none();
        }
        return run_code(vm, f->code, args, nargs, f->cells, f->defaults, f->ndefaults,
                        f->module);
    }
    if (t == SETAE_T_EXCTYPE) {
        SetaeExcType *et = setae_to_ptr(callee);
        if (nargs > 1) {
            setae_vm_raise(vm, "TypeError", "%s() takes at most 1 argument (%d given)",
                           et->name, nargs);
            return setae_none();
        }
        return setae_exc_new(vm->heap, et->name, nargs == 1 ? args[0] : setae_none());
    }
    if (t == SETAE_T_CLASS) {
        SetaeClass *c = setae_to_ptr(callee);
        SetaeValue inst = setae_instance_new(vm->heap, callee);
        SetaeValue init;
        if (class_lookup(callee, "__init__", &init)) {
            if (setae_obj_type(init) != SETAE_T_FUNCTION) {
                setae_vm_raise(vm, "TypeError", "__init__ must be a function");
                return setae_none();
            }
            SetaeFunc *f = setae_to_ptr(init);
            uint32_t nparams = setae_code_nparams(f->code);
            uint32_t required = nparams - f->ndefaults;
            if ((uint32_t)nargs + 1 < required || (uint32_t)nargs + 1 > nparams) {
                setae_vm_raise(
                    vm, "TypeError",
                    "%.*s.__init__() takes %u positional arguments but %u were given",
                    (int)setae_str_len(c->name), setae_str_data(c->name), nparams,
                    (uint32_t)nargs + 1);
                return setae_none();
            }
            SetaeValue argv[256];
            argv[0] = inst;
            for (int i = 0; i < nargs; i++) {
                argv[i + 1] = args[i];
            }
            run_code(vm, f->code, argv, nargs + 1, f->cells, f->defaults, f->ndefaults,
                     f->module);
            if (vm->error) {
                return setae_none();
            }
        } else if (nargs != 0) {
            setae_vm_raise(vm, "TypeError", "%.*s() takes no arguments (%d given)",
                           (int)setae_str_len(c->name), setae_str_data(c->name), nargs);
            return setae_none();
        }
        return inst;
    }
    if (t == SETAE_T_BOUND) {
        SetaeBound *b = setae_to_ptr(callee);
        SetaeFunc *f = setae_to_ptr(b->func);
        uint32_t nparams = setae_code_nparams(f->code);
        uint32_t required = nparams - f->ndefaults;
        if ((uint32_t)nargs + 1 < required || (uint32_t)nargs + 1 > nparams) {
            setae_vm_raise(vm, "TypeError",
                           "%s() takes %u positional arguments but %u were given",
                           setae_code_fname(f->code), nparams, (uint32_t)nargs + 1);
            return setae_none();
        }
        SetaeValue argv[256];
        argv[0] = b->self;
        for (int i = 0; i < nargs; i++) {
            argv[i + 1] = args[i];
        }
        return run_code(vm, f->code, argv, nargs + 1, f->cells, f->defaults, f->ndefaults,
                        f->module);
    }
    setae_vm_raise(vm, "TypeError", "'%s' object is not callable", setae_type_name(callee));
    return setae_none();
}

static SetaeValue call_method(SetaeVM *vm, SetaeValue obj, const char *name,
                              SetaeValue *args, int nargs, SetaeInlineCache *c) {
    int t = setae_obj_type(obj);
    if (t == SETAE_T_INSTANCE) {
        SetaeInstance *inst = setae_to_ptr(obj);
        SetaeValue found;
        if (setae_instance_get(inst, name, &found)) {
            return call_value(vm, found, args, nargs);
        }
        SetaeValue v;
        if (class_lookup(inst->cls, name, &v)) {
            if (setae_obj_type(v) == SETAE_T_FUNCTION) {
                SetaeFunc *f = setae_to_ptr(v);
                uint32_t nparams = setae_code_nparams(f->code);
                uint32_t required = nparams - f->ndefaults;
                if ((uint32_t)nargs + 1 < required || (uint32_t)nargs + 1 > nparams) {
                    setae_vm_raise(
                        vm, "TypeError",
                        "%s() takes %u positional arguments but %u were given",
                        setae_code_fname(f->code), nparams, (uint32_t)nargs + 1);
                    return setae_none();
                }
                c->kind = 4;
                c->shape = inst->shape;
                c->cls = inst->cls;
                c->method = v;
                c->guard = vm->class_version;
                return run_code(vm, f->code, args - 1, nargs + 1, f->cells, f->defaults,
                                f->ndefaults, f->module);
            }
            return call_value(vm, v, args, nargs);
        }
        attr_error(vm, obj, name);
        return setae_none();
    }
    if (t == SETAE_T_CLASS) {
        SetaeValue v;
        if (class_lookup(obj, name, &v)) {
            return call_value(vm, v, args, nargs);
        }
        attr_error(vm, obj, name);
        return setae_none();
    }
    if (t == SETAE_T_MODULE) {
        SetaeModule *m = setae_to_ptr(obj);
        SetaeDict *d = setae_to_ptr(m->dict);
        int64_t i = dict_find_cstr(d, name);
        if (i >= 0) {
            return call_value(vm, d->entries[i].value, args, nargs);
        }
        attr_error(vm, obj, name);
        return setae_none();
    }
    if (t == SETAE_T_LIST) {
        SetaeList *l = setae_to_ptr(obj);
        if (strcmp(name, "append") == 0) {
            if (nargs != 1) {
                setae_vm_raise(vm, "TypeError", "append() takes exactly one argument (%d given)",
                               nargs);
                return setae_none();
            }
            setae_list_push(l, args[0]);
            return setae_none();
        }
        if (strcmp(name, "pop") == 0) {
            if (nargs > 1) {
                setae_vm_raise(vm, "TypeError", "pop() takes at most 1 argument (%d given)",
                               nargs);
                return setae_none();
            }
            if (l->len == 0) {
                setae_vm_raise(vm, "IndexError", "pop from empty list");
                return setae_none();
            }
            int64_t i = l->len - 1;
            if (nargs == 1) {
                if (!setae_is_int(args[0])) {
                    setae_vm_raise(vm, "TypeError", "'%s' object cannot be interpreted as an integer",
                                   setae_type_name(args[0]));
                    return setae_none();
                }
                i = setae_to_int(args[0]);
                if (i < 0) {
                    i += l->len;
                }
                if (i < 0 || i >= (int64_t)l->len) {
                    setae_vm_raise(vm, "IndexError", "pop index out of range");
                    return setae_none();
                }
            }
            SetaeValue v = l->items[i];
            memmove(&l->items[i], &l->items[i + 1],
                    (l->len - 1 - (size_t)i) * sizeof(SetaeValue));
            l->len--;
            return v;
        }
    } else if (t == SETAE_T_DICT) {
        SetaeDict *d = setae_to_ptr(obj);
        if (strcmp(name, "get") == 0) {
            if (nargs < 1 || nargs > 2) {
                setae_vm_raise(vm, "TypeError", "get expected 1 or 2 arguments, got %d", nargs);
                return setae_none();
            }
            if (!hashable(args[0])) {
                setae_vm_raise(vm, "TypeError", "unhashable type: '%s'",
                               setae_type_name(args[0]));
                return setae_none();
            }
            int64_t i = dict_find(d, args[0]);
            if (i >= 0) {
                return d->entries[i].value;
            }
            return nargs == 2 ? args[1] : setae_none();
        }
        if (strcmp(name, "items") == 0) {
            if (nargs != 0) {
                setae_vm_raise(vm, "TypeError", "items() takes no arguments (%d given)",
                               nargs);
                return setae_none();
            }
            SetaeValue rv = setae_list_new(vm->heap, d->len);
            setae_vm_push_tmp(vm, rv);
            SetaeList *r = setae_to_ptr(rv);
            for (uint32_t i = 0; i < d->len; i++) {
                SetaeValue kv[2] = {d->entries[i].key, d->entries[i].value};
                setae_list_push(r, setae_tuple_new(vm->heap, kv, 2));
            }
            setae_vm_pop_tmp(vm);
            return rv;
        }
        if (strcmp(name, "keys") == 0 || strcmp(name, "values") == 0) {
            if (nargs != 0) {
                setae_vm_raise(vm, "TypeError", "%s() takes no arguments (%d given)", name,
                               nargs);
                return setae_none();
            }
            int keys = name[0] == 'k';
            SetaeValue rv = setae_list_new(vm->heap, d->len);
            SetaeList *r = setae_to_ptr(rv);
            for (uint32_t i = 0; i < d->len; i++) {
                setae_list_push(r, keys ? d->entries[i].key : d->entries[i].value);
            }
            return rv;
        }
    }
    setae_vm_raise(vm, "AttributeError", "'%s' object has no attribute '%s'",
                   setae_type_name(obj), name);
    return setae_none();
}

static SetaeValue run_code(SetaeVM *vm, const SetaeCode *code, SetaeValue *args,
                           int nargs, const SetaeValue *captured,
                           const SetaeValue *defaults, uint32_t ndefaults,
                           SetaeValue module) {
    if (vm->depth >= MAX_DEPTH) {
        setae_vm_raise(vm, "RecursionError", "maximum recursion depth exceeded");
        return setae_none();
    }
    vm->depth++;

    const SetaeValue *consts = setae_code_consts(code);
    uint32_t ncode;
    const uint8_t *bytes = setae_code_bytes(code, &ncode);
    SetaeInlineCache *ic = setae_code_ic((SetaeCode *)code);
    uint32_t nlocals = setae_code_nlocals(code);
    uint32_t nparams = setae_code_nparams(code);
    uint32_t ncells = setae_code_ncells(code);
    uint32_t nfrees = setae_code_nfrees(code);
    uint32_t fixed = nlocals + ncells + nfrees;

    uint32_t frame_cap;
    SetaeValue *frame = frame_alloc(vm, fixed + STACK_MAX, &frame_cap);
    memset(frame, 0, fixed * sizeof(SetaeValue));
    SetaeValue *locals = frame;
    SetaeValue *cellbase = frame + nlocals;
    SetaeValue *stack = frame + fixed;
    for (int i = 0; i < nargs; i++) {
        locals[i] = args[i];
    }
    for (uint32_t i = (uint32_t)nargs; i < nparams; i++) {
        locals[i] = defaults[i - (nparams - ndefaults)];
    }
    int sp = 0;

    SetaeFrame fr = {frame, fixed, 0, module, vm->frames};
    vm->frames = &fr;

    for (uint32_t i = 0; i < ncells; i++) {
        cellbase[i] = setae_cell_new(vm->heap);
    }
    for (uint32_t i = 0; i < nfrees; i++) {
        cellbase[ncells + i] = captured[i];
    }

    int limited = vm->step_limit != 0 || vm->deadline_ns != 0;
    SetaeValue result = setae_none();
    uint32_t ip = 0;
    uint32_t ext = 0;
    while (ip < ncode && !vm->error) {
        fr.sp = sp;
        if (limited) {
            vm->steps++;
            if (vm->step_limit != 0 && vm->steps > vm->step_limit) {
                vm->interrupted = 1;
                vm->error = 1;
                snprintf(vm->errmsg, sizeof(vm->errmsg),
                         "RuntimeError: step limit exceeded");
                break;
            }
            if (vm->deadline_ns != 0 && (vm->steps & 0xfff) == 0 &&
                monotonic_ns() > vm->deadline_ns) {
                vm->interrupted = 1;
                vm->error = 1;
                snprintf(vm->errmsg, sizeof(vm->errmsg),
                         "RuntimeError: time limit exceeded");
                break;
            }
        }
        uint32_t unit = ip / 2;
        uint8_t op = bytes[ip];
        uint32_t arg = ext | bytes[ip + 1];
        ip += 2;
        if (op == OP_EXTENDED_ARG) {
            ext = arg << 8;
            continue;
        }
        ext = 0;
        if (sp >= STACK_MAX - 1) {
            setae_vm_raise(vm, "RuntimeError", "value stack overflow");
            break;
        }
        switch (op) {
        case OP_LOAD_CONST:
            stack[sp++] = consts[arg];
            break;
        case OP_LOAD_NAME: {
            SetaeInlineCache *c = &ic[unit];
            if (c->kind == 1) {
                stack[sp++] = vm->globals[c->slot].value;
                break;
            }
            SetaeDict *md =
                module != 0 ? setae_to_ptr(((SetaeModule *)setae_to_ptr(module))->dict) : NULL;
            if (c->kind == 2) {
                stack[sp++] = md->entries[c->slot].value;
                break;
            }
            uint32_t guard = md != NULL ? md->len : (uint32_t)vm->nglobals;
            if (c->kind == 3 && c->guard == guard) {
                stack[sp++] = vm->builtins[c->slot].value;
                break;
            }
            const char *name = setae_code_name(code, arg);
            if (md != NULL) {
                int64_t i = dict_find_cstr(md, name);
                if (i >= 0) {
                    c->kind = 2;
                    c->slot = (uint32_t)i;
                    stack[sp++] = md->entries[i].value;
                    break;
                }
            } else {
                int64_t i = tab_find(vm->globals, vm->nglobals, vm->globals_index,
                                     vm->globals_index_cap, name);
                if (i >= 0) {
                    c->kind = 1;
                    c->slot = (uint32_t)i;
                    stack[sp++] = vm->globals[i].value;
                    break;
                }
            }
            int64_t bi = tab_find(vm->builtins, vm->nbuiltins, vm->builtins_index,
                                  vm->builtins_index_cap, name);
            if (bi >= 0) {
                c->kind = 3;
                c->slot = (uint32_t)bi;
                c->guard = guard;
                stack[sp++] = vm->builtins[bi].value;
                break;
            }
            setae_vm_raise(vm, "NameError", "name '%s' is not defined", name);
            break;
        }
        case OP_STORE_NAME: {
            const char *name = setae_code_name(code, arg);
            SetaeValue val = stack[--sp];
            if (module != 0) {
                SetaeDict *d = setae_to_ptr(((SetaeModule *)setae_to_ptr(module))->dict);
                SetaeValue key = setae_str_new(vm->heap, name, strlen(name));
                dict_set(d, key, val);
            } else {
                setae_vm_set_global(vm, name, val);
            }
            break;
        }
        case OP_LOAD_LOCAL:
            if (locals[arg] == 0) {
                setae_vm_raise(
                    vm, "UnboundLocalError", "local variable referenced before assignment");
                break;
            }
            stack[sp++] = locals[arg];
            break;
        case OP_STORE_LOCAL:
            locals[arg] = stack[--sp];
            break;
        case OP_POP_TOP:
            sp--;
            break;
        case OP_BINARY_OP: {
            SetaeValue b = stack[--sp];
            SetaeValue a = stack[--sp];
            stack[sp++] = binary_op(vm, (SetaeBinOp)(arg & 0x7f), (int)(arg >> 7), a, b);
            break;
        }
        case OP_CALL: {
            int n = (int)arg;
            SetaeValue *argv = &stack[sp - n];
            SetaeValue callee = stack[sp - n - 1];
            SetaeValue r = call_value(vm, callee, argv, n);
            sp -= n + 1;
            stack[sp++] = r;
            break;
        }
        case OP_RETURN:
            result = stack[--sp];
            ip = ncode;
            break;
        case OP_JUMP:
            ip = arg * 2;
            break;
        case OP_POP_JUMP_IF_FALSE:
            if (!truthy(stack[--sp])) {
                ip = arg * 2;
            }
            break;
        case OP_POP_JUMP_IF_TRUE:
            if (truthy(stack[--sp])) {
                ip = arg * 2;
            }
            break;
        case OP_JUMP_IF_FALSE_OR_POP:
            if (!truthy(stack[sp - 1])) {
                ip = arg * 2;
            } else {
                sp--;
            }
            break;
        case OP_JUMP_IF_TRUE_OR_POP:
            if (truthy(stack[sp - 1])) {
                ip = arg * 2;
            } else {
                sp--;
            }
            break;
        case OP_COMPARE_OP: {
            SetaeValue b = stack[--sp];
            SetaeValue a = stack[--sp];
            stack[sp++] = compare(vm, (SetaeCmpOp)arg, a, b);
            break;
        }
        case OP_UNARY_NEG:
            stack[sp - 1] = unary_neg(vm, stack[sp - 1]);
            break;
        case OP_UNARY_NOT:
            stack[sp - 1] = setae_bool(!truthy(stack[sp - 1]));
            break;
        case OP_MAKE_FUNCTION: {
            const SetaeCode *child = setae_code_child(code, arg);
            if (child == NULL) {
                setae_vm_raise(vm, "RuntimeError", "bad code index %u", arg);
                break;
            }
            uint32_t nf = setae_code_nfrees(child);
            uint32_t nd = setae_code_ndefaults(child);
            SetaeValue f = setae_func_new(vm->heap, child, &stack[sp - nf], nf,
                                          &stack[sp - nf - nd], nd, module);
            sp -= (int)(nf + nd);
            stack[sp++] = f;
            break;
        }
        case OP_LOAD_CLOSURE:
            stack[sp++] = cellbase[arg];
            break;
        case OP_LOAD_DEREF: {
            SetaeCell *cell = setae_to_ptr(cellbase[arg]);
            if (cell->value == 0) {
                setae_vm_raise(
                    vm, "UnboundLocalError", "variable referenced before assignment");
                break;
            }
            stack[sp++] = cell->value;
            break;
        }
        case OP_STORE_DEREF: {
            SetaeCell *cell = setae_to_ptr(cellbase[arg]);
            cell->value = stack[--sp];
            break;
        }
        case OP_BUILD_TUPLE: {
            int n = (int)arg;
            SetaeValue tv = setae_tuple_new(vm->heap, &stack[sp - n], (uint32_t)n);
            sp -= n;
            stack[sp++] = tv;
            break;
        }
        case OP_LOAD_ATTR: {
            SetaeValue obj = stack[sp - 1];
            if (setae_obj_type(obj) == SETAE_T_INSTANCE) {
                SetaeInstance *inst = setae_to_ptr(obj);
                if (ic[unit].shape == inst->shape) {
                    stack[sp - 1] = inst->slots[ic[unit].slot];
                    break;
                }
                int64_t slot = setae_instance_slot(inst, setae_code_name(code, arg));
                if (slot >= 0) {
                    ic[unit].shape = inst->shape;
                    ic[unit].slot = (uint32_t)slot;
                    stack[sp - 1] = inst->slots[slot];
                    break;
                }
            }
            stack[sp - 1] = load_attr(vm, obj, setae_code_name(code, arg));
            break;
        }
        case OP_STORE_ATTR: {
            SetaeValue obj = stack[sp - 1];
            SetaeValue val = stack[sp - 2];
            int t = setae_obj_type(obj);
            if (t == SETAE_T_INSTANCE) {
                SetaeInstance *inst = setae_to_ptr(obj);
                if (ic[unit].shape == inst->shape) {
                    SetaeShape *ns = ic[unit].next;
                    if (ns->nslots > inst->slots_cap) {
                        uint32_t cap = inst->slots_cap ? inst->slots_cap * 2 : 4;
                        if (cap < ns->nslots) {
                            cap = ns->nslots;
                        }
                        inst->slots = realloc(inst->slots, cap * sizeof(SetaeValue));
                        inst->slots_cap = cap;
                    }
                    inst->slots[ic[unit].slot] = val;
                    inst->shape = ns;
                    sp -= 2;
                    break;
                }
                const char *name = setae_code_name(code, arg);
                SetaeShape *from = inst->shape;
                setae_instance_set(vm->heap, inst, name, val);
                ic[unit].shape = from;
                ic[unit].next = inst->shape;
                ic[unit].slot = (uint32_t)setae_instance_slot(inst, name);
                sp -= 2;
                break;
            }
            const char *name = setae_code_name(code, arg);
            if (t == SETAE_T_CLASS) {
                SetaeValue dv = ((SetaeClass *)setae_to_ptr(obj))->dict;
                SetaeValue key = setae_str_new(vm->heap, name, strlen(name));
                dict_set(setae_to_ptr(dv), key, val);
                vm->class_version++;
                sp -= 2;
            } else {
                setae_vm_raise(vm, "AttributeError", "'%s' object has no attribute '%s'",
                               setae_type_name(obj), name);
            }
            break;
        }
        case OP_MAKE_CLASS: {
            SetaeValue cname = stack[--sp];
            SetaeValue base = stack[--sp];
            SetaeValue dict = stack[--sp];
            vm->class_version++;
            if (!setae_is_none(base) && setae_obj_type(base) == SETAE_T_EXCTYPE) {
                char name[128];
                size_t len = setae_str_len(cname);
                if (len >= sizeof(name)) {
                    len = sizeof(name) - 1;
                }
                memcpy(name, setae_str_data(cname), len);
                name[len] = '\0';
                stack[sp++] = setae_exctype_new(vm->heap, name);
                break;
            }
            if (!setae_is_none(base) && setae_obj_type(base) != SETAE_T_CLASS) {
                setae_vm_raise(vm, "TypeError", "base must be a class, not '%s'",
                               setae_type_name(base));
                break;
            }
            stack[sp++] = setae_class_new(vm->heap, cname, base, dict);
            break;
        }
        case OP_IMPORT: {
            if (arg >= vm->nmodules) {
                setae_vm_raise(vm, "RuntimeError", "bad module index %u", arg);
                break;
            }
            if (vm->module_cache[arg] != 0) {
                stack[sp++] = vm->module_cache[arg];
                break;
            }
            const SetaeCode *mcode = setae_code_module(vm->root, arg);
            SetaeValue mname =
                setae_str_new(vm->heap, setae_code_fname(mcode),
                              strlen(setae_code_fname(mcode)));
            setae_vm_push_tmp(vm, mname);
            SetaeValue mdict = setae_dict_new(vm->heap);
            setae_vm_push_tmp(vm, mdict);
            SetaeValue mod = setae_module_new(vm->heap, mname, mdict);
            setae_vm_pop_tmp(vm);
            setae_vm_pop_tmp(vm);
            vm->module_cache[arg] = mod;
            int32_t parent = setae_code_module_parent(mcode);
            if (parent >= 0 && (uint32_t)parent < vm->nmodules &&
                vm->module_cache[parent] != 0) {
                const char *qual = setae_code_fname(mcode);
                const char *leaf = strrchr(qual, '.');
                leaf = leaf ? leaf + 1 : qual;
                SetaeModule *pm = setae_to_ptr(vm->module_cache[parent]);
                SetaeValue key = setae_str_new(vm->heap, leaf, strlen(leaf));
                dict_set(setae_to_ptr(pm->dict), key, mod);
            }
            run_code(vm, mcode, NULL, 0, NULL, NULL, 0, mod);
            if (vm->error) {
                break;
            }
            stack[sp++] = mod;
            break;
        }
        case OP_IMPORT_MISSING:
            setae_vm_raise(vm, "ImportError", "No module named '%s'",
                           setae_code_name(code, arg));
            break;
        case OP_RAISE:
            raise_value(vm, stack[--sp]);
            break;
        case OP_RERAISE:
            raise_pending(vm, stack[--sp]);
            break;
        case OP_EXC_MATCH: {
            SetaeValue type = stack[--sp];
            int r = exc_matches(vm, stack[sp - 1], type);
            if (vm->error) {
                break;
            }
            stack[sp++] = setae_bool(r);
            break;
        }
        case OP_UNPACK_SEQUENCE: {
            SetaeValue seq = stack[--sp];
            const SetaeValue *items;
            uint32_t len;
            int t = setae_obj_type(seq);
            if (t == SETAE_T_TUPLE) {
                SetaeTuple *tp = setae_to_ptr(seq);
                items = tp->items;
                len = tp->len;
            } else if (t == SETAE_T_LIST) {
                SetaeList *l = setae_to_ptr(seq);
                items = l->items;
                len = l->len;
            } else {
                setae_vm_raise(vm, "TypeError", "cannot unpack non-iterable %s object",
                               setae_type_name(seq));
                break;
            }
            if (len < arg) {
                setae_vm_raise(
                    vm, "ValueError", "not enough values to unpack (expected %u, got %u)",
                    arg, len);
                break;
            }
            if (len > arg) {
                setae_vm_raise(vm, "ValueError", "too many values to unpack (expected %u)",
                               arg);
                break;
            }
            for (uint32_t i = arg; i-- > 0;) {
                stack[sp++] = items[i];
            }
            break;
        }
        case OP_BUILD_LIST: {
            int n = (int)arg;
            SetaeValue lv = setae_list_new(vm->heap, (uint32_t)n);
            SetaeList *l = setae_to_ptr(lv);
            for (int i = 0; i < n; i++) {
                setae_list_push(l, stack[sp - n + i]);
            }
            sp -= n;
            stack[sp++] = lv;
            break;
        }
        case OP_BUILD_DICT: {
            int n = (int)arg;
            SetaeValue dv = setae_dict_new(vm->heap);
            SetaeDict *d = setae_to_ptr(dv);
            for (int i = 0; i < n; i++) {
                SetaeValue key = stack[sp - 2 * n + 2 * i];
                SetaeValue val = stack[sp - 2 * n + 2 * i + 1];
                if (!hashable(key)) {
                    setae_vm_raise(vm, "TypeError", "unhashable type: '%s'",
                                   setae_type_name(key));
                    break;
                }
                dict_set(d, key, val);
            }
            sp -= 2 * n;
            stack[sp++] = dv;
            break;
        }
        case OP_SUBSCR: {
            SetaeValue idx = stack[--sp];
            SetaeValue obj = stack[--sp];
            stack[sp++] = subscript(vm, obj, idx);
            break;
        }
        case OP_STORE_SUBSCR: {
            SetaeValue idx = stack[--sp];
            SetaeValue obj = stack[--sp];
            SetaeValue val = stack[--sp];
            store_subscript(vm, obj, idx, val);
            break;
        }
        case OP_GET_ITER: {
            SetaeValue v = stack[sp - 1];
            int t = setae_obj_type(v);
            if (t != SETAE_T_LIST && t != SETAE_T_TUPLE && t != SETAE_T_DICT &&
                t != SETAE_T_STR && t != SETAE_T_RANGE) {
                setae_vm_raise(vm, "TypeError", "'%s' object is not iterable",
                               setae_type_name(v));
                break;
            }
            stack[sp - 1] = setae_iter_new(vm->heap, v);
            break;
        }
        case OP_FOR_ITER: {
            SetaeIter *it = setae_to_ptr(stack[sp - 1]);
            SetaeValue next;
            if (iter_next(vm, it, &next)) {
                stack[sp++] = next;
            } else {
                sp--;
                ip = arg * 2;
            }
            break;
        }
        case OP_CALL_METHOD: {
            int n = (int)(arg & 0xff);
            SetaeValue *argv = &stack[sp - n];
            SetaeValue obj = stack[sp - n - 1];
            SetaeInlineCache *c = &ic[unit];
            if (c->kind == 4 && setae_obj_type(obj) == SETAE_T_INSTANCE) {
                SetaeInstance *inst = setae_to_ptr(obj);
                if (c->shape == inst->shape && c->cls == inst->cls &&
                    c->guard == vm->class_version) {
                    SetaeFunc *f = setae_to_ptr(c->method);
                    uint32_t nparams = setae_code_nparams(f->code);
                    uint32_t required = nparams - f->ndefaults;
                    SetaeValue r;
                    if ((uint32_t)n + 1 < required || (uint32_t)n + 1 > nparams) {
                        setae_vm_raise(
                            vm, "TypeError",
                            "%s() takes %u positional arguments but %u were given",
                            setae_code_fname(f->code), nparams, (uint32_t)n + 1);
                        r = setae_none();
                    } else {
                        r = run_code(vm, f->code, argv - 1, n + 1, f->cells, f->defaults,
                                     f->ndefaults, f->module);
                    }
                    sp -= n + 1;
                    stack[sp++] = r;
                    break;
                }
            }
            const char *name = setae_code_name(code, arg >> 8);
            SetaeValue r = call_method(vm, obj, name, argv, n, c);
            sp -= n + 1;
            stack[sp++] = r;
            break;
        }
        default:
            setae_vm_raise(vm, "RuntimeError", "bad opcode %u", op);
            break;
        }
        if (vm->error && !vm->interrupted) {
            uint32_t nexc;
            const SetaeExcEntry *entries = setae_code_excs(code, &nexc);
            for (uint32_t i = 0; i < nexc; i++) {
                if (unit >= entries[i].start && unit < entries[i].end) {
                    sp = (int)entries[i].depth;
                    fr.sp = sp;
                    SetaeValue exc = vm->exc;
                    vm->error = 0;
                    vm->errmsg[0] = '\0';
                    vm->exc = 0;
                    if (exc == 0) {
                        exc = setae_exc_new(vm->heap, "RuntimeError", setae_none());
                    }
                    stack[sp++] = exc;
                    ip = entries[i].target * 2;
                    break;
                }
            }
        }
    }

    vm->frames = fr.parent;
    frame_release(vm, frame, frame_cap);
    vm->depth--;
    return result;
}

SetaeValue setae_vm_run(SetaeVM *vm, SetaeCode *code) {
    attach_code(vm, code);
    vm->root = code;
    uint32_t nm = setae_code_nmodules(code);
    if (nm > vm->nmodules) {
        vm->module_cache = realloc(vm->module_cache, nm * sizeof(SetaeValue));
    }
    for (uint32_t i = 0; i < nm; i++) {
        vm->module_cache[i] = 0;
    }
    vm->nmodules = nm;
    vm->steps = 0;
    vm->interrupted = 0;
    vm->deadline_ns = vm->time_limit_ns != 0 ? monotonic_ns() + vm->time_limit_ns : 0;
    if (vm->oom == 0) {
        vm->oom = setae_exc_new(vm->heap, "MemoryError", setae_none());
    }
    return run_code(vm, code, NULL, 0, NULL, NULL, 0, 0);
}
