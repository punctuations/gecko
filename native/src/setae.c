#define _POSIX_C_SOURCE 200809L

#include "internal.h"

#include <math.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#if !defined(__GNUC__)
#error "the Setae dispatch loop requires computed goto (GCC or Clang)"
#endif

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
    free(vm->tmp_roots);
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

int setae_vm_del_global(SetaeVM *vm, const char *name) {
    int64_t i = tab_find(vm->globals, vm->nglobals, vm->globals_index, vm->globals_index_cap,
                         name);
    if (i < 0) {
        return 0;
    }
    free(vm->globals[i].name);
    for (size_t j = (size_t)i; j + 1 < vm->nglobals; j++) {
        vm->globals[j] = vm->globals[j + 1];
    }
    vm->nglobals--;
    free(vm->globals_index);
    vm->globals_index = NULL;
    vm->globals_index_cap = 0;
    for (size_t j = 0; j < vm->nglobals; j++) {
        tab_index_add(vm->globals, j + 1, &vm->globals_index, &vm->globals_index_cap,
                      (uint32_t)j);
    }
    return 1;
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

void setae_vm_clear_error(SetaeVM *vm) {
    vm->error = 0;
    vm->errmsg[0] = '\0';
    vm->exc = setae_none();
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
    if (vm->ntmp == vm->tmp_cap) {
        vm->tmp_cap = vm->tmp_cap ? vm->tmp_cap * 2 : 16;
        vm->tmp_roots = realloc(vm->tmp_roots, (size_t)vm->tmp_cap * sizeof(SetaeValue));
    }
    vm->tmp_roots[vm->ntmp++] = v;
}

void setae_vm_pop_tmp(SetaeVM *vm) {
    vm->ntmp--;
}

static double as_number(SetaeValue v) {
    if (setae_obj_type(v) == SETAE_T_BIGINT) {
        return setae_int_to_double(v);
    }
    if (setae_is_bool(v)) {
        return setae_to_bool(v) ? 1.0 : 0.0;
    }
    return setae_is_int(v) ? (double)setae_to_int(v) : setae_to_float(v);
}

static SetaeValue from_i64(SetaeVM *vm, int64_t i) {
    if (i >= INT32_MIN && i <= INT32_MAX) {
        return setae_from_int((int32_t)i);
    }
    return setae_int_from_i64(vm->heap, i);
}

static int hashable(SetaeValue v) {
    int t = setae_obj_type(v);
    if (t == SETAE_T_LIST || t == SETAE_T_DICT) {
        return 0;
    }
    if (t == SETAE_T_SET) {
        return ((SetaeSet *)setae_to_ptr(v))->frozen;
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

void setae_dict_set(SetaeDict *d, SetaeValue key, SetaeValue value) {
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

static int attr_name_cstr(SetaeVM *vm, SetaeValue v, char *buf, size_t cap) {
    if (!setae_is_str(v)) {
        setae_vm_raise(vm, "TypeError", "attribute name must be string, not '%s'",
                       setae_type_name(v));
        return 0;
    }
    size_t n = setae_str_len(v);
    if (n >= cap) {
        setae_vm_raise(vm, "AttributeError", "attribute name too long");
        return 0;
    }
    memcpy(buf, setae_str_data(v), n);
    buf[n] = '\0';
    return 1;
}

SetaeValue setae_builtin_getattr(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs < 2 || nargs > 3) {
        setae_vm_raise(vm, "TypeError", "getattr expected 2 or 3 arguments, got %d", nargs);
        return setae_none();
    }
    char buf[256];
    if (!attr_name_cstr(vm, args[1], buf, sizeof(buf))) {
        return setae_none();
    }
    SetaeValue r = load_attr(vm, args[0], buf);
    if (vm->error && nargs == 3) {
        vm->error = 0;
        vm->errmsg[0] = '\0';
        vm->exc = setae_none();
        return args[2];
    }
    return r;
}

SetaeValue setae_builtin_hasattr(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 2) {
        setae_vm_raise(vm, "TypeError", "hasattr expected 2 arguments, got %d", nargs);
        return setae_none();
    }
    char buf[256];
    if (!attr_name_cstr(vm, args[1], buf, sizeof(buf))) {
        return setae_none();
    }
    load_attr(vm, args[0], buf);
    if (vm->error) {
        vm->error = 0;
        vm->errmsg[0] = '\0';
        vm->exc = setae_none();
        return setae_bool(0);
    }
    return setae_bool(1);
}

SetaeValue setae_builtin_setattr(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 3) {
        setae_vm_raise(vm, "TypeError", "setattr expected 3 arguments, got %d", nargs);
        return setae_none();
    }
    char buf[256];
    if (!attr_name_cstr(vm, args[1], buf, sizeof(buf))) {
        return setae_none();
    }
    int t = setae_obj_type(args[0]);
    if (t == SETAE_T_INSTANCE) {
        setae_instance_set(vm->heap, setae_to_ptr(args[0]), buf, args[2]);
        return setae_none();
    }
    if (t == SETAE_T_CLASS) {
        SetaeValue dv = ((SetaeClass *)setae_to_ptr(args[0]))->dict;
        SetaeValue key = setae_str_new(vm->heap, buf, strlen(buf));
        setae_dict_set(setae_to_ptr(dv), key, args[2]);
        return setae_none();
    }
    setae_vm_raise(vm, "AttributeError", "'%s' object has no attribute '%s'",
                   setae_type_name(args[0]), buf);
    return setae_none();
}

static const char *bin_symbol(SetaeBinOp op, int aug) {
    static const char *const plain[] = {"+", "-",  "*",  "/",  "%",  "//",
                                        "**", "&", "|", "^", "<<", ">>"};
    static const char *const augmented[] = {"+=", "-=",  "*=", "/=", "%=",  "//=",
                                            "**=", "&=", "|=", "^=", "<<=", ">>="};
    return aug ? augmented[op] : plain[op];
}

static SetaeValue set_binop(SetaeVM *vm, SetaeBinOp op, SetaeSet *a, SetaeSet *b) {
    uint8_t frozen = a->frozen;
    SetaeValue rv = setae_set_new(vm->heap);
    setae_vm_push_tmp(vm, rv);
    SetaeSet *r = setae_to_ptr(rv);
    if (op == BIN_BITOR) {
        setae_set_merge(r, a);
        setae_set_merge(r, b);
    } else if (op == BIN_BITAND) {
        SetaeSet *small = a->used <= b->used ? a : b;
        SetaeSet *big = a->used <= b->used ? b : a;
        for (uint32_t i = 0; i <= small->mask; i++) {
            if (small->table[i].state == SET_ACTIVE &&
                setae_set_contains(big, small->table[i].key)) {
                setae_set_add(r, small->table[i].key);
            }
        }
    } else if (op == BIN_SUB) {
        if ((a->used >> 2) > b->used) {
            setae_set_merge(r, a);
            for (uint32_t i = 0; i <= b->mask; i++) {
                if (b->table[i].state == SET_ACTIVE) {
                    setae_set_discard(r, b->table[i].key);
                }
            }
        } else {
            for (uint32_t i = 0; i <= a->mask; i++) {
                if (a->table[i].state == SET_ACTIVE &&
                    !setae_set_contains(b, a->table[i].key)) {
                    setae_set_add(r, a->table[i].key);
                }
            }
        }
    } else {
        setae_set_merge(r, b);
        for (uint32_t i = 0; i <= a->mask; i++) {
            if (a->table[i].state == SET_ACTIVE) {
                if (!setae_set_discard(r, a->table[i].key)) {
                    setae_set_add(r, a->table[i].key);
                }
            }
        }
    }
    r->frozen = frozen;
    setae_vm_pop_tmp(vm);
    return rv;
}

static SetaeValue coerce_to_set(SetaeVM *vm, SetaeValue v) {
    if (setae_obj_type(v) == SETAE_T_SET) {
        return v;
    }
    SetaeValue sv = setae_set_new(vm->heap);
    setae_vm_push_tmp(vm, sv);
    SetaeValue it = setae_make_iter(vm, v);
    if (vm->error) {
        setae_vm_pop_tmp(vm);
        return setae_none();
    }
    setae_vm_push_tmp(vm, it);
    SetaeValue x;
    while (setae_iter_advance(vm, it, &x)) {
        if (vm->error) {
            break;
        }
        setae_set_add(setae_to_ptr(sv), x);
    }
    setae_vm_pop_tmp(vm);
    setae_vm_pop_tmp(vm);
    return vm->error ? setae_none() : sv;
}

static SetaeValue binary_op(SetaeVM *vm, SetaeBinOp op, int aug, SetaeValue a,
                            SetaeValue b) {
    if (setae_is_int(a) && setae_is_int(b)) {
        int64_t x = setae_to_int(a);
        int64_t y = setae_to_int(b);
        int64_t r;
        switch (op) {
        case BIN_ADD:
            return from_i64(vm, x + y);
        case BIN_SUB:
            return from_i64(vm, x - y);
        case BIN_MUL:
            return from_i64(vm, x * y);
        case BIN_MOD:
        case BIN_FLOORDIV:
            if (y != 0) {
                if (op == BIN_MOD) {
                    r = x % y;
                    if (r != 0 && (r < 0) != (y < 0)) {
                        r += y;
                    }
                    return from_i64(vm, r);
                }
                r = x / y;
                if (x % y != 0 && (x < 0) != (y < 0)) {
                    r--;
                }
                return from_i64(vm, r);
            }
            break;
        case BIN_BITAND:
            return from_i64(vm, x & y);
        case BIN_BITOR:
            return from_i64(vm, x | y);
        case BIN_BITXOR:
            return from_i64(vm, x ^ y);
        case BIN_RSHIFT:
            if (y >= 0) {
                return from_i64(vm, y >= 63 ? (x < 0 ? -1 : 0) : (x >> y));
            }
            break;
        default:
            break;
        }
    }
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
    if ((op == BIN_BITOR || op == BIN_BITAND || op == BIN_SUB || op == BIN_BITXOR) &&
        setae_obj_type(a) == SETAE_T_SET && setae_obj_type(b) == SETAE_T_SET) {
        return set_binop(vm, op, setae_to_ptr(a), setae_to_ptr(b));
    }
    int a_int = setae_is_integer(a);
    int b_int = setae_is_integer(b);
    int is_bitwise = op == BIN_BITAND || op == BIN_BITOR || op == BIN_BITXOR ||
                     op == BIN_LSHIFT || op == BIN_RSHIFT;
    int numeric = (a_int || setae_is_float(a)) && (b_int || setae_is_float(b));
    if (!numeric || (is_bitwise && !(a_int && b_int))) {
        setae_vm_raise(vm, "TypeError", "unsupported operand type(s) for %s: '%s' and '%s'",
                       bin_symbol(op, aug), setae_type_name(a), setae_type_name(b));
        return setae_none();
    }
    if (a_int && b_int && op != BIN_DIV) {
        SetaeHeap *h = vm->heap;
        int a_big = setae_obj_type(a) == SETAE_T_BIGINT;
        int b_big = setae_obj_type(b) == SETAE_T_BIGINT;
        if (!a_big && !b_big) {
            int64_t x = setae_is_bool(a) ? (setae_to_bool(a) ? 1 : 0) : setae_to_int(a);
            int64_t y = setae_is_bool(b) ? (setae_to_bool(b) ? 1 : 0) : setae_to_int(b);
            int64_t r;
            switch (op) {
            case BIN_ADD:
                if (!__builtin_add_overflow(x, y, &r)) {
                    return from_i64(vm, r);
                }
                break;
            case BIN_SUB:
                if (!__builtin_sub_overflow(x, y, &r)) {
                    return from_i64(vm, r);
                }
                break;
            case BIN_MUL:
                if (!__builtin_mul_overflow(x, y, &r)) {
                    return from_i64(vm, r);
                }
                break;
            case BIN_MOD:
            case BIN_FLOORDIV: {
                if (y == 0) {
                    setae_vm_raise(vm, "ZeroDivisionError",
                                   "integer division or modulo by zero");
                    return setae_none();
                }
                if (op == BIN_MOD) {
                    int64_t m = x % y;
                    if (m != 0 && (m < 0) != (y < 0)) {
                        m += y;
                    }
                    return from_i64(vm, m);
                }
                int64_t q = x / y;
                if (x % y != 0 && (x < 0) != (y < 0)) {
                    q--;
                }
                return from_i64(vm, q);
            }
            case BIN_BITAND:
                return setae_is_bool(a) && setae_is_bool(b) ? setae_bool(x & y)
                                                            : from_i64(vm, x & y);
            case BIN_BITOR:
                return setae_is_bool(a) && setae_is_bool(b) ? setae_bool(x | y)
                                                            : from_i64(vm, x | y);
            case BIN_BITXOR:
                return setae_is_bool(a) && setae_is_bool(b) ? setae_bool(x ^ y)
                                                            : from_i64(vm, x ^ y);
            case BIN_RSHIFT:
                if (y < 0) {
                    setae_vm_raise(vm, "ValueError", "negative shift count");
                    return setae_none();
                }
                return from_i64(vm, y >= 63 ? (x < 0 ? -1 : 0) : (x >> y));
            case BIN_LSHIFT:
            case BIN_POW:
                break;
            default:
                break;
            }
        }
        switch (op) {
        case BIN_ADD:
            return setae_int_add(h, a, b);
        case BIN_SUB:
            return setae_int_sub(h, a, b);
        case BIN_MUL:
            return setae_int_mul(h, a, b);
        case BIN_MOD:
        case BIN_FLOORDIV: {
            if (setae_int_sign(b) == 0) {
                setae_vm_raise(vm, "ZeroDivisionError", "integer division or modulo by zero");
                return setae_none();
            }
            SetaeValue q, r;
            setae_int_divmod(h, a, b, &q, &r);
            return op == BIN_MOD ? r : q;
        }
        case BIN_POW: {
            int64_t e;
            if (setae_int_sign(b) < 0) {
                return setae_from_float(pow(as_number(a), as_number(b)));
            }
            if (!setae_int_fits_i64(b, &e)) {
                setae_vm_raise(vm, "OverflowError", "exponent too large");
                return setae_none();
            }
            return setae_int_pow(h, a, e);
        }
        case BIN_LSHIFT: {
            int64_t e;
            if (setae_int_sign(b) < 0) {
                setae_vm_raise(vm, "ValueError", "negative shift count");
                return setae_none();
            }
            if (!setae_int_fits_i64(b, &e)) {
                setae_vm_raise(vm, "OverflowError", "shift count too large");
                return setae_none();
            }
            return setae_int_lshift(h, a, e);
        }
        case BIN_RSHIFT: {
            int64_t e;
            if (setae_int_sign(b) < 0) {
                setae_vm_raise(vm, "ValueError", "negative shift count");
                return setae_none();
            }
            if (!setae_int_fits_i64(b, &e)) {
                return setae_int_sign(a) < 0 ? setae_from_int(-1) : setae_from_int(0);
            }
            return setae_int_rshift(h, a, e);
        }
        case BIN_BITAND:
        case BIN_BITOR:
        case BIN_BITXOR: {
            int64_t x, y;
            if (setae_int_fits_i64(a, &x) && setae_int_fits_i64(b, &y)) {
                return setae_int_from_i64(
                    h, op == BIN_BITAND ? (x & y) : op == BIN_BITOR ? (x | y) : (x ^ y));
            }
            setae_vm_raise(vm, "OverflowError",
                           "bitwise operation on integers too large");
            return setae_none();
        }
        default:
            break;
        }
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
    case BIN_POW:
        return setae_from_float(pow(x, y));
    case BIN_FLOORDIV:
        if (y == 0.0) {
            setae_vm_raise(vm, "ZeroDivisionError", "float floor division by zero");
            return setae_none();
        }
        return setae_from_float(floor(x / y));
    default:
        setae_vm_raise(vm, "TypeError", "unsupported operand type(s) for %s: '%s' and '%s'",
                       bin_symbol(op, aug), setae_type_name(a), setae_type_name(b));
        return setae_none();
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
    case SETAE_T_SET:
        return ((SetaeSet *)setae_to_ptr(v))->used != 0;
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
    case SETAE_T_SET:
        return setae_set_contains(setae_to_ptr(container), x);
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

static int set_is_subset(SetaeSet *sa, SetaeSet *sb) {
    if (sa->used > sb->used) {
        return 0;
    }
    for (uint32_t i = 0; i <= sa->mask; i++) {
        if (sa->table[i].state == SET_ACTIVE &&
            !setae_set_contains(sb, sa->table[i].key)) {
            return 0;
        }
    }
    return 1;
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
    if (setae_obj_type(a) == SETAE_T_SET && setae_obj_type(b) == SETAE_T_SET) {
        SetaeSet *sa = setae_to_ptr(a);
        SetaeSet *sb = setae_to_ptr(b);
        int r;
        switch (op) {
        case CMP_EQ:
            r = sa->used == sb->used && set_is_subset(sa, sb);
            break;
        case CMP_NE:
            r = !(sa->used == sb->used && set_is_subset(sa, sb));
            break;
        case CMP_LE:
            r = set_is_subset(sa, sb);
            break;
        case CMP_GE:
            r = set_is_subset(sb, sa);
            break;
        case CMP_LT:
            r = sa->used < sb->used && set_is_subset(sa, sb);
            break;
        default:
            r = sa->used > sb->used && set_is_subset(sb, sa);
            break;
        }
        return setae_bool(r);
    }
    if (op == CMP_EQ || op == CMP_NE) {
        int eq = setae_value_eq(a, b);
        return setae_bool(op == CMP_EQ ? eq : !eq);
    }
    if (setae_is_int(a) && setae_is_int(b)) {
        int32_t x = setae_to_int(a);
        int32_t y = setae_to_int(b);
        int fc = x < y ? -1 : x > y ? 1 : 0;
        int fr = op == CMP_LT   ? fc < 0
                 : op == CMP_LE ? fc <= 0
                 : op == CMP_GT ? fc > 0
                                : fc >= 0;
        return setae_bool(fr);
    }
    int a_int = setae_is_integer(a);
    int b_int = setae_is_integer(b);
    int an = a_int || setae_is_float(a);
    int bn = b_int || setae_is_float(b);
    int at = setae_obj_type(a);
    int bt = setae_obj_type(b);
    int c;
    if (a_int && b_int) {
        c = setae_int_cmp(a, b);
    } else if (an && bn) {
        double x = as_number(a);
        double y = as_number(b);
        c = x < y ? -1 : x > y ? 1 : 0;
    } else if (setae_is_str(a) && setae_is_str(b)) {
        c = str_order(a, b);
    } else if ((at == SETAE_T_TUPLE && bt == SETAE_T_TUPLE) ||
               (at == SETAE_T_LIST && bt == SETAE_T_LIST)) {
        SetaeValue *ai, *bi;
        uint32_t al, bl;
        if (at == SETAE_T_TUPLE) {
            ai = ((SetaeTuple *)setae_to_ptr(a))->items;
            al = ((SetaeTuple *)setae_to_ptr(a))->len;
            bi = ((SetaeTuple *)setae_to_ptr(b))->items;
            bl = ((SetaeTuple *)setae_to_ptr(b))->len;
        } else {
            ai = ((SetaeList *)setae_to_ptr(a))->items;
            al = ((SetaeList *)setae_to_ptr(a))->len;
            bi = ((SetaeList *)setae_to_ptr(b))->items;
            bl = ((SetaeList *)setae_to_ptr(b))->len;
        }
        uint32_t n = al < bl ? al : bl;
        c = 0;
        for (uint32_t i = 0; i < n; i++) {
            if (setae_value_eq(ai[i], bi[i])) {
                continue;
            }
            int lt = setae_value_lt(vm, ai[i], bi[i]);
            if (vm->error) {
                return setae_none();
            }
            c = lt ? -1 : 1;
            break;
        }
        if (c == 0) {
            c = al < bl ? -1 : al > bl ? 1 : 0;
        }
    } else {
        setae_vm_raise(vm, "TypeError", "'%s' not supported between instances of '%s' and '%s'",
                       op == CMP_LT   ? "<"
                       : op == CMP_LE ? "<="
                       : op == CMP_GT ? ">"
                                      : ">=",
                       setae_type_name(a), setae_type_name(b));
        return setae_none();
    }
    int r = op == CMP_LT ? c < 0 : op == CMP_LE ? c <= 0 : op == CMP_GT ? c > 0 : c >= 0;
    return setae_bool(r);
}

static SetaeValue unary_neg(SetaeVM *vm, SetaeValue a) {
    if (setae_is_int(a)) {
        return from_i64(vm, -(int64_t)setae_to_int(a));
    }
    if (setae_obj_type(a) == SETAE_T_BIGINT) {
        return setae_int_neg(vm->heap, a);
    }
    if (setae_is_bool(a)) {
        return setae_from_int(setae_to_bool(a) ? -1 : 0);
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

static int slice_get(SetaeVM *vm, SetaeSlice *sl, int64_t len, int64_t *ostart,
                     int64_t *ostep, int64_t *ocount) {
    int64_t step = 1;
    if (!setae_is_none(sl->step)) {
        if (!setae_is_int(sl->step)) {
            setae_vm_raise(vm, "TypeError", "slice indices must be integers or None");
            return 0;
        }
        step = setae_to_int(sl->step);
        if (step == 0) {
            setae_vm_raise(vm, "ValueError", "slice step cannot be zero");
            return 0;
        }
    }
    int64_t lower = step < 0 ? -1 : 0;
    int64_t upper = step < 0 ? len - 1 : len;
    int64_t start, stop;
    if (setae_is_none(sl->lower)) {
        start = step < 0 ? upper : lower;
    } else if (!setae_is_int(sl->lower)) {
        setae_vm_raise(vm, "TypeError", "slice indices must be integers or None");
        return 0;
    } else {
        start = setae_to_int(sl->lower);
        if (start < 0) {
            start += len;
            if (start < lower) {
                start = lower;
            }
        } else if (start > upper) {
            start = upper;
        }
    }
    if (setae_is_none(sl->upper)) {
        stop = step < 0 ? lower : upper;
    } else if (!setae_is_int(sl->upper)) {
        setae_vm_raise(vm, "TypeError", "slice indices must be integers or None");
        return 0;
    } else {
        stop = setae_to_int(sl->upper);
        if (stop < 0) {
            stop += len;
            if (stop < lower) {
                stop = lower;
            }
        } else if (stop > upper) {
            stop = upper;
        }
    }
    int64_t count;
    if (step > 0) {
        count = start < stop ? (stop - start + step - 1) / step : 0;
    } else {
        count = start > stop ? (start - stop - step - 1) / -step : 0;
    }
    *ostart = start;
    *ostep = step;
    *ocount = count;
    return 1;
}

static SetaeValue slice_str(SetaeVM *vm, SetaeValue s, int64_t start, int64_t step,
                            int64_t count) {
    const char *p = setae_str_data(s);
    size_t n = setae_str_len(s);
    size_t ncp = setae_str_count(s);
    size_t *off = malloc((ncp + 1) * sizeof(size_t));
    size_t ci = 0;
    for (size_t i = 0; i <= n; i++) {
        if (i == n || ((unsigned char)p[i] & 0xc0) != 0x80) {
            off[ci++] = i;
            if (ci > ncp) {
                break;
            }
        }
    }
    size_t cap = n + 1, len = 0;
    char *buf = malloc(cap);
    for (int64_t k = 0, cp = start; k < count; k++, cp += step) {
        size_t a = off[cp];
        size_t b = off[cp + 1];
        while (len + (b - a) > cap) {
            cap *= 2;
            buf = realloc(buf, cap);
        }
        memcpy(buf + len, p + a, b - a);
        len += b - a;
    }
    SetaeValue r = setae_str_new(vm->heap, buf, len);
    free(buf);
    free(off);
    return r;
}

static SetaeValue do_slice(SetaeVM *vm, SetaeValue obj, SetaeSlice *sl) {
    int t = setae_obj_type(obj);
    int64_t len;
    if (t == SETAE_T_LIST) {
        len = ((SetaeList *)setae_to_ptr(obj))->len;
    } else if (t == SETAE_T_TUPLE) {
        len = ((SetaeTuple *)setae_to_ptr(obj))->len;
    } else if (t == SETAE_T_STR) {
        len = (int64_t)setae_str_count(obj);
    } else {
        setae_vm_raise(vm, "TypeError", "'%s' object is not subscriptable",
                       setae_type_name(obj));
        return setae_none();
    }
    int64_t start, step, count;
    if (!slice_get(vm, sl, len, &start, &step, &count)) {
        return setae_none();
    }
    setae_vm_push_tmp(vm, obj);
    SetaeValue rv;
    if (t == SETAE_T_STR) {
        rv = slice_str(vm, obj, start, step, count);
    } else if (t == SETAE_T_LIST) {
        rv = setae_list_new(vm->heap, (uint32_t)count);
        SetaeList *r = setae_to_ptr(rv);
        SetaeList *src = setae_to_ptr(obj);
        for (int64_t k = 0, i = start; k < count; k++, i += step) {
            setae_list_push(r, src->items[i]);
        }
    } else {
        rv = setae_tuple_new(vm->heap, NULL, (uint32_t)count);
        SetaeTuple *r = setae_to_ptr(rv);
        SetaeTuple *src = setae_to_ptr(obj);
        for (int64_t k = 0, i = start; k < count; k++, i += step) {
            r->items[k] = src->items[i];
        }
    }
    setae_vm_pop_tmp(vm);
    return rv;
}

static SetaeValue subscript(SetaeVM *vm, SetaeValue obj, SetaeValue idx) {
    if (setae_obj_type(idx) == SETAE_T_SLICE) {
        return do_slice(vm, obj, setae_to_ptr(idx));
    }
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
        return from_i64(vm, r->start + i * r->step);
    }
    default:
        setae_vm_raise(vm, "TypeError", "'%s' object is not subscriptable",
                       setae_type_name(obj));
        return setae_none();
    }
}

static void del_subscript(SetaeVM *vm, SetaeValue obj, SetaeValue idx) {
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
        for (uint32_t j = (uint32_t)i; j + 1 < l->len; j++) {
            l->items[j] = l->items[j + 1];
        }
        l->len--;
        return;
    }
    case SETAE_T_DICT:
        if (!setae_dict_del(setae_to_ptr(obj), idx)) {
            setae_vm_raise(vm, "KeyError", "key not found");
        }
        return;
    default:
        setae_vm_raise(vm, "TypeError", "'%s' object does not support item deletion",
                       setae_type_name(obj));
    }
}

static void del_attr(SetaeVM *vm, SetaeValue obj, const char *name) {
    if (setae_obj_type(obj) == SETAE_T_INSTANCE) {
        SetaeInstance *inst = setae_to_ptr(obj);
        int64_t slot = setae_instance_slot(inst, name);
        if (slot >= 0 && inst->slots[slot] != 0) {
            inst->slots[slot] = 0;
            return;
        }
        setae_vm_raise(vm, "AttributeError", "'%.*s' object has no attribute '%s'",
                       (int)setae_str_len(((SetaeClass *)setae_to_ptr(inst->cls))->name),
                       setae_str_data(((SetaeClass *)setae_to_ptr(inst->cls))->name), name);
        return;
    }
    setae_vm_raise(vm, "TypeError", "'%s' object does not support attribute deletion",
                   setae_type_name(obj));
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
        setae_dict_set(setae_to_ptr(obj), idx, val);
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
    case SETAE_T_SET: {
        SetaeSet *s = setae_to_ptr(it->target);
        while (it->index <= s->mask) {
            SetaeSetEntry *e = &s->table[it->index++];
            if (e->state == SET_ACTIVE) {
                *out = e->key;
                return 1;
            }
        }
        return 0;
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
        *out = from_i64(vm, r->start + (int64_t)it->index * r->step);
        it->index++;
        return 1;
    }
    }
}

static SetaeValue run_code(SetaeVM *vm, const SetaeCode *code, SetaeValue *args,
                           int nargs, const SetaeValue *captured,
                           const SetaeValue *defaults, uint32_t ndefaults,
                           SetaeValue kwargs, SetaeValue module, SetaeGen *gen);

static SetaeValue make_generator(SetaeVM *vm, const SetaeCode *code, SetaeValue *args, int nargs,
                                 const SetaeValue *captured, const SetaeValue *defaults,
                                 uint32_t ndefaults, SetaeValue kwargs, SetaeValue module);
static SetaeValue gen_resume(SetaeVM *vm, SetaeGen *g, SetaeValue sent, int *stopped);

static int has_kwargs_given(SetaeValue kwargs) {
    return kwargs != 0 && ((SetaeDict *)setae_to_ptr(kwargs))->len > 0;
}

static int cur_kwarg(SetaeVM *vm, const char *name, SetaeValue *out) {
    if (vm->cur_kwargs == 0) {
        return 0;
    }
    SetaeDict *d = setae_to_ptr(vm->cur_kwargs);
    size_t len = strlen(name);
    for (uint32_t i = 0; i < d->len; i++) {
        SetaeValue k = d->entries[i].key;
        if (setae_is_str(k) && setae_str_len(k) == len &&
            memcmp(setae_str_data(k), name, len) == 0) {
            *out = d->entries[i].value;
            return 1;
        }
    }
    return 0;
}

static SetaeValue call_value(SetaeVM *vm, SetaeValue callee, SetaeValue *args,
                             int nargs, SetaeValue kwargs) {
    int t = setae_obj_type(callee);
    if (t == SETAE_T_BUILTIN) {
        SetaeBuiltin *b = setae_to_ptr(callee);
        if (has_kwargs_given(kwargs) && !b->kwargs_ok) {
            setae_vm_raise(vm, "TypeError", "%s() takes no keyword arguments", b->name);
            return setae_none();
        }
        SetaeValue saved = vm->cur_kwargs;
        vm->cur_kwargs = kwargs;
        SetaeValue r = b->fn(vm, args, nargs);
        vm->cur_kwargs = saved;
        return r;
    }
    if (t == SETAE_T_FUNCTION) {
        SetaeFunc *f = setae_to_ptr(callee);
        return run_code(vm, f->code, args, nargs, f->cells, f->defaults, f->ndefaults, kwargs,
                        f->module, NULL);
    }
    if (t == SETAE_T_EXCTYPE) {
        SetaeExcType *et = setae_to_ptr(callee);
        if (has_kwargs_given(kwargs)) {
            setae_vm_raise(vm, "TypeError", "%s() takes no keyword arguments", et->name);
            return setae_none();
        }
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
            if (nargs + 1 > 256) {
                setae_vm_raise(vm, "TypeError", "too many arguments");
                return setae_none();
            }
            SetaeValue argv[256];
            argv[0] = inst;
            for (int i = 0; i < nargs; i++) {
                argv[i + 1] = args[i];
            }
            run_code(vm, f->code, argv, nargs + 1, f->cells, f->defaults, f->ndefaults, kwargs,
                     f->module, NULL);
            if (vm->error) {
                return setae_none();
            }
        } else if (nargs != 0 || has_kwargs_given(kwargs)) {
            setae_vm_raise(vm, "TypeError", "%.*s() takes no arguments (%d given)",
                           (int)setae_str_len(c->name), setae_str_data(c->name), nargs);
            return setae_none();
        }
        return inst;
    }
    if (t == SETAE_T_BOUND) {
        SetaeBound *b = setae_to_ptr(callee);
        SetaeFunc *f = setae_to_ptr(b->func);
        if (nargs + 1 > 256) {
            setae_vm_raise(vm, "TypeError", "too many arguments");
            return setae_none();
        }
        SetaeValue argv[256];
        argv[0] = b->self;
        for (int i = 0; i < nargs; i++) {
            argv[i + 1] = args[i];
        }
        return run_code(vm, f->code, argv, nargs + 1, f->cells, f->defaults, f->ndefaults,
                        kwargs, f->module, NULL);
    }
    setae_vm_raise(vm, "TypeError", "'%s' object is not callable", setae_type_name(callee));
    return setae_none();
}

SetaeValue setae_call(SetaeVM *vm, SetaeValue callee, SetaeValue *args, int nargs) {
    return call_value(vm, callee, args, nargs, 0);
}

static SetaeValue call_method(SetaeVM *vm, SetaeValue obj, const char *name,
                              SetaeValue *args, int nargs, SetaeInlineCache *c) {
    int t = setae_obj_type(obj);
    if (t == SETAE_T_INSTANCE) {
        SetaeInstance *inst = setae_to_ptr(obj);
        SetaeValue found;
        if (setae_instance_get(inst, name, &found)) {
            return call_value(vm, found, args, nargs, 0);
        }
        SetaeValue v;
        if (class_lookup(inst->cls, name, &v)) {
            if (setae_obj_type(v) == SETAE_T_FUNCTION) {
                SetaeFunc *f = setae_to_ptr(v);
                c->kind = 4;
                c->shape = inst->shape;
                c->cls = inst->cls;
                c->method = v;
                c->guard = vm->class_version;
                return run_code(vm, f->code, args - 1, nargs + 1, f->cells, f->defaults,
                                f->ndefaults, 0, f->module, NULL);
            }
            return call_value(vm, v, args, nargs, 0);
        }
        attr_error(vm, obj, name);
        return setae_none();
    }
    if (t == SETAE_T_CLASS) {
        SetaeValue v;
        if (class_lookup(obj, name, &v)) {
            return call_value(vm, v, args, nargs, 0);
        }
        attr_error(vm, obj, name);
        return setae_none();
    }
    if (t == SETAE_T_MODULE) {
        SetaeModule *m = setae_to_ptr(obj);
        SetaeDict *d = setae_to_ptr(m->dict);
        int64_t i = dict_find_cstr(d, name);
        if (i >= 0) {
            return call_value(vm, d->entries[i].value, args, nargs, 0);
        }
        attr_error(vm, obj, name);
        return setae_none();
    }
    if (t == SETAE_T_SUBJECT) {
        if (strcmp(name, "send") == 0) {
            if (nargs != 1) {
                setae_vm_raise(vm, "TypeError",
                               "send() takes exactly one argument (%d given)", nargs);
                return setae_none();
            }
            setae_subject_send_value(vm, obj, args[0]);
            return setae_none();
        }
        if (strcmp(name, "call") == 0) {
            if (nargs != 2) {
                setae_vm_raise(vm, "TypeError",
                               "call() takes exactly two arguments (%d given)", nargs);
                return setae_none();
            }
            return setae_subject_call_value(vm, obj, args[0], args[1]);
        }
        if (strcmp(name, "send_after") == 0) {
            if (nargs != 2) {
                setae_vm_raise(vm, "TypeError",
                               "send_after() takes exactly two arguments (%d given)", nargs);
                return setae_none();
            }
            setae_subject_send_after_value(vm, obj, args[0], args[1]);
            return setae_none();
        }
        attr_error(vm, obj, name);
        return setae_none();
    }
    if (t == SETAE_T_GEN) {
        if (strcmp(name, "send") == 0) {
            SetaeValue sent = nargs >= 1 ? args[0] : setae_none();
            SetaeValue out;
            if (setae_gen_next(vm, obj, sent, &out)) {
                return out;
            }
            if (vm->error) {
                return setae_none();
            }
            setae_vm_raise(vm, "StopIteration", "");
            return setae_none();
        }
        attr_error(vm, obj, name);
        return setae_none();
    }
    if (t == SETAE_T_STR) {
        int found;
        SetaeValue r = setae_str_method(vm, obj, name, args, nargs, &found);
        if (found) {
            return r;
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
        if (strcmp(name, "extend") == 0) {
            if (nargs != 1) {
                setae_vm_raise(vm, "TypeError",
                               "extend() takes exactly one argument (%d given)", nargs);
                return setae_none();
            }
            SetaeValue it = setae_make_iter(vm, args[0]);
            if (vm->error) {
                return setae_none();
            }
            setae_vm_push_tmp(vm, it);
            SetaeValue x;
            while (setae_iter_advance(vm, it, &x)) {
                if (vm->error) {
                    break;
                }
                setae_list_push(setae_to_ptr(obj), x);
            }
            setae_vm_pop_tmp(vm);
            return setae_none();
        }
        if (strcmp(name, "insert") == 0) {
            if (nargs != 2 || !setae_is_int(args[0])) {
                setae_vm_raise(vm, "TypeError", "insert() takes an index and a value");
                return setae_none();
            }
            int64_t i = setae_to_int(args[0]);
            if (i < 0) {
                i += l->len;
                if (i < 0) {
                    i = 0;
                }
            }
            if (i > (int64_t)l->len) {
                i = l->len;
            }
            setae_list_push(l, setae_none());
            l = setae_to_ptr(obj);
            for (uint32_t j = l->len - 1; j > (uint32_t)i; j--) {
                l->items[j] = l->items[j - 1];
            }
            l->items[i] = args[1];
            return setae_none();
        }
        if (strcmp(name, "remove") == 0) {
            if (nargs != 1) {
                setae_vm_raise(vm, "TypeError",
                               "remove() takes exactly one argument (%d given)", nargs);
                return setae_none();
            }
            for (uint32_t i = 0; i < l->len; i++) {
                if (setae_value_eq(l->items[i], args[0])) {
                    for (uint32_t j = i; j + 1 < l->len; j++) {
                        l->items[j] = l->items[j + 1];
                    }
                    l->len--;
                    return setae_none();
                }
            }
            setae_vm_raise(vm, "ValueError", "list.remove(x): x not in list");
            return setae_none();
        }
        if (strcmp(name, "index") == 0) {
            if (nargs < 1) {
                setae_vm_raise(vm, "TypeError", "index() takes at least 1 argument");
                return setae_none();
            }
            for (uint32_t i = 0; i < l->len; i++) {
                if (setae_value_eq(l->items[i], args[0])) {
                    return setae_from_int((int32_t)i);
                }
            }
            setae_vm_raise(vm, "ValueError", "is not in list");
            return setae_none();
        }
        if (strcmp(name, "count") == 0) {
            if (nargs != 1) {
                setae_vm_raise(vm, "TypeError",
                               "count() takes exactly one argument (%d given)", nargs);
                return setae_none();
            }
            int32_t c = 0;
            for (uint32_t i = 0; i < l->len; i++) {
                if (setae_value_eq(l->items[i], args[0])) {
                    c++;
                }
            }
            return setae_from_int(c);
        }
        if (strcmp(name, "reverse") == 0) {
            for (uint32_t i = 0, j = l->len; i < j; i++) {
                j--;
                SetaeValue tmp = l->items[i];
                l->items[i] = l->items[j];
                l->items[j] = tmp;
            }
            return setae_none();
        }
        if (strcmp(name, "clear") == 0) {
            l->len = 0;
            return setae_none();
        }
        if (strcmp(name, "copy") == 0) {
            SetaeValue rv = setae_list_new(vm->heap, l->len);
            SetaeList *r = setae_to_ptr(rv);
            SetaeList *src = setae_to_ptr(obj);
            for (uint32_t i = 0; i < src->len; i++) {
                setae_list_push(r, src->items[i]);
            }
            return rv;
        }
        if (strcmp(name, "sort") == 0) {
            SetaeValue keyfn = setae_none();
            int reverse = 0;
            SetaeValue kv;
            if (cur_kwarg(vm, "key", &kv)) {
                keyfn = kv;
            }
            if (cur_kwarg(vm, "reverse", &kv)) {
                reverse = setae_truthy(kv);
            }
            SetaeValue keysv = setae_list_new(vm->heap, l->len);
            setae_vm_push_tmp(vm, keysv);
            SetaeList *keys = setae_to_ptr(keysv);
            for (uint32_t i = 0; i < l->len; i++) {
                SetaeValue k = setae_is_none(keyfn)
                                   ? l->items[i]
                                   : setae_call(vm, keyfn, &l->items[i], 1);
                if (vm->error) {
                    setae_vm_pop_tmp(vm);
                    return setae_none();
                }
                setae_list_push(keys, k);
            }
            l = setae_to_ptr(obj);
            for (uint32_t i = 1; i < l->len; i++) {
                SetaeValue kk = keys->items[i];
                SetaeValue vv = l->items[i];
                uint32_t j = i;
                while (j > 0) {
                    int lt = reverse ? setae_value_lt(vm, keys->items[j - 1], kk)
                                     : setae_value_lt(vm, kk, keys->items[j - 1]);
                    if (vm->error) {
                        setae_vm_pop_tmp(vm);
                        return setae_none();
                    }
                    if (!lt) {
                        break;
                    }
                    keys->items[j] = keys->items[j - 1];
                    l->items[j] = l->items[j - 1];
                    j--;
                }
                keys->items[j] = kk;
                l->items[j] = vv;
            }
            setae_vm_pop_tmp(vm);
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
        if (strcmp(name, "setdefault") == 0) {
            if (nargs < 1 || nargs > 2) {
                setae_vm_raise(vm, "TypeError", "setdefault expected 1 or 2 arguments");
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
            SetaeValue def = nargs == 2 ? args[1] : setae_none();
            setae_dict_set(d, args[0], def);
            return def;
        }
        if (strcmp(name, "pop") == 0) {
            if (nargs < 1 || nargs > 2) {
                setae_vm_raise(vm, "TypeError", "pop expected 1 or 2 arguments");
                return setae_none();
            }
            int64_t i = dict_find(d, args[0]);
            if (i >= 0) {
                SetaeValue v = d->entries[i].value;
                setae_dict_del(d, args[0]);
                return v;
            }
            if (nargs == 2) {
                return args[1];
            }
            setae_vm_raise(vm, "KeyError", "");
            return setae_none();
        }
        if (strcmp(name, "popitem") == 0) {
            if (d->len == 0) {
                setae_vm_raise(vm, "KeyError", "'popitem(): dictionary is empty'");
                return setae_none();
            }
            uint32_t last = d->len - 1;
            SetaeValue kv[2] = {d->entries[last].key, d->entries[last].value};
            SetaeValue r = setae_tuple_new(vm->heap, kv, 2);
            setae_vm_push_tmp(vm, r);
            setae_dict_del(setae_to_ptr(obj), kv[0]);
            setae_vm_pop_tmp(vm);
            return r;
        }
        if (strcmp(name, "clear") == 0) {
            d->len = 0;
            free(d->index);
            d->index = NULL;
            d->index_cap = 0;
            return setae_none();
        }
        if (strcmp(name, "copy") == 0) {
            SetaeValue rv = setae_dict_new(vm->heap);
            setae_vm_push_tmp(vm, rv);
            SetaeDict *src = setae_to_ptr(obj);
            for (uint32_t i = 0; i < src->len; i++) {
                setae_dict_set(setae_to_ptr(rv), src->entries[i].key, src->entries[i].value);
            }
            setae_vm_pop_tmp(vm);
            return rv;
        }
        if (strcmp(name, "update") == 0) {
            if (nargs != 1) {
                setae_vm_raise(vm, "TypeError", "update expected 1 argument, got %d", nargs);
                return setae_none();
            }
            if (setae_obj_type(args[0]) == SETAE_T_DICT) {
                SetaeDict *o = setae_to_ptr(args[0]);
                for (uint32_t i = 0; i < o->len; i++) {
                    setae_dict_set(setae_to_ptr(obj), o->entries[i].key, o->entries[i].value);
                }
                return setae_none();
            }
            SetaeValue it = setae_make_iter(vm, args[0]);
            if (vm->error) {
                return setae_none();
            }
            setae_vm_push_tmp(vm, it);
            SetaeValue pair;
            while (setae_iter_advance(vm, it, &pair)) {
                if (vm->error) {
                    break;
                }
                setae_vm_push_tmp(vm, pair);
                SetaeValue pl = setae_iter_collect(vm, pair);
                setae_vm_pop_tmp(vm);
                if (vm->error) {
                    break;
                }
                SetaeList *pll = setae_to_ptr(pl);
                if (pll->len != 2) {
                    setae_vm_raise(vm, "ValueError", "dictionary update sequence element "
                                                     "has the wrong length");
                    break;
                }
                setae_dict_set(setae_to_ptr(obj), pll->items[0], pll->items[1]);
            }
            setae_vm_pop_tmp(vm);
            return setae_none();
        }
    } else if (t == SETAE_T_SET) {
        SetaeSet *s = setae_to_ptr(obj);
        if (s->frozen &&
            (strcmp(name, "add") == 0 || strcmp(name, "discard") == 0 ||
             strcmp(name, "remove") == 0 || strcmp(name, "pop") == 0 ||
             strcmp(name, "clear") == 0 || strcmp(name, "update") == 0)) {
            setae_vm_raise(vm, "AttributeError",
                           "'frozenset' object has no attribute '%s'", name);
            return setae_none();
        }
        if (strcmp(name, "add") == 0) {
            if (nargs != 1) {
                setae_vm_raise(vm, "TypeError", "add() takes exactly one argument (%d given)",
                               nargs);
                return setae_none();
            }
            if (!hashable(args[0])) {
                setae_vm_raise(vm, "TypeError", "unhashable type: '%s'",
                               setae_type_name(args[0]));
                return setae_none();
            }
            setae_set_add(s, args[0]);
            return setae_none();
        }
        if (strcmp(name, "discard") == 0) {
            if (nargs != 1) {
                setae_vm_raise(vm, "TypeError",
                               "discard() takes exactly one argument (%d given)", nargs);
                return setae_none();
            }
            setae_set_discard(s, args[0]);
            return setae_none();
        }
        if (strcmp(name, "remove") == 0) {
            if (nargs != 1) {
                setae_vm_raise(vm, "TypeError",
                               "remove() takes exactly one argument (%d given)", nargs);
                return setae_none();
            }
            if (!setae_set_discard(s, args[0])) {
                setae_vm_raise(vm, "KeyError", "");
            }
            return setae_none();
        }
        if (strcmp(name, "clear") == 0) {
            for (uint32_t i = 0; i <= s->mask; i++) {
                s->table[i].state = SET_EMPTY;
                s->table[i].key = 0;
            }
            s->fill = 0;
            s->used = 0;
            return setae_none();
        }
        if (strcmp(name, "copy") == 0) {
            SetaeValue cp = setae_set_new(vm->heap);
            setae_set_merge(setae_to_ptr(cp), s);
            return cp;
        }
        if (strcmp(name, "pop") == 0) {
            for (uint32_t i = 0; i <= s->mask; i++) {
                if (s->table[i].state == SET_ACTIVE) {
                    SetaeValue k = s->table[i].key;
                    s->table[i].state = SET_DUMMY;
                    s->table[i].key = 0;
                    s->used--;
                    return k;
                }
            }
            setae_vm_raise(vm, "KeyError", "pop from an empty set");
            return setae_none();
        }
        if (strcmp(name, "union") == 0 || strcmp(name, "update") == 0) {
            int inplace = name[0] == 'u' && name[1] == 'p';
            SetaeValue rv;
            SetaeSet *r;
            if (inplace) {
                rv = obj;
                r = s;
            } else {
                rv = setae_set_new(vm->heap);
                setae_vm_push_tmp(vm, rv);
                r = setae_to_ptr(rv);
                setae_set_merge(r, s);
            }
            for (int ai = 0; ai < nargs; ai++) {
                SetaeValue it = setae_make_iter(vm, args[ai]);
                if (vm->error) {
                    break;
                }
                setae_vm_push_tmp(vm, it);
                SetaeValue x;
                while (setae_iter_advance(vm, it, &x)) {
                    if (vm->error) {
                        break;
                    }
                    setae_set_add(r, x);
                }
                setae_vm_pop_tmp(vm);
                if (vm->error) {
                    break;
                }
            }
            if (!inplace) {
                setae_vm_pop_tmp(vm);
            }
            return inplace ? setae_none() : (vm->error ? setae_none() : rv);
        }
        if (strcmp(name, "intersection") == 0 || strcmp(name, "difference") == 0 ||
            strcmp(name, "symmetric_difference") == 0) {
            SetaeBinOp bop = name[0] == 'i' ? BIN_BITAND
                             : name[0] == 'd' ? BIN_SUB
                                              : BIN_BITXOR;
            SetaeValue acc = setae_set_new(vm->heap);
            setae_vm_push_tmp(vm, acc);
            setae_set_merge(setae_to_ptr(acc), s);
            for (int ai = 0; ai < nargs; ai++) {
                SetaeValue o = coerce_to_set(vm, args[ai]);
                if (vm->error) {
                    setae_vm_pop_tmp(vm);
                    return setae_none();
                }
                setae_vm_push_tmp(vm, o);
                SetaeValue next =
                    set_binop(vm, bop, setae_to_ptr(acc), setae_to_ptr(o));
                setae_vm_pop_tmp(vm);
                acc = next;
                vm->tmp_roots[vm->ntmp - 1] = acc;
            }
            setae_vm_pop_tmp(vm);
            return acc;
        }
        if (strcmp(name, "issubset") == 0 || strcmp(name, "issuperset") == 0 ||
            strcmp(name, "isdisjoint") == 0) {
            if (nargs != 1) {
                setae_vm_raise(vm, "TypeError", "%s() takes exactly one argument (%d given)",
                               name, nargs);
                return setae_none();
            }
            SetaeValue o = coerce_to_set(vm, args[0]);
            if (vm->error) {
                return setae_none();
            }
            SetaeSet *so = setae_to_ptr(o);
            if (name[2] == 'd') {
                for (uint32_t i = 0; i <= s->mask; i++) {
                    if (s->table[i].state == SET_ACTIVE &&
                        setae_set_contains(so, s->table[i].key)) {
                        return setae_bool(0);
                    }
                }
                return setae_bool(1);
            }
            int sub = name[2] == 's' && name[4] == 'b' ? set_is_subset(s, so)
                                                       : set_is_subset(so, s);
            return setae_bool(sub);
        }
    } else if (t == SETAE_T_BUILTIN) {
        SetaeBuiltin *bt = setae_to_ptr(obj);
        if (strcmp(bt->name, "dict") == 0 && strcmp(name, "fromkeys") == 0) {
            if (nargs < 1 || nargs > 2) {
                setae_vm_raise(vm, "TypeError",
                               "fromkeys expected 1 or 2 arguments, got %d", nargs);
                return setae_none();
            }
            SetaeValue val = nargs == 2 ? args[1] : setae_none();
            SetaeValue dv = setae_dict_new(vm->heap);
            setae_vm_push_tmp(vm, dv);
            SetaeValue it = setae_make_iter(vm, args[0]);
            if (vm->error) {
                setae_vm_pop_tmp(vm);
                return setae_none();
            }
            setae_vm_push_tmp(vm, it);
            SetaeValue k;
            while (setae_iter_advance(vm, it, &k)) {
                if (vm->error) {
                    break;
                }
                setae_dict_set(setae_to_ptr(dv), k, val);
            }
            setae_vm_pop_tmp(vm);
            setae_vm_pop_tmp(vm);
            return vm->error ? setae_none() : dv;
        }
    }
    setae_vm_raise(vm, "AttributeError", "'%s' object has no attribute '%s'",
                   setae_type_name(obj), name);
    return setae_none();
}

static SetaeValue run_code(SetaeVM *vm, const SetaeCode *code, SetaeValue *args,
                           int nargs, const SetaeValue *captured,
                           const SetaeValue *defaults, uint32_t ndefaults,
                           SetaeValue kwargs, SetaeValue module, SetaeGen *gen);

static int bind_args(SetaeVM *vm, const SetaeCode *code, SetaeValue *args, int nargs,
                     const SetaeValue *defaults, uint32_t ndefaults, SetaeValue kwargs,
                     SetaeValue *locals) {
    uint32_t k = setae_code_nparams(code);
    int has_va = setae_code_varargs(code);
    int has_kw = setae_code_kwargs(code);
    uint32_t va_slot = k;
    uint32_t kw_slot = k + (has_va ? 1u : 0u);
    uint32_t required = k - ndefaults;
    uint32_t npos = (uint32_t)nargs;

    if (k > 256) {
        setae_vm_raise(vm, "RuntimeError", "too many parameters");
        return 0;
    }
    uint8_t filled[256];
    memset(filled, 0, k);

    uint32_t nbind = npos < k ? npos : k;
    for (uint32_t i = 0; i < nbind; i++) {
        locals[i] = args[i];
        filled[i] = 1;
    }
    if (npos > k) {
        if (has_va) {
            locals[va_slot] = setae_tuple_new(vm->heap, &args[k], npos - k);
        } else {
            setae_vm_raise(vm, "TypeError",
                           "%s() takes %u positional arguments but %u were given",
                           setae_code_fname(code), k, npos);
            return 0;
        }
    } else if (has_va) {
        locals[va_slot] = setae_tuple_new(vm->heap, NULL, 0);
    }

    SetaeValue kwdict = 0;
    if (has_kw) {
        kwdict = setae_dict_new(vm->heap);
        locals[kw_slot] = kwdict;
    }
    if (kwargs != 0) {
        SetaeDict *kd = setae_to_ptr(kwargs);
        for (uint32_t e = 0; e < kd->len; e++) {
            SetaeValue key = kd->entries[e].key;
            SetaeValue val = kd->entries[e].value;
            int matched = -1;
            if (setae_is_str(key)) {
                size_t klen = setae_str_len(key);
                const char *kdata = setae_str_data(key);
                for (uint32_t p = 0; p < k; p++) {
                    const char *pn = setae_code_param_name(code, p);
                    if (strlen(pn) == klen && memcmp(pn, kdata, klen) == 0) {
                        matched = (int)p;
                        break;
                    }
                }
            }
            if (matched >= 0) {
                if (filled[matched]) {
                    setae_vm_raise(vm, "TypeError",
                                   "%s() got multiple values for argument '%s'",
                                   setae_code_fname(code), setae_code_param_name(code, matched));
                    return 0;
                }
                locals[matched] = val;
                filled[matched] = 1;
            } else if (has_kw) {
                setae_dict_set(setae_to_ptr(kwdict), key, val);
            } else if (setae_is_str(key)) {
                setae_vm_raise(vm, "TypeError",
                               "%s() got an unexpected keyword argument '%.*s'",
                               setae_code_fname(code), (int)setae_str_len(key),
                               setae_str_data(key));
                return 0;
            } else {
                setae_vm_raise(vm, "TypeError", "keywords must be strings");
                return 0;
            }
        }
    }

    for (uint32_t i = 0; i < k; i++) {
        if (!filled[i]) {
            if (i >= required) {
                locals[i] = defaults[i - required];
            } else {
                setae_vm_raise(vm, "TypeError",
                               "%s() missing a required positional argument: '%s'",
                               setae_code_fname(code), setae_code_param_name(code, i));
                return 0;
            }
        }
    }
    return 1;
}

static SetaeValue run_code(SetaeVM *vm, const SetaeCode *code, SetaeValue *args,
                           int nargs, const SetaeValue *captured,
                           const SetaeValue *defaults, uint32_t ndefaults,
                           SetaeValue kwargs, SetaeValue module, SetaeGen *gen) {
    if (gen == NULL && setae_code_generator(code)) {
        return make_generator(vm, code, args, nargs, captured, defaults, ndefaults, kwargs,
                              module);
    }
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
    uint32_t ncells = setae_code_ncells(code);
    uint32_t nfrees = setae_code_nfrees(code);
    uint32_t fixed = nlocals + ncells + nfrees;

    uint32_t frame_cap;
    SetaeValue *frame;
    int sp;
    uint32_t ip;
    if (gen != NULL) {
        frame = gen->frame;
        frame_cap = gen->frame_cap;
        sp = gen->sp;
        ip = gen->ip;
    } else {
        frame = frame_alloc(vm, fixed + STACK_MAX, &frame_cap);
        memset(frame, 0, fixed * sizeof(SetaeValue));
        sp = 0;
        ip = 0;
    }
    SetaeValue *locals = frame;
    SetaeValue *cellbase = frame + nlocals;
    SetaeValue *stack = frame + fixed;

    SetaeFrame fr = {frame, fixed, sp, module, vm->frames};
    vm->frames = &fr;

    if (gen != NULL) {
        if (gen->resumed) {
            stack[sp++] = kwargs;
        }
        gen->resumed = 1;
    } else {
        if (!bind_args(vm, code, args, nargs, defaults, ndefaults, kwargs, locals)) {
            vm->frames = fr.parent;
            frame_release(vm, frame, frame_cap);
            vm->depth--;
            return setae_none();
        }
        for (uint32_t i = 0; i < ncells; i++) {
            cellbase[i] = setae_cell_new(vm->heap);
        }
        for (uint32_t i = 0; i < nfrees; i++) {
            cellbase[ncells + i] = captured[i];
        }
    }

    int limited = vm->step_limit != 0 || vm->deadline_ns != 0;
    SetaeValue result = setae_none();
    uint32_t ext = 0;
    uint32_t unit = 0;
    uint32_t arg = 0;
    uint8_t op = 0;

    static void *const table[] = {
        [OP_LOAD_CONST] = &&L_OP_LOAD_CONST,
        [OP_LOAD_NAME] = &&L_OP_LOAD_NAME,
        [OP_STORE_NAME] = &&L_OP_STORE_NAME,
        [OP_LOAD_LOCAL] = &&L_OP_LOAD_LOCAL,
        [OP_STORE_LOCAL] = &&L_OP_STORE_LOCAL,
        [OP_POP_TOP] = &&L_OP_POP_TOP,
        [OP_BINARY_OP] = &&L_OP_BINARY_OP,
        [OP_CALL] = &&L_OP_CALL,
        [OP_RETURN] = &&L_OP_RETURN,
        [OP_JUMP] = &&L_OP_JUMP,
        [OP_POP_JUMP_IF_FALSE] = &&L_OP_POP_JUMP_IF_FALSE,
        [OP_POP_JUMP_IF_TRUE] = &&L_OP_POP_JUMP_IF_TRUE,
        [OP_JUMP_IF_FALSE_OR_POP] = &&L_OP_JUMP_IF_FALSE_OR_POP,
        [OP_JUMP_IF_TRUE_OR_POP] = &&L_OP_JUMP_IF_TRUE_OR_POP,
        [OP_COMPARE_OP] = &&L_OP_COMPARE_OP,
        [OP_UNARY_NEG] = &&L_OP_UNARY_NEG,
        [OP_UNARY_NOT] = &&L_OP_UNARY_NOT,
        [OP_UNARY_INVERT] = &&L_OP_UNARY_INVERT,
        [OP_MAKE_FUNCTION] = &&L_OP_MAKE_FUNCTION,
        [OP_BUILD_LIST] = &&L_OP_BUILD_LIST,
        [OP_BUILD_DICT] = &&L_OP_BUILD_DICT,
        [OP_BUILD_SET] = &&L_OP_BUILD_SET,
        [OP_BUILD_SET_CONST] = &&L_OP_BUILD_SET_CONST,
        [OP_BUILD_SLICE] = &&L_OP_BUILD_SLICE,
        [OP_SUBSCR] = &&L_OP_SUBSCR,
        [OP_STORE_SUBSCR] = &&L_OP_STORE_SUBSCR,
        [OP_GET_ITER] = &&L_OP_GET_ITER,
        [OP_FOR_ITER] = &&L_OP_FOR_ITER,
        [OP_CALL_METHOD] = &&L_OP_CALL_METHOD,
        [OP_CALL_METHOD_KW] = &&L_OP_CALL_METHOD_KW,
        [OP_EXTENDED_ARG] = &&L_OP_EXTENDED_ARG,
        [OP_LOAD_CLOSURE] = &&L_OP_LOAD_CLOSURE,
        [OP_LOAD_DEREF] = &&L_OP_LOAD_DEREF,
        [OP_STORE_DEREF] = &&L_OP_STORE_DEREF,
        [OP_BUILD_TUPLE] = &&L_OP_BUILD_TUPLE,
        [OP_UNPACK_SEQUENCE] = &&L_OP_UNPACK_SEQUENCE,
        [OP_RAISE] = &&L_OP_RAISE,
        [OP_EXC_MATCH] = &&L_OP_EXC_MATCH,
        [OP_RERAISE] = &&L_OP_RERAISE,
        [OP_LOAD_ATTR] = &&L_OP_LOAD_ATTR,
        [OP_STORE_ATTR] = &&L_OP_STORE_ATTR,
        [OP_MAKE_CLASS] = &&L_OP_MAKE_CLASS,
        [OP_IMPORT] = &&L_OP_IMPORT,
        [OP_IMPORT_MISSING] = &&L_OP_IMPORT_MISSING,
        [OP_CALL_EX] = &&L_OP_CALL_EX,
        [OP_LIST_EXTEND] = &&L_OP_LIST_EXTEND,
        [OP_DICT_MERGE] = &&L_OP_DICT_MERGE,
        [OP_DUP_TOP] = &&L_OP_DUP_TOP,
        [OP_DELETE_NAME] = &&L_OP_DELETE_NAME,
        [OP_DELETE_SUBSCR] = &&L_OP_DELETE_SUBSCR,
        [OP_DELETE_ATTR] = &&L_OP_DELETE_ATTR,
        [OP_DELETE_LOCAL] = &&L_OP_DELETE_LOCAL,
        [OP_ROT_TWO] = &&L_OP_ROT_TWO,
        [OP_ROT_THREE] = &&L_OP_ROT_THREE,
        [OP_DELETE_DEREF] = &&L_OP_DELETE_DEREF,
        [OP_FORMAT_VALUE] = &&L_OP_FORMAT_VALUE,
        [OP_FORMAT_SPEC] = &&L_OP_FORMAT_SPEC,
        [OP_YIELD_VALUE] = &&L_OP_YIELD_VALUE,
        [OP_AWAIT] = &&L_OP_AWAIT,
    };

#define DISPATCH()                                                             \
    do {                                                                       \
        if (vm->error || ip >= ncode || limited)                               \
            goto slow_dispatch;                                                \
        if (sp >= STACK_MAX - 1)                                               \
            goto stack_overflow;                                               \
        fr.sp = sp;                                                            \
        unit = ip / 2;                                                         \
        op = bytes[ip];                                                        \
        arg = ext | bytes[ip + 1];                                             \
        ext = 0;                                                               \
        ip += 2;                                                               \
        goto *table[op];                                                       \
    } while (0)

    DISPATCH();

slow_dispatch:
    if (vm->error)
        goto handle_error;
    if (ip >= ncode)
        goto loop_done;
    fr.sp = sp;
    if (limited) {
        vm->steps++;
        if (vm->step_limit != 0 && vm->steps > vm->step_limit) {
            vm->interrupted = 1;
            vm->error = 1;
            snprintf(vm->errmsg, sizeof(vm->errmsg), "RuntimeError: step limit exceeded");
            goto loop_done;
        }
        if (vm->deadline_ns != 0 && (vm->steps & 0xfff) == 0 &&
            monotonic_ns() > vm->deadline_ns) {
            vm->interrupted = 1;
            vm->error = 1;
            snprintf(vm->errmsg, sizeof(vm->errmsg), "RuntimeError: time limit exceeded");
            goto loop_done;
        }
    }
    if (sp >= STACK_MAX - 1)
        goto stack_overflow;
    unit = ip / 2;
    op = bytes[ip];
    arg = ext | bytes[ip + 1];
    ext = 0;
    ip += 2;
    goto *table[op];

stack_overflow:
    setae_vm_raise(vm, "RuntimeError", "value stack overflow");
    goto handle_error;

        L_OP_LOAD_CONST:
            stack[sp++] = consts[arg];
            DISPATCH();
        L_OP_LOAD_NAME: {
            SetaeInlineCache *c = &ic[unit];
            if (c->kind == 1) {
                stack[sp++] = vm->globals[c->slot].value;
                DISPATCH();
            }
            SetaeDict *md =
                module != 0 ? setae_to_ptr(((SetaeModule *)setae_to_ptr(module))->dict) : NULL;
            if (c->kind == 2) {
                stack[sp++] = md->entries[c->slot].value;
                DISPATCH();
            }
            uint32_t guard = md != NULL ? md->len : (uint32_t)vm->nglobals;
            if (c->kind == 3 && c->guard == guard) {
                stack[sp++] = vm->builtins[c->slot].value;
                DISPATCH();
            }
            const char *name = setae_code_name(code, arg);
            if (md != NULL) {
                int64_t i = dict_find_cstr(md, name);
                if (i >= 0) {
                    c->kind = 2;
                    c->slot = (uint32_t)i;
                    stack[sp++] = md->entries[i].value;
                    DISPATCH();
                }
            } else {
                int64_t i = tab_find(vm->globals, vm->nglobals, vm->globals_index,
                                     vm->globals_index_cap, name);
                if (i >= 0) {
                    c->kind = 1;
                    c->slot = (uint32_t)i;
                    stack[sp++] = vm->globals[i].value;
                    DISPATCH();
                }
            }
            int64_t bi = tab_find(vm->builtins, vm->nbuiltins, vm->builtins_index,
                                  vm->builtins_index_cap, name);
            if (bi >= 0) {
                c->kind = 3;
                c->slot = (uint32_t)bi;
                c->guard = guard;
                stack[sp++] = vm->builtins[bi].value;
                DISPATCH();
            }
            setae_vm_raise(vm, "NameError", "name '%s' is not defined", name);
            DISPATCH();
        }
        L_OP_STORE_NAME: {
            const char *name = setae_code_name(code, arg);
            SetaeValue val = stack[--sp];
            if (module != 0) {
                SetaeDict *d = setae_to_ptr(((SetaeModule *)setae_to_ptr(module))->dict);
                SetaeValue key = setae_str_new(vm->heap, name, strlen(name));
                setae_dict_set(d, key, val);
            } else {
                setae_vm_set_global(vm, name, val);
            }
            DISPATCH();
        }
        L_OP_LOAD_LOCAL:
            if (locals[arg] == 0) {
                setae_vm_raise(
                    vm, "UnboundLocalError", "local variable referenced before assignment");
                DISPATCH();
            }
            stack[sp++] = locals[arg];
            DISPATCH();
        L_OP_STORE_LOCAL:
            locals[arg] = stack[--sp];
            DISPATCH();
        L_OP_POP_TOP:
            sp--;
            DISPATCH();
        L_OP_DUP_TOP:
            stack[sp] = stack[sp - 1];
            sp++;
            DISPATCH();
        L_OP_ROT_TWO: {
            SetaeValue t = stack[sp - 1];
            stack[sp - 1] = stack[sp - 2];
            stack[sp - 2] = t;
            DISPATCH();
        }
        L_OP_ROT_THREE: {
            SetaeValue t = stack[sp - 1];
            stack[sp - 1] = stack[sp - 2];
            stack[sp - 2] = stack[sp - 3];
            stack[sp - 3] = t;
            DISPATCH();
        }
        L_OP_DELETE_DEREF: {
            SetaeCell *cell = setae_to_ptr(cellbase[arg]);
            if (cell->value == 0) {
                setae_vm_raise(vm, "UnboundLocalError",
                               "variable referenced before assignment");
                DISPATCH();
            }
            cell->value = 0;
            DISPATCH();
        }
        L_OP_FORMAT_SPEC: {
            SetaeValue spec = stack[--sp];
            stack[sp - 1] = setae_format_spec(vm, stack[sp - 1], spec, (int)arg);
            DISPATCH();
        }
        L_OP_FORMAT_VALUE:
            stack[sp - 1] = setae_format_value(vm, stack[sp - 1], (int)arg);
            DISPATCH();
        L_OP_YIELD_VALUE:
            gen->ip = ip;
            gen->sp = sp - 1;
            result = stack[sp - 1];
            vm->frames = fr.parent;
            vm->depth--;
            return result;
        L_OP_AWAIT: {
            SetaeValue sendval = stack[sp - 1];
            SetaeValue awaitable = stack[sp - 2];
            int at = setae_obj_type(awaitable);
            if (at == SETAE_T_GEN) {
                SetaeGen *g = setae_to_ptr(awaitable);
                if (!g->coroutine && !g->resumed) {
                    setae_vm_raise(vm, "TypeError", "object '%s' is not awaitable",
                                   setae_type_name(awaitable));
                    DISPATCH();
                }
            } else if (at == SETAE_T_INSTANCE) {
                SetaeValue m = load_attr(vm, awaitable, "__await__");
                if (vm->error) {
                    DISPATCH();
                }
                SetaeValue it = call_value(vm, m, NULL, 0, 0);
                if (vm->error) {
                    DISPATCH();
                }
                if (setae_obj_type(it) != SETAE_T_GEN) {
                    setae_vm_raise(vm, "TypeError", "__await__ must return an iterator");
                    DISPATCH();
                }
                stack[sp - 2] = it;
                awaitable = it;
            } else {
                setae_vm_raise(vm, "TypeError", "object '%s' is not awaitable",
                               setae_type_name(awaitable));
                DISPATCH();
            }
            SetaeGen *sub = setae_to_ptr(awaitable);
            int stopped;
            SetaeValue y = gen_resume(vm, sub, sendval, &stopped);
            if (vm->error) {
                DISPATCH();
            }
            if (stopped) {
                sp -= 2;
                stack[sp++] = sub->retval;
                DISPATCH();
            }
            if (gen == NULL) {
                stack[sp - 1] = setae_none();
                DISPATCH();
            }
            sp--;
            gen->ip = ip - 2;
            gen->sp = sp;
            result = y;
            vm->frames = fr.parent;
            vm->depth--;
            return result;
        }
        L_OP_DELETE_LOCAL:
            if (locals[arg] == 0) {
                setae_vm_raise(vm, "UnboundLocalError",
                               "local variable referenced before assignment");
                DISPATCH();
            }
            locals[arg] = 0;
            DISPATCH();
        L_OP_DELETE_NAME: {
            const char *name = setae_code_name(code, arg);
            if (module != 0) {
                SetaeDict *d = setae_to_ptr(((SetaeModule *)setae_to_ptr(module))->dict);
                if (!setae_dict_del_cstr(d, name)) {
                    setae_vm_raise(vm, "NameError", "name '%s' is not defined", name);
                }
            } else if (!setae_vm_del_global(vm, name)) {
                setae_vm_raise(vm, "NameError", "name '%s' is not defined", name);
            }
            DISPATCH();
        }
        L_OP_DELETE_SUBSCR: {
            SetaeValue idx = stack[--sp];
            SetaeValue obj = stack[--sp];
            del_subscript(vm, obj, idx);
            DISPATCH();
        }
        L_OP_DELETE_ATTR: {
            const char *name = setae_code_name(code, arg);
            SetaeValue obj = stack[--sp];
            del_attr(vm, obj, name);
            DISPATCH();
        }
        L_OP_BINARY_OP: {
            SetaeValue b = stack[--sp];
            SetaeValue a = stack[--sp];
            stack[sp++] = binary_op(vm, (SetaeBinOp)(arg & 0x7f), (int)(arg >> 7), a, b);
            DISPATCH();
        }
        L_OP_CALL: {
            int n = (int)arg;
            SetaeValue *argv = &stack[sp - n];
            SetaeValue callee = stack[sp - n - 1];
            SetaeValue r = call_value(vm, callee, argv, n, 0);
            sp -= n + 1;
            stack[sp++] = r;
            DISPATCH();
        }
        L_OP_RETURN:
            result = stack[--sp];
            ip = ncode;
            DISPATCH();
        L_OP_JUMP:
            ip = arg * 2;
            DISPATCH();
        L_OP_POP_JUMP_IF_FALSE:
            if (!truthy(stack[--sp])) {
                ip = arg * 2;
            }
            DISPATCH();
        L_OP_POP_JUMP_IF_TRUE:
            if (truthy(stack[--sp])) {
                ip = arg * 2;
            }
            DISPATCH();
        L_OP_JUMP_IF_FALSE_OR_POP:
            if (!truthy(stack[sp - 1])) {
                ip = arg * 2;
            } else {
                sp--;
            }
            DISPATCH();
        L_OP_JUMP_IF_TRUE_OR_POP:
            if (truthy(stack[sp - 1])) {
                ip = arg * 2;
            } else {
                sp--;
            }
            DISPATCH();
        L_OP_COMPARE_OP: {
            SetaeValue b = stack[--sp];
            SetaeValue a = stack[--sp];
            stack[sp++] = compare(vm, (SetaeCmpOp)arg, a, b);
            DISPATCH();
        }
        L_OP_UNARY_NEG:
            stack[sp - 1] = unary_neg(vm, stack[sp - 1]);
            DISPATCH();
        L_OP_UNARY_NOT:
            stack[sp - 1] = setae_bool(!truthy(stack[sp - 1]));
            DISPATCH();
        L_OP_UNARY_INVERT: {
            SetaeValue a = stack[sp - 1];
            if (setae_is_bool(a)) {
                stack[sp - 1] = from_i64(vm, ~(int64_t)(setae_to_bool(a) ? 1 : 0));
            } else if (setae_is_int(a)) {
                stack[sp - 1] = from_i64(vm, ~(int64_t)setae_to_int(a));
            } else {
                setae_vm_raise(vm, "TypeError", "bad operand type for unary ~: '%s'",
                               setae_type_name(a));
            }
            DISPATCH();
        }
        L_OP_MAKE_FUNCTION: {
            const SetaeCode *child = setae_code_child(code, arg);
            if (child == NULL) {
                setae_vm_raise(vm, "RuntimeError", "bad code index %u", arg);
                DISPATCH();
            }
            uint32_t nf = setae_code_nfrees(child);
            uint32_t nd = setae_code_ndefaults(child);
            SetaeValue f = setae_func_new(vm->heap, child, &stack[sp - nf], nf,
                                          &stack[sp - nf - nd], nd, module);
            sp -= (int)(nf + nd);
            stack[sp++] = f;
            DISPATCH();
        }
        L_OP_LOAD_CLOSURE:
            stack[sp++] = cellbase[arg];
            DISPATCH();
        L_OP_LOAD_DEREF: {
            SetaeCell *cell = setae_to_ptr(cellbase[arg]);
            if (cell->value == 0) {
                setae_vm_raise(
                    vm, "UnboundLocalError", "variable referenced before assignment");
                DISPATCH();
            }
            stack[sp++] = cell->value;
            DISPATCH();
        }
        L_OP_STORE_DEREF: {
            SetaeCell *cell = setae_to_ptr(cellbase[arg]);
            cell->value = stack[--sp];
            DISPATCH();
        }
        L_OP_BUILD_TUPLE: {
            int n = (int)arg;
            SetaeValue tv = setae_tuple_new(vm->heap, &stack[sp - n], (uint32_t)n);
            sp -= n;
            stack[sp++] = tv;
            DISPATCH();
        }
        L_OP_LOAD_ATTR: {
            SetaeValue obj = stack[sp - 1];
            if (setae_obj_type(obj) == SETAE_T_INSTANCE) {
                SetaeInstance *inst = setae_to_ptr(obj);
                if (ic[unit].shape == inst->shape && inst->slots[ic[unit].slot] != 0) {
                    stack[sp - 1] = inst->slots[ic[unit].slot];
                    DISPATCH();
                }
                int64_t slot = setae_instance_slot(inst, setae_code_name(code, arg));
                if (slot >= 0 && inst->slots[slot] != 0) {
                    ic[unit].shape = inst->shape;
                    ic[unit].slot = (uint32_t)slot;
                    stack[sp - 1] = inst->slots[slot];
                    DISPATCH();
                }
            }
            stack[sp - 1] = load_attr(vm, obj, setae_code_name(code, arg));
            DISPATCH();
        }
        L_OP_STORE_ATTR: {
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
                    DISPATCH();
                }
                const char *name = setae_code_name(code, arg);
                SetaeShape *from = inst->shape;
                setae_instance_set(vm->heap, inst, name, val);
                ic[unit].shape = from;
                ic[unit].next = inst->shape;
                ic[unit].slot = (uint32_t)setae_instance_slot(inst, name);
                sp -= 2;
                DISPATCH();
            }
            const char *name = setae_code_name(code, arg);
            if (t == SETAE_T_CLASS) {
                SetaeValue dv = ((SetaeClass *)setae_to_ptr(obj))->dict;
                SetaeValue key = setae_str_new(vm->heap, name, strlen(name));
                setae_dict_set(setae_to_ptr(dv), key, val);
                vm->class_version++;
                sp -= 2;
            } else {
                setae_vm_raise(vm, "AttributeError", "'%s' object has no attribute '%s'",
                               setae_type_name(obj), name);
            }
            DISPATCH();
        }
        L_OP_MAKE_CLASS: {
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
                DISPATCH();
            }
            if (!setae_is_none(base) && setae_obj_type(base) != SETAE_T_CLASS) {
                setae_vm_raise(vm, "TypeError", "base must be a class, not '%s'",
                               setae_type_name(base));
                DISPATCH();
            }
            stack[sp++] = setae_class_new(vm->heap, cname, base, dict);
            DISPATCH();
        }
        L_OP_IMPORT: {
            if (arg >= vm->nmodules) {
                setae_vm_raise(vm, "RuntimeError", "bad module index %u", arg);
                DISPATCH();
            }
            if (vm->module_cache[arg] != 0) {
                stack[sp++] = vm->module_cache[arg];
                DISPATCH();
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
                setae_dict_set(setae_to_ptr(pm->dict), key, mod);
            }
            run_code(vm, mcode, NULL, 0, NULL, NULL, 0, 0, mod, NULL);
            if (vm->error) {
                DISPATCH();
            }
            stack[sp++] = mod;
            DISPATCH();
        }
        L_OP_IMPORT_MISSING:
            setae_vm_raise(vm, "ImportError", "No module named '%s'",
                           setae_code_name(code, arg));
            DISPATCH();
        L_OP_CALL_EX: {
            SetaeValue kwd = stack[sp - 1];
            SetaeList *pl = setae_to_ptr(stack[sp - 2]);
            SetaeValue callee = stack[sp - 3];
            SetaeValue r = call_value(vm, callee, pl->items, (int)pl->len, kwd);
            sp -= 3;
            stack[sp++] = r;
            DISPATCH();
        }
        L_OP_LIST_EXTEND: {
            SetaeValue itv = stack[sp - 1];
            int et = setae_obj_type(itv);
            if (et == SETAE_T_LIST) {
                SetaeList *src = setae_to_ptr(itv);
                for (uint32_t i = 0; i < src->len; i++) {
                    setae_list_push(setae_to_ptr(stack[sp - 2]), src->items[i]);
                }
            } else if (et == SETAE_T_TUPLE) {
                SetaeTuple *src = setae_to_ptr(itv);
                for (uint32_t i = 0; i < src->len; i++) {
                    setae_list_push(setae_to_ptr(stack[sp - 2]), src->items[i]);
                }
            } else if (et == SETAE_T_RANGE || et == SETAE_T_STR || et == SETAE_T_DICT) {
                SetaeValue iterv = setae_iter_new(vm->heap, itv);
                setae_vm_push_tmp(vm, iterv);
                SetaeValue elem;
                while (iter_next(vm, setae_to_ptr(iterv), &elem)) {
                    setae_list_push(setae_to_ptr(stack[sp - 2]), elem);
                }
                setae_vm_pop_tmp(vm);
            } else {
                setae_vm_raise(vm, "TypeError", "argument after * must be iterable, not %s",
                               setae_type_name(itv));
            }
            sp--;
            DISPATCH();
        }
        L_OP_DICT_MERGE: {
            SetaeValue srcv = stack[sp - 1];
            if (setae_obj_type(srcv) != SETAE_T_DICT) {
                setae_vm_raise(vm, "TypeError", "argument after ** must be a dict, not %s",
                               setae_type_name(srcv));
            } else {
                SetaeDict *src = setae_to_ptr(srcv);
                for (uint32_t i = 0; i < src->len; i++) {
                    setae_dict_set(setae_to_ptr(stack[sp - 2]), src->entries[i].key,
                             src->entries[i].value);
                }
            }
            sp--;
            DISPATCH();
        }
        L_OP_RAISE:
            raise_value(vm, stack[--sp]);
            DISPATCH();
        L_OP_RERAISE:
            raise_pending(vm, stack[--sp]);
            DISPATCH();
        L_OP_EXC_MATCH: {
            SetaeValue type = stack[--sp];
            int r = exc_matches(vm, stack[sp - 1], type);
            if (vm->error) {
                DISPATCH();
            }
            stack[sp++] = setae_bool(r);
            DISPATCH();
        }
        L_OP_UNPACK_SEQUENCE: {
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
                DISPATCH();
            }
            if (len < arg) {
                setae_vm_raise(
                    vm, "ValueError", "not enough values to unpack (expected %u, got %u)",
                    arg, len);
                DISPATCH();
            }
            if (len > arg) {
                setae_vm_raise(vm, "ValueError", "too many values to unpack (expected %u)",
                               arg);
                DISPATCH();
            }
            for (uint32_t i = arg; i-- > 0;) {
                stack[sp++] = items[i];
            }
            DISPATCH();
        }
        L_OP_BUILD_LIST: {
            int n = (int)arg;
            SetaeValue lv = setae_list_new(vm->heap, (uint32_t)n);
            SetaeList *l = setae_to_ptr(lv);
            for (int i = 0; i < n; i++) {
                setae_list_push(l, stack[sp - n + i]);
            }
            sp -= n;
            stack[sp++] = lv;
            DISPATCH();
        }
        L_OP_BUILD_DICT: {
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
                setae_dict_set(d, key, val);
            }
            sp -= 2 * n;
            stack[sp++] = dv;
            DISPATCH();
        }
        L_OP_BUILD_SET: {
            int n = (int)arg;
            SetaeValue sv = setae_set_new(vm->heap);
            SetaeSet *s = setae_to_ptr(sv);
            for (int i = 0; i < n; i++) {
                SetaeValue key = stack[sp - n + i];
                if (!hashable(key)) {
                    setae_vm_raise(vm, "TypeError", "unhashable type: '%s'",
                                   setae_type_name(key));
                    break;
                }
                setae_set_add(s, key);
            }
            sp -= n;
            stack[sp++] = sv;
            DISPATCH();
        }
        L_OP_BUILD_SET_CONST: {
            int n = (int)arg;
            SetaeValue fv = setae_set_new(vm->heap);
            SetaeSet *f = setae_to_ptr(fv);
            for (int i = 0; i < n; i++) {
                SetaeValue key = stack[sp - n + i];
                if (!hashable(key)) {
                    setae_vm_raise(vm, "TypeError", "unhashable type: '%s'",
                                   setae_type_name(key));
                    break;
                }
                setae_set_add(f, key);
            }
            setae_vm_push_tmp(vm, fv);
            SetaeValue sv = setae_set_new(vm->heap);
            SetaeSet *s = setae_to_ptr(sv);
            f = setae_to_ptr(fv);
            setae_set_presize(s, f->used);
            if (s->mask == f->mask) {
                memcpy(s->table, f->table,
                       ((size_t)f->mask + 1) * sizeof(SetaeSetEntry));
                s->fill = f->fill;
                s->used = f->used;
            } else {
                for (uint32_t i = 0; i <= f->mask; i++) {
                    if (f->table[i].state == SET_ACTIVE) {
                        setae_set_add(s, f->table[i].key);
                    }
                }
            }
            setae_vm_pop_tmp(vm);
            sp -= n;
            stack[sp++] = sv;
            DISPATCH();
        }
        L_OP_BUILD_SLICE: {
            SetaeValue step = stack[--sp];
            SetaeValue upper = stack[--sp];
            SetaeValue lower = stack[--sp];
            stack[sp++] = setae_slice_new(vm->heap, lower, upper, step);
            DISPATCH();
        }
        L_OP_SUBSCR: {
            SetaeValue idx = stack[--sp];
            SetaeValue obj = stack[--sp];
            stack[sp++] = subscript(vm, obj, idx);
            DISPATCH();
        }
        L_OP_STORE_SUBSCR: {
            SetaeValue idx = stack[--sp];
            SetaeValue obj = stack[--sp];
            SetaeValue val = stack[--sp];
            store_subscript(vm, obj, idx, val);
            DISPATCH();
        }
        L_OP_GET_ITER:
            stack[sp - 1] = setae_make_iter(vm, stack[sp - 1]);
            DISPATCH();
        L_OP_FOR_ITER: {
            SetaeValue out;
            int got = setae_iter_advance(vm, stack[sp - 1], &out);
            if (vm->error) {
                DISPATCH();
            }
            if (got) {
                stack[sp++] = out;
            } else {
                sp--;
                ip = arg * 2;
            }
            DISPATCH();
        }
        L_OP_CALL_METHOD: {
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
                                     f->ndefaults, 0, f->module, NULL);
                    }
                    sp -= n + 1;
                    stack[sp++] = r;
                    DISPATCH();
                }
            }
            const char *name = setae_code_name(code, arg >> 8);
            SetaeValue saved_kw = vm->cur_kwargs;
            vm->cur_kwargs = 0;
            SetaeValue r = call_method(vm, obj, name, argv, n, c);
            vm->cur_kwargs = saved_kw;
            sp -= n + 1;
            stack[sp++] = r;
            DISPATCH();
        }
        L_OP_CALL_METHOD_KW: {
            int n = (int)(arg & 0xff);
            SetaeValue kwargs = stack[--sp];
            SetaeValue *argv = &stack[sp - n];
            SetaeValue obj = stack[sp - n - 1];
            const char *name = setae_code_name(code, arg >> 8);
            SetaeValue kw =
                (setae_obj_type(kwargs) == SETAE_T_DICT &&
                 ((SetaeDict *)setae_to_ptr(kwargs))->len > 0)
                    ? kwargs
                    : 0;
            int ot = setae_obj_type(obj);
            SetaeValue r;
            if (ot == SETAE_T_INSTANCE || ot == SETAE_T_CLASS || ot == SETAE_T_MODULE) {
                SetaeValue m = load_attr(vm, obj, name);
                r = vm->error ? setae_none() : call_value(vm, m, argv, n, kw);
            } else {
                SetaeValue saved = vm->cur_kwargs;
                vm->cur_kwargs = kw;
                r = call_method(vm, obj, name, argv, n, &ic[unit]);
                vm->cur_kwargs = saved;
            }
            sp -= n + 1;
            stack[sp++] = r;
            DISPATCH();
        }
    L_OP_EXTENDED_ARG:
        ext = arg << 8;
        DISPATCH();

handle_error:
    if (!vm->interrupted) {
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
                goto slow_dispatch;
            }
        }
    }
    goto loop_done;

loop_done:
#undef DISPATCH

    vm->frames = fr.parent;
    if (gen != NULL) {
        gen->done = 1;
        gen->retval = result;
    } else {
        frame_release(vm, frame, frame_cap);
    }
    vm->depth--;
    return result;
}

static SetaeValue make_generator(SetaeVM *vm, const SetaeCode *code, SetaeValue *args, int nargs,
                                 const SetaeValue *captured, const SetaeValue *defaults,
                                 uint32_t ndefaults, SetaeValue kwargs, SetaeValue module) {
    uint32_t nlocals = setae_code_nlocals(code);
    uint32_t ncells = setae_code_ncells(code);
    uint32_t nfrees = setae_code_nfrees(code);
    uint32_t fixed = nlocals + ncells + nfrees;
    SetaeValue genv = setae_gen_new(vm->heap, code, module);
    SetaeGen *g = setae_to_ptr(genv);
    g->coroutine = setae_code_coroutine(code);
    g->fixed = fixed;
    g->frame_cap = fixed + STACK_MAX;
    g->frame = calloc(g->frame_cap, sizeof(SetaeValue));
    setae_vm_push_tmp(vm, genv);
    SetaeValue *locals = g->frame;
    SetaeValue *cellbase = g->frame + nlocals;
    if (!bind_args(vm, code, args, nargs, defaults, ndefaults, kwargs, locals)) {
        g->done = 1;
        setae_vm_pop_tmp(vm);
        return setae_none();
    }
    for (uint32_t i = 0; i < ncells; i++) {
        cellbase[i] = setae_cell_new(vm->heap);
    }
    for (uint32_t i = 0; i < nfrees; i++) {
        cellbase[ncells + i] = captured[i];
    }
    setae_vm_pop_tmp(vm);
    return genv;
}

static SetaeValue gen_resume(SetaeVM *vm, SetaeGen *g, SetaeValue sent, int *stopped) {
    if (g->done) {
        *stopped = 1;
        return setae_none();
    }
    SetaeValue r = run_code(vm, g->code, NULL, 0, NULL, NULL, 0, sent, g->module, g);
    if (g->done) {
        *stopped = 1;
        return setae_none();
    }
    *stopped = 0;
    return r;
}

int setae_gen_next(SetaeVM *vm, SetaeValue genv, SetaeValue sent, SetaeValue *out) {
    int stopped;
    SetaeValue v = gen_resume(vm, setae_to_ptr(genv), sent, &stopped);
    if (stopped) {
        return 0;
    }
    *out = v;
    return 1;
}

int setae_truthy(SetaeValue v) {
    return truthy(v);
}

SetaeValue setae_value_add(SetaeVM *vm, SetaeValue a, SetaeValue b) {
    return binary_op(vm, BIN_ADD, 0, a, b);
}

int setae_value_lt(SetaeVM *vm, SetaeValue a, SetaeValue b) {
    return setae_to_bool(compare(vm, CMP_LT, a, b));
}

static int iterop_next(SetaeVM *vm, SetaeIterOp *op, SetaeValue *out);

SetaeValue setae_make_iter(SetaeVM *vm, SetaeValue v) {
    int t = setae_obj_type(v);
    if (t == SETAE_T_GEN) {
        if (((SetaeGen *)setae_to_ptr(v))->coroutine) {
            setae_vm_raise(vm, "TypeError", "'coroutine' object is not iterable");
            return setae_none();
        }
        return v;
    }
    if (t == SETAE_T_ITEROP) {
        return v;
    }
    if (t == SETAE_T_LIST || t == SETAE_T_TUPLE || t == SETAE_T_DICT || t == SETAE_T_SET ||
        t == SETAE_T_STR || t == SETAE_T_RANGE) {
        return setae_iter_new(vm->heap, v);
    }
    setae_vm_raise(vm, "TypeError", "'%s' object is not iterable", setae_type_name(v));
    return setae_none();
}

int setae_iter_advance(SetaeVM *vm, SetaeValue it, SetaeValue *out) {
    int t = setae_obj_type(it);
    if (t == SETAE_T_ITER) {
        return iter_next(vm, setae_to_ptr(it), out);
    }
    if (t == SETAE_T_GEN) {
        if (((SetaeGen *)setae_to_ptr(it))->coroutine) {
            setae_vm_raise(vm, "TypeError", "'coroutine' object is not an iterator");
            return 0;
        }
        return setae_gen_next(vm, it, setae_none(), out);
    }
    if (t == SETAE_T_ITEROP) {
        return iterop_next(vm, setae_to_ptr(it), out);
    }
    setae_vm_raise(vm, "TypeError", "'%s' object is not an iterator", setae_type_name(it));
    return 0;
}

static int iterop_next(SetaeVM *vm, SetaeIterOp *op, SetaeValue *out) {
    SetaeList *srcs = setae_to_ptr(op->sources);
    switch (op->kind) {
    case ITEROP_MAP: {
        SetaeValue x;
        if (!setae_iter_advance(vm, srcs->items[0], &x) || vm->error) {
            return 0;
        }
        setae_vm_push_tmp(vm, x);
        *out = setae_call(vm, op->func, &x, 1);
        setae_vm_pop_tmp(vm);
        return vm->error ? 0 : 1;
    }
    case ITEROP_FILTER: {
        for (;;) {
            SetaeValue x;
            if (!setae_iter_advance(vm, srcs->items[0], &x) || vm->error) {
                return 0;
            }
            int keep;
            if (setae_is_none(op->func)) {
                keep = setae_truthy(x);
            } else {
                setae_vm_push_tmp(vm, x);
                SetaeValue r = setae_call(vm, op->func, &x, 1);
                setae_vm_pop_tmp(vm);
                if (vm->error) {
                    return 0;
                }
                keep = setae_truthy(r);
            }
            if (keep) {
                *out = x;
                return 1;
            }
        }
    }
    case ITEROP_ENUMERATE: {
        SetaeValue x;
        if (!setae_iter_advance(vm, srcs->items[0], &x) || vm->error) {
            return 0;
        }
        vm->gc_disabled++;
        SetaeValue pair[2] = {setae_from_int((int32_t)op->index), x};
        *out = setae_tuple_new(vm->heap, pair, 2);
        vm->gc_disabled--;
        op->index++;
        return 1;
    }
    case ITEROP_ZIP: {
        uint32_t n = srcs->len;
        if (n == 0) {
            return 0;
        }
        SetaeValue row[16];
        SetaeValue *tmp = n <= 16 ? row : malloc(n * sizeof(SetaeValue));
        vm->gc_disabled++;
        int ok = 1;
        for (uint32_t i = 0; i < n; i++) {
            if (!setae_iter_advance(vm, srcs->items[i], &tmp[i]) || vm->error) {
                ok = 0;
                break;
            }
        }
        if (ok) {
            *out = setae_tuple_new(vm->heap, tmp, n);
        }
        vm->gc_disabled--;
        if (tmp != row) {
            free(tmp);
        }
        return ok;
    }
    case ITEROP_REVERSED: {
        SetaeList *seq = setae_to_ptr(srcs->items[0]);
        if (op->index >= (int64_t)seq->len) {
            return 0;
        }
        op->index++;
        *out = seq->items[seq->len - (uint32_t)op->index];
        return 1;
    }
    }
    return 0;
}

SetaeValue setae_iter_collect(SetaeVM *vm, SetaeValue v) {
    SetaeValue itv = setae_make_iter(vm, v);
    if (vm->error) {
        return setae_none();
    }
    setae_vm_push_tmp(vm, itv);
    SetaeValue lst = setae_list_new(vm->heap, 0);
    setae_vm_push_tmp(vm, lst);
    SetaeValue out;
    while (setae_iter_advance(vm, itv, &out)) {
        if (vm->error) {
            break;
        }
        setae_list_push(setae_to_ptr(lst), out);
    }
    setae_vm_pop_tmp(vm);
    setae_vm_pop_tmp(vm);
    return lst;
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
    return run_code(vm, code, NULL, 0, NULL, NULL, 0, 0, 0, NULL);
}
