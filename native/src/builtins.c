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

static void display(GkVM *vm, GkValue v) {
    char buf[64];
    if (gk_is_str(v)) {
        gk_vm_append_output(vm, gk_str_data(v), gk_str_len(v));
    } else if (gk_is_int(v)) {
        int n = snprintf(buf, sizeof(buf), "%d", gk_to_int(v));
        gk_vm_append_output(vm, buf, (size_t)n);
    } else if (gk_is_float(v)) {
        fmt_float(buf, sizeof(buf), gk_to_float(v));
        gk_vm_append_output(vm, buf, strlen(buf));
    } else if (gk_is_none(v)) {
        gk_vm_append_output(vm, "None", 4);
    } else if (gk_is_bool(v)) {
        if (gk_to_bool(v)) {
            gk_vm_append_output(vm, "True", 4);
        } else {
            gk_vm_append_output(vm, "False", 5);
        }
    } else {
        gk_vm_append_output(vm, "<object>", 8);
    }
}

static GkValue builtin_print(GkVM *vm, GkValue *args, int nargs) {
    for (int i = 0; i < nargs; i++) {
        if (i > 0) {
            gk_vm_append_output(vm, " ", 1);
        }
        display(vm, args[i]);
    }
    gk_vm_append_output(vm, "\n", 1);
    return gk_none();
}

void gk_vm_register_builtins(GkVM *vm) {
    GkValue p = gk_builtin_new(gk_vm_heap(vm), builtin_print, "print");
    gk_vm_set_global(vm, "print", p);
}
