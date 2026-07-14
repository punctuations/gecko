#include "internal.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void zeros(char *z, int k) {
    for (int i = 0; i < k; i++) {
        z[i] = '0';
    }
    z[k] = '\0';
}

/* Shortest digit string that reads back as d, laid out like CPython's repr:
   fixed notation for decimal exponents in [-4, 16), scientific outside. */
static void fmt_float(char *out, size_t cap, double d) {
    if (isnan(d)) {
        snprintf(out, cap, "nan");
        return;
    }
    if (isinf(d)) {
        snprintf(out, cap, d < 0 ? "-inf" : "inf");
        return;
    }

    char sci[40];
    for (int p = 1; p <= 17; p++) {
        snprintf(sci, sizeof(sci), "%.*e", p - 1, d);
        if (strtod(sci, NULL) == d) {
            break;
        }
    }

    const char *s = sci;
    const char *sign = "";
    if (*s == '-') {
        sign = "-";
        s++;
    }

    char digits[24];
    size_t n = 0;
    for (; *s != '\0' && *s != 'e'; s++) {
        if (*s != '.') {
            digits[n++] = *s;
        }
    }
    digits[n] = '\0';
    int e = atoi(s + 1);

    char pad[24];
    if (e < -4 || e >= 16) {
        if (n > 1) {
            snprintf(out, cap, "%s%c.%se%+03d", sign, digits[0], digits + 1, e);
        } else {
            snprintf(out, cap, "%s%ce%+03d", sign, digits[0], e);
        }
    } else if (e < 0) {
        zeros(pad, -e - 1);
        snprintf(out, cap, "%s0.%s%s", sign, pad, digits);
    } else if ((size_t)(e + 1) >= n) {
        zeros(pad, (int)((size_t)(e + 1) - n));
        snprintf(out, cap, "%s%s%s.0", sign, digits, pad);
    } else {
        snprintf(out, cap, "%s%.*s.%s", sign, e + 1, digits, digits + e + 1);
    }
}

static void out_str(SetaeVM *vm, const char *s) {
    setae_vm_append_output(vm, s, strlen(s));
}

static void repr_quoted(SetaeVM *vm, SetaeValue v) {
    out_str(vm, "'");
    const char *p = setae_str_data(v);
    size_t n = setae_str_len(v);
    for (size_t i = 0; i < n; i++) {
        switch (p[i]) {
        case '\n':
            out_str(vm, "\\n");
            break;
        case '\t':
            out_str(vm, "\\t");
            break;
        case '\r':
            out_str(vm, "\\r");
            break;
        case '\\':
            out_str(vm, "\\\\");
            break;
        case '\'':
            out_str(vm, "\\'");
            break;
        default:
            setae_vm_append_output(vm, &p[i], 1);
        }
    }
    out_str(vm, "'");
}

static void repr(SetaeVM *vm, SetaeValue v, int nested) {
    char buf[64];
    if (setae_is_int(v)) {
        int n = snprintf(buf, sizeof(buf), "%d", setae_to_int(v));
        setae_vm_append_output(vm, buf, (size_t)n);
        return;
    }
    if (setae_is_float(v)) {
        fmt_float(buf, sizeof(buf), setae_to_float(v));
        out_str(vm, buf);
        return;
    }
    if (setae_is_none(v)) {
        out_str(vm, "None");
        return;
    }
    if (setae_is_bool(v)) {
        out_str(vm, setae_to_bool(v) ? "True" : "False");
        return;
    }
    switch (setae_obj_type(v)) {
    case SETAE_T_STR:
        if (nested) {
            repr_quoted(vm, v);
        } else {
            setae_vm_append_output(vm, setae_str_data(v), setae_str_len(v));
        }
        return;
    case SETAE_T_LIST: {
        SetaeList *l = setae_to_ptr(v);
        if (l->obj.gc & 1) {
            out_str(vm, "[...]");
            return;
        }
        l->obj.gc |= 1;
        out_str(vm, "[");
        for (uint32_t i = 0; i < l->len; i++) {
            if (i > 0) {
                out_str(vm, ", ");
            }
            repr(vm, l->items[i], 1);
        }
        out_str(vm, "]");
        l->obj.gc &= ~1u;
        return;
    }
    case SETAE_T_DICT: {
        SetaeDict *d = setae_to_ptr(v);
        if (d->obj.gc & 1) {
            out_str(vm, "{...}");
            return;
        }
        d->obj.gc |= 1;
        out_str(vm, "{");
        for (uint32_t i = 0; i < d->len; i++) {
            if (i > 0) {
                out_str(vm, ", ");
            }
            repr(vm, d->entries[i].key, 1);
            out_str(vm, ": ");
            repr(vm, d->entries[i].value, 1);
        }
        out_str(vm, "}");
        d->obj.gc &= ~1u;
        return;
    }
    case SETAE_T_RANGE: {
        SetaeRange *r = setae_to_ptr(v);
        int n;
        if (r->step == 1) {
            n = snprintf(buf, sizeof(buf), "range(%lld, %lld)", (long long)r->start,
                         (long long)r->stop);
        } else {
            n = snprintf(buf, sizeof(buf), "range(%lld, %lld, %lld)", (long long)r->start,
                         (long long)r->stop, (long long)r->step);
        }
        setae_vm_append_output(vm, buf, (size_t)n);
        return;
    }
    case SETAE_T_FUNCTION: {
        SetaeFunc *f = setae_to_ptr(v);
        out_str(vm, "<function ");
        out_str(vm, setae_code_fname(f->code));
        out_str(vm, ">");
        return;
    }
    case SETAE_T_BUILTIN: {
        SetaeBuiltin *b = setae_to_ptr(v);
        out_str(vm, "<built-in function ");
        out_str(vm, b->name);
        out_str(vm, ">");
        return;
    }
    default:
        out_str(vm, "<object>");
    }
}

static SetaeValue builtin_print(SetaeVM *vm, SetaeValue *args, int nargs) {
    for (int i = 0; i < nargs; i++) {
        if (i > 0) {
            setae_vm_append_output(vm, " ", 1);
        }
        repr(vm, args[i], 0);
    }
    setae_vm_append_output(vm, "\n", 1);
    return setae_none();
}

static SetaeValue builtin_len(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1) {
        setae_vm_failf(vm, "TypeError: len() takes exactly one argument (%d given)",
                       nargs);
        return setae_none();
    }
    SetaeValue v = args[0];
    switch (setae_obj_type(v)) {
    case SETAE_T_STR:
        return setae_from_int((int32_t)setae_str_count(v));
    case SETAE_T_LIST:
        return setae_from_int((int32_t)((SetaeList *)setae_to_ptr(v))->len);
    case SETAE_T_DICT:
        return setae_from_int((int32_t)((SetaeDict *)setae_to_ptr(v))->len);
    case SETAE_T_RANGE: {
        int64_t n = setae_range_len(setae_to_ptr(v));
        if (n > INT32_MAX) {
            n = INT32_MAX;
        }
        return setae_from_int((int32_t)n);
    }
    default:
        setae_vm_failf(vm, "TypeError: object of type '%s' has no len()",
                       setae_type_name(v));
        return setae_none();
    }
}

static SetaeValue builtin_range(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs < 1 || nargs > 3) {
        setae_vm_failf(vm, "TypeError: range expected 1 to 3 arguments, got %d", nargs);
        return setae_none();
    }
    for (int i = 0; i < nargs; i++) {
        if (!setae_is_int(args[i])) {
            setae_vm_failf(vm, "TypeError: '%s' object cannot be interpreted as an integer",
                           setae_type_name(args[i]));
            return setae_none();
        }
    }
    int64_t start = 0;
    int64_t stop;
    int64_t step = 1;
    if (nargs == 1) {
        stop = setae_to_int(args[0]);
    } else {
        start = setae_to_int(args[0]);
        stop = setae_to_int(args[1]);
        if (nargs == 3) {
            step = setae_to_int(args[2]);
        }
    }
    if (step == 0) {
        setae_vm_failf(vm, "ValueError: range() arg 3 must not be zero");
        return setae_none();
    }
    return setae_range_new(setae_vm_heap(vm), start, stop, step);
}

void setae_vm_register_builtins(SetaeVM *vm) {
    SetaeHeap *h = setae_vm_heap(vm);
    setae_vm_set_global(vm, "print", setae_builtin_new(h, builtin_print, "print"));
    setae_vm_set_global(vm, "len", setae_builtin_new(h, builtin_len, "len"));
    setae_vm_set_global(vm, "range", setae_builtin_new(h, builtin_range, "range"));
}
