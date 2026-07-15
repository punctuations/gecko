#include "internal.h"

#include <math.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define STACK_MAX 1024
#define MAX_DEPTH 500

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
    free(vm->codes);
    free(vm->out);
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

void setae_vm_set_global(SetaeVM *vm, const char *name, SetaeValue v) {
    for (size_t i = 0; i < vm->nglobals; i++) {
        if (strcmp(vm->globals[i].name, name) == 0) {
            vm->globals[i].value = v;
            return;
        }
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
}

static int global_lookup(SetaeVM *vm, const char *name, SetaeValue *out) {
    for (size_t i = 0; i < vm->nglobals; i++) {
        if (strcmp(vm->globals[i].name, name) == 0) {
            *out = vm->globals[i].value;
            return 1;
        }
    }
    return 0;
}

int setae_vm_error(SetaeVM *vm) {
    return vm->error;
}

const char *setae_vm_error_msg(SetaeVM *vm) {
    return vm->errmsg;
}

void setae_vm_failf(SetaeVM *vm, const char *fmt, ...) {
    if (vm->error) {
        return;
    }
    vm->error = 1;
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(vm->errmsg, sizeof(vm->errmsg), fmt, ap);
    va_end(ap);
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
    return t != SETAE_T_LIST && t != SETAE_T_DICT;
}

static int64_t dict_find(const SetaeDict *d, SetaeValue key) {
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

static const char *bin_symbol(SetaeBinOp op) {
    switch (op) {
    case BIN_ADD:
        return "+";
    case BIN_SUB:
        return "-";
    case BIN_MUL:
        return "*";
    case BIN_DIV:
        return "/";
    case BIN_MOD:
        return "%";
    default:
        return "//";
    }
}

static SetaeValue binary_op(SetaeVM *vm, SetaeBinOp op, SetaeValue a, SetaeValue b) {
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
        setae_vm_failf(vm, "TypeError: unsupported operand type(s) for %s: '%s' and '%s'",
                       bin_symbol(op), setae_type_name(a), setae_type_name(b));
        return setae_none();
    }
    if (setae_is_int(a) && setae_is_int(b) && op != BIN_DIV) {
        int64_t x = setae_to_int(a);
        int64_t y = setae_to_int(b);
        if (op == BIN_MOD || op == BIN_FLOORDIV) {
            if (y == 0) {
                setae_vm_failf(vm, "ZeroDivisionError: integer division or modulo by zero");
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
            setae_vm_failf(vm, "ZeroDivisionError: division by zero");
            return setae_none();
        }
        return setae_from_float(x / y);
    case BIN_MOD: {
        if (y == 0.0) {
            setae_vm_failf(vm, "ZeroDivisionError: float modulo");
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
            setae_vm_failf(vm, "ZeroDivisionError: float floor division by zero");
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
    case SETAE_T_DICT:
        return dict_find(setae_to_ptr(container), x) >= 0;
    case SETAE_T_STR: {
        if (!setae_is_str(x)) {
            setae_vm_failf(
                vm, "TypeError: 'in <string>' requires string as left operand, not %s",
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
        setae_vm_failf(vm, "TypeError: argument of type '%s' is not iterable",
                       setae_type_name(container));
        return 0;
    }
}

static SetaeValue compare(SetaeVM *vm, SetaeCmpOp op, SetaeValue a, SetaeValue b) {
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
        setae_vm_failf(vm, "TypeError: comparison not supported between '%s' and '%s'",
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
    setae_vm_failf(vm, "TypeError: bad operand type for unary -: '%s'",
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
    setae_vm_failf(vm, "IndexError: string index out of range");
    return setae_none();
}

static SetaeValue subscript(SetaeVM *vm, SetaeValue obj, SetaeValue idx) {
    switch (setae_obj_type(obj)) {
    case SETAE_T_LIST: {
        SetaeList *l = setae_to_ptr(obj);
        if (!setae_is_int(idx)) {
            setae_vm_failf(vm, "TypeError: list indices must be integers, not %s",
                           setae_type_name(idx));
            return setae_none();
        }
        int64_t i = setae_to_int(idx);
        if (i < 0) {
            i += l->len;
        }
        if (i < 0 || i >= (int64_t)l->len) {
            setae_vm_failf(vm, "IndexError: list index out of range");
            return setae_none();
        }
        return l->items[i];
    }
    case SETAE_T_DICT: {
        SetaeDict *d = setae_to_ptr(obj);
        if (!hashable(idx)) {
            setae_vm_failf(vm, "TypeError: unhashable type: '%s'", setae_type_name(idx));
            return setae_none();
        }
        int64_t i = dict_find(d, idx);
        if (i < 0) {
            setae_vm_failf(vm, "KeyError");
            return setae_none();
        }
        return d->entries[i].value;
    }
    case SETAE_T_STR: {
        if (!setae_is_int(idx)) {
            setae_vm_failf(vm, "TypeError: string indices must be integers, not %s",
                           setae_type_name(idx));
            return setae_none();
        }
        int64_t i = setae_to_int(idx);
        int64_t n = (int64_t)setae_str_count(obj);
        if (i < 0) {
            i += n;
        }
        if (i < 0 || i >= n) {
            setae_vm_failf(vm, "IndexError: string index out of range");
            return setae_none();
        }
        return str_char_at(vm, obj, (size_t)i);
    }
    case SETAE_T_RANGE: {
        SetaeRange *r = setae_to_ptr(obj);
        if (!setae_is_int(idx)) {
            setae_vm_failf(vm, "TypeError: range indices must be integers, not %s",
                           setae_type_name(idx));
            return setae_none();
        }
        int64_t i = setae_to_int(idx);
        int64_t n = setae_range_len(r);
        if (i < 0) {
            i += n;
        }
        if (i < 0 || i >= n) {
            setae_vm_failf(vm, "IndexError: range object index out of range");
            return setae_none();
        }
        return from_i64(r->start + i * r->step);
    }
    default:
        setae_vm_failf(vm, "TypeError: '%s' object is not subscriptable",
                       setae_type_name(obj));
        return setae_none();
    }
}

static void store_subscript(SetaeVM *vm, SetaeValue obj, SetaeValue idx, SetaeValue val) {
    switch (setae_obj_type(obj)) {
    case SETAE_T_LIST: {
        SetaeList *l = setae_to_ptr(obj);
        if (!setae_is_int(idx)) {
            setae_vm_failf(vm, "TypeError: list indices must be integers, not %s",
                           setae_type_name(idx));
            return;
        }
        int64_t i = setae_to_int(idx);
        if (i < 0) {
            i += l->len;
        }
        if (i < 0 || i >= (int64_t)l->len) {
            setae_vm_failf(vm, "IndexError: list assignment index out of range");
            return;
        }
        l->items[i] = val;
        return;
    }
    case SETAE_T_DICT: {
        if (!hashable(idx)) {
            setae_vm_failf(vm, "TypeError: unhashable type: '%s'", setae_type_name(idx));
            return;
        }
        dict_set(setae_to_ptr(obj), idx, val);
        return;
    }
    default:
        setae_vm_failf(vm, "TypeError: '%s' object does not support item assignment",
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
                           int nargs, const SetaeValue *captured);

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
        if ((uint32_t)nargs != nparams) {
            setae_vm_failf(vm, "TypeError: %s() takes %u positional arguments but %d were given",
                           setae_code_fname(f->code), nparams, nargs);
            return setae_none();
        }
        return run_code(vm, f->code, args, nargs, f->cells);
    }
    setae_vm_failf(vm, "TypeError: '%s' object is not callable", setae_type_name(callee));
    return setae_none();
}

static SetaeValue call_method(SetaeVM *vm, SetaeValue obj, const char *name,
                              SetaeValue *args, int nargs) {
    int t = setae_obj_type(obj);
    if (t == SETAE_T_LIST) {
        SetaeList *l = setae_to_ptr(obj);
        if (strcmp(name, "append") == 0) {
            if (nargs != 1) {
                setae_vm_failf(vm, "TypeError: append() takes exactly one argument (%d given)",
                               nargs);
                return setae_none();
            }
            setae_list_push(l, args[0]);
            return setae_none();
        }
        if (strcmp(name, "pop") == 0) {
            if (nargs > 1) {
                setae_vm_failf(vm, "TypeError: pop() takes at most 1 argument (%d given)",
                               nargs);
                return setae_none();
            }
            if (l->len == 0) {
                setae_vm_failf(vm, "IndexError: pop from empty list");
                return setae_none();
            }
            int64_t i = l->len - 1;
            if (nargs == 1) {
                if (!setae_is_int(args[0])) {
                    setae_vm_failf(vm, "TypeError: '%s' object cannot be interpreted as an integer",
                                   setae_type_name(args[0]));
                    return setae_none();
                }
                i = setae_to_int(args[0]);
                if (i < 0) {
                    i += l->len;
                }
                if (i < 0 || i >= (int64_t)l->len) {
                    setae_vm_failf(vm, "IndexError: pop index out of range");
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
                setae_vm_failf(vm, "TypeError: get expected 1 or 2 arguments, got %d", nargs);
                return setae_none();
            }
            if (!hashable(args[0])) {
                setae_vm_failf(vm, "TypeError: unhashable type: '%s'",
                               setae_type_name(args[0]));
                return setae_none();
            }
            int64_t i = dict_find(d, args[0]);
            if (i >= 0) {
                return d->entries[i].value;
            }
            return nargs == 2 ? args[1] : setae_none();
        }
        if (strcmp(name, "keys") == 0 || strcmp(name, "values") == 0) {
            if (nargs != 0) {
                setae_vm_failf(vm, "TypeError: %s() takes no arguments (%d given)", name,
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
    setae_vm_failf(vm, "AttributeError: '%s' object has no attribute '%s'",
                   setae_type_name(obj), name);
    return setae_none();
}

static SetaeValue run_code(SetaeVM *vm, const SetaeCode *code, SetaeValue *args,
                           int nargs, const SetaeValue *captured) {
    if (vm->depth >= MAX_DEPTH) {
        setae_vm_failf(vm, "RecursionError: maximum recursion depth exceeded");
        return setae_none();
    }
    vm->depth++;

    const SetaeValue *consts = setae_code_consts(code);
    uint32_t ncode;
    const uint8_t *bytes = setae_code_bytes(code, &ncode);
    uint32_t nlocals = setae_code_nlocals(code);
    uint32_t ncells = setae_code_ncells(code);
    uint32_t nfrees = setae_code_nfrees(code);
    uint32_t fixed = nlocals + ncells + nfrees;

    SetaeValue *frame = calloc(fixed + STACK_MAX, sizeof(SetaeValue));
    SetaeValue *locals = frame;
    SetaeValue *cellbase = frame + nlocals;
    SetaeValue *stack = frame + fixed;
    for (int i = 0; i < nargs; i++) {
        locals[i] = args[i];
    }
    int sp = 0;

    SetaeFrame fr = {frame, fixed, 0, vm->frames};
    vm->frames = &fr;

    for (uint32_t i = 0; i < ncells; i++) {
        cellbase[i] = setae_cell_new(vm->heap);
    }
    for (uint32_t i = 0; i < nfrees; i++) {
        cellbase[ncells + i] = captured[i];
    }

    SetaeValue result = setae_none();
    uint32_t ip = 0;
    uint32_t ext = 0;
    while (ip < ncode && !vm->error) {
        fr.sp = sp;
        uint8_t op = bytes[ip];
        uint32_t arg = ext | bytes[ip + 1];
        ip += 2;
        if (op == OP_EXTENDED_ARG) {
            ext = arg << 8;
            continue;
        }
        ext = 0;
        if (sp >= STACK_MAX - 1) {
            setae_vm_failf(vm, "RuntimeError: value stack overflow");
            break;
        }
        switch (op) {
        case OP_LOAD_CONST:
            stack[sp++] = consts[arg];
            break;
        case OP_LOAD_NAME: {
            SetaeValue v;
            if (!global_lookup(vm, setae_code_name(code, arg), &v)) {
                setae_vm_failf(vm, "NameError: name '%s' is not defined",
                               setae_code_name(code, arg));
                break;
            }
            stack[sp++] = v;
            break;
        }
        case OP_STORE_NAME:
            setae_vm_set_global(vm, setae_code_name(code, arg), stack[--sp]);
            break;
        case OP_LOAD_LOCAL:
            if (locals[arg] == 0) {
                setae_vm_failf(
                    vm, "UnboundLocalError: local variable referenced before assignment");
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
            stack[sp++] = binary_op(vm, (SetaeBinOp)arg, a, b);
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
                setae_vm_failf(vm, "RuntimeError: bad code index %u", arg);
                break;
            }
            uint32_t nf = setae_code_nfrees(child);
            SetaeValue f = setae_func_new(vm->heap, child, &stack[sp - nf], nf);
            sp -= (int)nf;
            stack[sp++] = f;
            break;
        }
        case OP_LOAD_CLOSURE:
            stack[sp++] = cellbase[arg];
            break;
        case OP_LOAD_DEREF: {
            SetaeCell *cell = setae_to_ptr(cellbase[arg]);
            if (cell->value == 0) {
                setae_vm_failf(
                    vm, "UnboundLocalError: variable referenced before assignment");
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
                    setae_vm_failf(vm, "TypeError: unhashable type: '%s'",
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
            if (t != SETAE_T_LIST && t != SETAE_T_DICT && t != SETAE_T_STR &&
                t != SETAE_T_RANGE) {
                setae_vm_failf(vm, "TypeError: '%s' object is not iterable",
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
            const char *name = setae_code_name(code, arg >> 8);
            SetaeValue *argv = &stack[sp - n];
            SetaeValue obj = stack[sp - n - 1];
            SetaeValue r = call_method(vm, obj, name, argv, n);
            sp -= n + 1;
            stack[sp++] = r;
            break;
        }
        default:
            setae_vm_failf(vm, "RuntimeError: bad opcode %u", op);
            break;
        }
    }

    vm->frames = fr.parent;
    free(frame);
    vm->depth--;
    return result;
}

SetaeValue setae_vm_run(SetaeVM *vm, SetaeCode *code) {
    attach_code(vm, code);
    return run_code(vm, code, NULL, 0, NULL);
}
