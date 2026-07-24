#include "internal.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int kwarg_get(SetaeVM *vm, const char *name, SetaeValue *out) {
    if (vm->cur_kwargs == 0) {
        return 0;
    }
    SetaeDict *d = setae_to_ptr(vm->cur_kwargs);
    size_t len = strlen(name);
    int64_t i;
    if (d->index != NULL) {
        i = setae_dict_index_get_cstr(d, name, len);
    } else {
        i = -1;
        for (uint32_t e = 0; e < d->len; e++) {
            SetaeValue k = d->entries[e].key;
            if (setae_is_str(k) && setae_str_len(k) == len &&
                memcmp(setae_str_data(k), name, len) == 0) {
                i = (int64_t)e;
                break;
            }
        }
    }
    if (i < 0) {
        return 0;
    }
    *out = d->entries[i].value;
    return 1;
}

static void zeros(char *z, int k) {
    for (int i = 0; i < k; i++) {
        z[i] = '0';
    }
    z[k] = '\0';
}

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
    if (setae_obj_type(v) == SETAE_T_BIGINT) {
        SetaeValue s = setae_bigint_to_str(vm->heap, v);
        setae_vm_append_output(vm, setae_str_data(s), setae_str_len(s));
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
    case SETAE_T_TUPLE: {
        SetaeTuple *t = setae_to_ptr(v);
        if (t->obj.gc & 1) {
            out_str(vm, "(...)");
            return;
        }
        t->obj.gc |= 1;
        out_str(vm, "(");
        for (uint32_t i = 0; i < t->len; i++) {
            if (i > 0) {
                out_str(vm, ", ");
            }
            repr(vm, t->items[i], 1);
        }
        if (t->len == 1) {
            out_str(vm, ",");
        }
        out_str(vm, ")");
        t->obj.gc &= ~1u;
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
    case SETAE_T_SET: {
        SetaeSet *s = setae_to_ptr(v);
        if (s->used == 0) {
            out_str(vm, s->frozen ? "frozenset()" : "set()");
            return;
        }
        if (s->obj.gc & 1) {
            out_str(vm, "{...}");
            return;
        }
        s->obj.gc |= 1;
        if (s->frozen) {
            out_str(vm, "frozenset(");
        }
        out_str(vm, "{");
        int first = 1;
        for (uint32_t i = 0; i <= s->mask; i++) {
            if (s->table[i].state != SET_ACTIVE) {
                continue;
            }
            if (!first) {
                out_str(vm, ", ");
            }
            first = 0;
            repr(vm, s->table[i].key, 1);
        }
        out_str(vm, "}");
        if (s->frozen) {
            out_str(vm, ")");
        }
        s->obj.gc &= ~1u;
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
    case SETAE_T_ARRAY:
        setae_array_repr(vm, v);
        return;
    case SETAE_T_FUNCTION: {
        SetaeFunc *f = setae_to_ptr(v);
        out_str(vm, "<function ");
        out_str(vm, setae_code_fname(f->code));
        out_str(vm, ">");
        return;
    }
    case SETAE_T_BUILTIN: {
        SetaeBuiltin *b = setae_to_ptr(v);
        if (b->is_type) {
            out_str(vm, "<class '");
            out_str(vm, b->name);
            out_str(vm, "'>");
        } else {
            out_str(vm, "<built-in function ");
            out_str(vm, b->name);
            out_str(vm, ">");
        }
        return;
    }
    case SETAE_T_CLASS: {
        SetaeClass *c = setae_to_ptr(v);
        out_str(vm, "<class '");
        setae_vm_append_output(vm, setae_str_data(c->name), setae_str_len(c->name));
        out_str(vm, "'>");
        return;
    }
    case SETAE_T_INSTANCE: {
        SetaeInstance *i = setae_to_ptr(v);
        SetaeClass *c = setae_to_ptr(i->cls);
        out_str(vm, "<");
        setae_vm_append_output(vm, setae_str_data(c->name), setae_str_len(c->name));
        out_str(vm, " object>");
        return;
    }
    case SETAE_T_BOUND: {
        SetaeBound *b = setae_to_ptr(v);
        SetaeFunc *f = setae_to_ptr(b->func);
        out_str(vm, "<bound method ");
        out_str(vm, setae_code_fname(f->code));
        out_str(vm, ">");
        return;
    }
    case SETAE_T_MODULE: {
        SetaeModule *m = setae_to_ptr(v);
        out_str(vm, "<module '");
        setae_vm_append_output(vm, setae_str_data(m->name), setae_str_len(m->name));
        out_str(vm, "'>");
        return;
    }
    case SETAE_T_EXCTYPE: {
        SetaeExcType *t = setae_to_ptr(v);
        out_str(vm, "<class '");
        out_str(vm, t->name);
        out_str(vm, "'>");
        return;
    }
    case SETAE_T_EXC: {
        SetaeExc *e = setae_to_ptr(v);
        if (!nested) {
            if (!setae_is_none(e->message)) {
                repr(vm, e->message, 0);
            }
            return;
        }
        out_str(vm, e->kind);
        out_str(vm, "(");
        if (!setae_is_none(e->message)) {
            repr(vm, e->message, 1);
        }
        out_str(vm, ")");
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

SetaeValue setae_format_value(SetaeVM *vm, SetaeValue v, int repr_mode) {
    char *saved = vm->out;
    size_t saved_len = vm->out_len;
    size_t saved_cap = vm->out_cap;
    vm->out = NULL;
    vm->out_len = 0;
    vm->out_cap = 0;
    repr(vm, v, repr_mode ? 1 : 0);
    SetaeValue s = setae_str_new(vm->heap, vm->out ? vm->out : "", vm->out_len);
    free(vm->out);
    vm->out = saved;
    vm->out_len = saved_len;
    vm->out_cap = saved_cap;
    return s;
}

static SetaeValue builtin_chr(SetaeVM *vm, SetaeValue *args, int nargs);

typedef struct {
    char fill;
    char align;
    char sign;
    int alt;
    long width;
    char grouping;
    int has_prec;
    long prec;
    char type;
} FmtSpec;

static void parse_spec(const char *s, size_t n, FmtSpec *f) {
    f->fill = ' ';
    f->align = 0;
    f->sign = '-';
    f->alt = 0;
    f->width = 0;
    f->grouping = 0;
    f->has_prec = 0;
    f->prec = 0;
    f->type = 0;
    size_t i = 0;
    if (n >= 2 && (s[1] == '<' || s[1] == '>' || s[1] == '^' || s[1] == '=')) {
        f->fill = s[0];
        f->align = s[1];
        i = 2;
    } else if (n >= 1 && (s[0] == '<' || s[0] == '>' || s[0] == '^' || s[0] == '=')) {
        f->align = s[0];
        i = 1;
    }
    if (i < n && (s[i] == '+' || s[i] == '-' || s[i] == ' ')) {
        f->sign = s[i++];
    }
    if (i < n && s[i] == '#') {
        f->alt = 1;
        i++;
    }
    if (i < n && s[i] == '0') {
        if (!f->align) {
            f->align = '=';
            f->fill = '0';
        }
        i++;
    }
    while (i < n && s[i] >= '0' && s[i] <= '9') {
        f->width = f->width * 10 + (s[i++] - '0');
    }
    if (i < n && (s[i] == ',' || s[i] == '_')) {
        f->grouping = s[i++];
    }
    if (i < n && s[i] == '.') {
        i++;
        f->has_prec = 1;
        while (i < n && s[i] >= '0' && s[i] <= '9') {
            f->prec = f->prec * 10 + (s[i++] - '0');
        }
    }
    if (i < n) {
        f->type = s[i];
    }
}

static size_t group_digits(char *dst, const char *digits, size_t len, char sep) {
    size_t out = 0;
    size_t first = len % 3 == 0 ? 3 : len % 3;
    for (size_t i = 0; i < len; i++) {
        if (i == first && i != 0) {
            dst[out++] = sep;
        } else if (i > first && (i - first) % 3 == 0) {
            dst[out++] = sep;
        }
        dst[out++] = digits[i];
    }
    return out;
}

static SetaeValue apply_field(SetaeVM *vm, const char *sign, const char *digits, size_t dlen,
                              const FmtSpec *f, char default_align) {
    char align = f->align ? f->align : default_align;
    size_t slen = strlen(sign);
    long content = (long)(slen + dlen);
    size_t need = slen + dlen + (f->width > 0 ? (size_t)f->width : 0) + 8;
    char stackbuf[600];
    char *buf = need <= sizeof(stackbuf) ? stackbuf : malloc(need);
    size_t o = 0;
    long pad = f->width > content ? f->width - content : 0;
    if (pad == 0) {
        memcpy(buf + o, sign, slen);
        o += slen;
        memcpy(buf + o, digits, dlen);
        o += dlen;
    } else if (align == '<') {
        memcpy(buf + o, sign, slen);
        o += slen;
        memcpy(buf + o, digits, dlen);
        o += dlen;
        for (long k = 0; k < pad; k++) {
            buf[o++] = f->fill;
        }
    } else if (align == '^') {
        long left = pad / 2;
        for (long k = 0; k < left; k++) {
            buf[o++] = f->fill;
        }
        memcpy(buf + o, sign, slen);
        o += slen;
        memcpy(buf + o, digits, dlen);
        o += dlen;
        for (long k = 0; k < pad - left; k++) {
            buf[o++] = f->fill;
        }
    } else if (align == '=') {
        memcpy(buf + o, sign, slen);
        o += slen;
        for (long k = 0; k < pad; k++) {
            buf[o++] = f->fill;
        }
        memcpy(buf + o, digits, dlen);
        o += dlen;
    } else {
        for (long k = 0; k < pad; k++) {
            buf[o++] = f->fill;
        }
        memcpy(buf + o, sign, slen);
        o += slen;
        memcpy(buf + o, digits, dlen);
        o += dlen;
    }
    SetaeValue r = setae_str_new(vm->heap, buf, o);
    if (buf != stackbuf) {
        free(buf);
    }
    return r;
}

static const char *sign_str(int negative, char signspec) {
    if (negative) {
        return "-";
    }
    if (signspec == '+') {
        return "+";
    }
    if (signspec == ' ') {
        return " ";
    }
    return "";
}

SetaeValue setae_format_spec(SetaeVM *vm, SetaeValue v, SetaeValue specv, int conv) {
    FmtSpec f;
    parse_spec(setae_str_data(specv), setae_str_len(specv), &f);
    char type = f.type;

    int is_stringish = conv != 0 || type == 's' ||
                       (type == 0 && setae_obj_type(v) == SETAE_T_STR);
    if (is_stringish) {
        SetaeValue sv = conv == 1 ? setae_format_value(vm, v, 1)
                        : setae_obj_type(v) == SETAE_T_STR
                            ? v
                            : setae_format_value(vm, v, 0);
        if (vm->error) {
            return setae_none();
        }
        size_t n = setae_str_len(sv);
        const char *p = setae_str_data(sv);
        if (f.has_prec && (size_t)f.prec < setae_str_count(sv)) {
            size_t cnt = 0, bi = 0;
            while (bi < n && cnt < (size_t)f.prec) {
                bi++;
                while (bi < n && ((unsigned char)p[bi] & 0xc0) == 0x80) {
                    bi++;
                }
                cnt++;
            }
            n = bi;
        }
        char *big = malloc(n + (f.width > 0 ? (size_t)f.width : 0) + 8);
        FmtSpec sf = f;
        char align = f.align ? f.align : '<';
        sf.align = align;
        size_t cnt = 0;
        for (size_t i = 0; i < n; i++) {
            if (((unsigned char)p[i] & 0xc0) != 0x80) {
                cnt++;
            }
        }
        size_t o = 0;
        long pad = f.width > (long)cnt ? f.width - (long)cnt : 0;
        long left = align == '>' ? pad : align == '^' ? pad / 2 : 0;
        long right = pad - left;
        for (long k = 0; k < left; k++) {
            big[o++] = f.fill;
        }
        memcpy(big + o, p, n);
        o += n;
        for (long k = 0; k < right; k++) {
            big[o++] = f.fill;
        }
        SetaeValue r = setae_str_new(vm->heap, big, o);
        free(big);
        return r;
    }

    if (type == 'c') {
        SetaeValue arg = v;
        return builtin_chr(vm, &arg, 1);
    }

    int is_int = setae_is_integer(v);
    int int_type = type == 'd' || type == 'b' || type == 'o' || type == 'x' || type == 'X';
    if (setae_obj_type(v) == SETAE_T_BIGINT && (type == 'd' || type == 0)) {
        size_t dl;
        char *dec = setae_bigint_decimal(v, &dl);
        int neg = dec[0] == '-';
        char *digs = dec + (neg ? 1 : 0);
        size_t diglen = dl - (neg ? 1 : 0);
        char *grouped = malloc(diglen + diglen / 3 + 4);
        size_t glen;
        if (f.grouping) {
            glen = group_digits(grouped, digs, diglen, f.grouping);
        } else {
            memcpy(grouped, digs, diglen);
            glen = diglen;
        }
        SetaeValue r = apply_field(vm, sign_str(neg, f.sign), grouped, glen, &f, '>');
        free(grouped);
        free(dec);
        return r;
    }
    if (setae_obj_type(v) == SETAE_T_BIGINT) {
        int64_t x64;
        if (!setae_int_fits_i64(v, &x64)) {
            SetaeValue dv = setae_bigint_to_str(vm->heap, v);
            return apply_field(vm, "", setae_str_data(dv), setae_str_len(dv), &f, '>');
        }
    }
    if (is_int && (int_type || type == 0)) {
        int64_t x;
        if (setae_obj_type(v) == SETAE_T_BIGINT) {
            setae_int_fits_i64(v, &x);
        } else if (setae_is_bool(v)) {
            x = setae_to_bool(v) ? 1 : 0;
        } else {
            x = setae_to_int(v);
        }
        int neg = x < 0;
        uint64_t u = neg ? (uint64_t)(-(x + 1)) + 1 : (uint64_t)x;
        int base = type == 'b' ? 2 : type == 'o' ? 8 : (type == 'x' || type == 'X') ? 16 : 10;
        char raw[70];
        int ri = 0;
        if (u == 0) {
            raw[ri++] = '0';
        }
        while (u > 0) {
            int d = (int)(u % (uint64_t)base);
            char c = (char)(d < 10 ? '0' + d : (type == 'X' ? 'A' : 'a') + d - 10);
            raw[ri++] = c;
            u /= (uint64_t)base;
        }
        char digits[140];
        int di = 0;
        for (int k = ri - 1; k >= 0; k--) {
            digits[di++] = raw[k];
        }
        char grouped[200];
        size_t glen = (size_t)di;
        if (f.grouping && base == 10) {
            glen = group_digits(grouped, digits, (size_t)di, f.grouping);
        } else {
            memcpy(grouped, digits, (size_t)di);
        }
        char pre[8];
        int pi = 0;
        if (f.alt && base != 10) {
            pre[pi++] = '0';
            pre[pi++] = type == 'X' ? 'X' : type == 'b' ? 'b' : type == 'o' ? 'o' : 'x';
        }
        char sd[210];
        int sdi = 0;
        for (int k = 0; k < pi; k++) {
            sd[sdi++] = pre[k];
        }
        memcpy(sd + sdi, grouped, glen);
        sdi += (int)glen;
        return apply_field(vm, sign_str(neg, f.sign), sd, (size_t)sdi, &f, '>');
    }

    double d = setae_is_float(v) ? setae_to_float(v)
               : setae_is_bool(v) ? (setae_to_bool(v) ? 1.0 : 0.0)
                                   : (double)setae_to_int(v);
    int neg = signbit(d);
    double ad = neg ? -d : d;
    int percent = type == '%';
    char num[128];
    if (type == 0 && !f.has_prec) {
        fmt_float(num, sizeof(num), ad);
    } else {
        char ct = percent ? 'f' : type;
        if (percent) {
            ad *= 100.0;
        }
        if (ct == 0 || ct == 'n') {
            ct = 'g';
        }
        int prec = f.has_prec ? (int)f.prec : 6;
        char fmt[16];
        snprintf(fmt, sizeof(fmt), "%%.%d%c", prec, ct == 'F' ? 'f' : ct);
        snprintf(num, sizeof(num), fmt, ad);
    }
    char intpart[100];
    char rest[100];
    size_t iplen;
    if (f.grouping) {
        char *dot = strchr(num, '.');
        char *epos = strpbrk(num, "eE");
        size_t ilen = dot ? (size_t)(dot - num) : (epos ? (size_t)(epos - num) : strlen(num));
        iplen = group_digits(intpart, num, ilen, f.grouping);
        strcpy(rest, num + ilen);
    } else {
        iplen = strlen(num);
        memcpy(intpart, num, iplen);
        rest[0] = '\0';
    }
    char full[220];
    size_t fl = 0;
    memcpy(full, intpart, iplen);
    fl += iplen;
    size_t rl = strlen(rest);
    memcpy(full + fl, rest, rl);
    fl += rl;
    if (percent) {
        full[fl++] = '%';
    }
    return apply_field(vm, sign_str(neg, f.sign), full, fl, &f, '>');
}

static SetaeValue builtin_len(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "len() takes exactly one argument (%d given)",
                       nargs);
        return setae_none();
    }
    SetaeValue v = args[0];
    switch (setae_obj_type(v)) {
    case SETAE_T_STR:
        return setae_from_int((int32_t)setae_str_count(v));
    case SETAE_T_LIST:
        return setae_from_int((int32_t)((SetaeList *)setae_to_ptr(v))->len);
    case SETAE_T_TUPLE:
        return setae_from_int((int32_t)((SetaeTuple *)setae_to_ptr(v))->len);
    case SETAE_T_DICT:
        return setae_from_int((int32_t)((SetaeDict *)setae_to_ptr(v))->len);
    case SETAE_T_SET:
        return setae_from_int((int32_t)((SetaeSet *)setae_to_ptr(v))->used);
    case SETAE_T_RANGE: {
        int64_t n = setae_range_len(setae_to_ptr(v));
        if (n > INT32_MAX) {
            n = INT32_MAX;
        }
        return setae_from_int((int32_t)n);
    }
    case SETAE_T_ARRAY:
        return setae_from_int((int32_t)((SetaeArray *)setae_to_ptr(v))->len);
    default:
        setae_vm_raise(vm, "TypeError", "object of type '%s' has no len()",
                       setae_type_name(v));
        return setae_none();
    }
}

static SetaeValue builtin_range(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs < 1 || nargs > 3) {
        setae_vm_raise(vm, "TypeError", "range expected 1 to 3 arguments, got %d", nargs);
        return setae_none();
    }
    for (int i = 0; i < nargs; i++) {
        if (!setae_is_int(args[i])) {
            setae_vm_raise(vm, "TypeError", "'%s' object cannot be interpreted as an integer",
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
        setae_vm_raise(vm, "ValueError", "range() arg 3 must not be zero");
        return setae_none();
    }
    return setae_range_new(setae_vm_heap(vm), start, stop, step);
}

static const char *const EXC_KINDS[] = {
    "Exception",         "TypeError",     "ValueError",        "KeyError",
    "IndexError",        "NameError",     "UnboundLocalError", "ZeroDivisionError",
    "RuntimeError",      "RecursionError", "AttributeError", "MemoryError",
    "SandboxError",      "ImportError",   "AssertionError",    "TimeoutError",
    "StopIteration",     "NotImplementedError",
};

static SetaeValue builtin_sandbox_run(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (vm->sandbox_hook == NULL) {
        setae_vm_raise(vm, "RuntimeError", "sandbox is not available in this runtime");
        return setae_none();
    }
    if (nargs < 1 || nargs > 4 || !setae_is_str(args[0])) {
        setae_vm_raise(vm, "TypeError",
                       "sandbox.run(source, steps, memory, millis) needs a source string");
        return setae_none();
    }
    uint64_t steps = 0;
    size_t mem = 0;
    uint64_t millis = 0;
    if (nargs >= 2) {
        if (!setae_is_int(args[1])) {
            setae_vm_raise(vm, "TypeError", "sandbox.run steps must be an int");
            return setae_none();
        }
        steps = (uint64_t)(uint32_t)setae_to_int(args[1]);
    }
    if (nargs >= 3) {
        if (!setae_is_int(args[2])) {
            setae_vm_raise(vm, "TypeError", "sandbox.run memory must be an int");
            return setae_none();
        }
        mem = (size_t)(uint32_t)setae_to_int(args[2]);
    }
    if (nargs >= 4) {
        if (!setae_is_int(args[3])) {
            setae_vm_raise(vm, "TypeError", "sandbox.run millis must be an int");
            return setae_none();
        }
        millis = (uint64_t)(uint32_t)setae_to_int(args[3]);
    }
    return vm->sandbox_hook(vm, setae_str_data(args[0]), setae_str_len(args[0]), steps,
                            mem, millis);
}

static SetaeValue builtin_next(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs < 1 || nargs > 2) {
        setae_vm_raise(vm, "TypeError", "next() takes 1 or 2 arguments (%d given)", nargs);
        return setae_none();
    }
    SetaeValue out;
    if (setae_iter_advance(vm, args[0], &out)) {
        return out;
    }
    if (vm->error) {
        return setae_none();
    }
    if (nargs == 2) {
        return args[1];
    }
    setae_vm_raise(vm, "StopIteration", "");
    return setae_none();
}

static SetaeValue builtin_type(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "type() takes 1 argument (%d given)", nargs);
        return setae_none();
    }
    SetaeValue v = args[0];
    if (setae_obj_type(v) == SETAE_T_INSTANCE) {
        return ((SetaeInstance *)setae_to_ptr(v))->cls;
    }
    const char *name = setae_type_name(v);
    for (size_t i = 0; i < vm->nbuiltins; i++) {
        if (strcmp(vm->builtins[i].name, name) == 0) {
            return vm->builtins[i].value;
        }
    }
    setae_vm_raise(vm, "TypeError", "type() is not available for '%s'", name);
    return setae_none();
}

static const char *const TYPE_NAMES[] = {
    "NoneType",
};

static SetaeValue builtin_str(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs == 0) {
        return setae_str_new(vm->heap, "", 0);
    }
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "str() takes at most 1 argument (%d given)", nargs);
        return setae_none();
    }
    return setae_format_value(vm, args[0], 0);
}

static SetaeValue builtin_bool(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs == 0) {
        return setae_bool(0);
    }
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "bool() takes at most 1 argument (%d given)", nargs);
        return setae_none();
    }
    return setae_bool(setae_truthy(args[0]));
}

static SetaeValue builtin_int(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs == 0) {
        return setae_from_int(0);
    }
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "int() takes at most 1 argument");
        return setae_none();
    }
    SetaeValue v = args[0];
    if (setae_is_bool(v)) {
        return setae_from_int(setae_to_bool(v) ? 1 : 0);
    }
    if (setae_is_int(v) || setae_obj_type(v) == SETAE_T_BIGINT) {
        return v;
    }
    if (setae_is_float(v)) {
        double d = setae_to_float(v);
        double t = d < 0 ? ceil(d) : floor(d);
        if (t >= -2147483648.0 && t <= 2147483647.0) {
            return setae_from_int((int32_t)t);
        }
        char tmp[64];
        snprintf(tmp, sizeof(tmp), "%.0f", t);
        return setae_int_from_decimal(vm->heap, tmp, strlen(tmp), 0);
    }
    if (setae_obj_type(v) == SETAE_T_STR) {
        size_t n = setae_str_len(v);
        const char *sp = setae_str_data(v);
        while (n > 0 && (*sp == ' ' || *sp == '\t' || *sp == '\n' || *sp == '\r')) {
            sp++;
            n--;
        }
        while (n > 0 && (sp[n - 1] == ' ' || sp[n - 1] == '\t' || sp[n - 1] == '\n' ||
                         sp[n - 1] == '\r')) {
            n--;
        }
        size_t start = 0;
        int neg = 0;
        if (n > 0 && (sp[0] == '-' || sp[0] == '+')) {
            neg = sp[0] == '-';
            start = 1;
        }
        if (n == start) {
            setae_vm_raise(vm, "ValueError", "invalid literal for int() with base 10");
            return setae_none();
        }
        for (size_t i = start; i < n; i++) {
            if (sp[i] < '0' || sp[i] > '9') {
                setae_vm_raise(vm, "ValueError", "invalid literal for int() with base 10");
                return setae_none();
            }
        }
        return setae_int_from_decimal(vm->heap, sp + start, n - start, neg);
    }
    setae_vm_raise(vm, "TypeError", "int() argument must be a string or a number, not '%s'",
                   setae_type_name(v));
    return setae_none();
}

static SetaeValue builtin_float(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs == 0) {
        return setae_from_float(0.0);
    }
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "float() takes at most 1 argument");
        return setae_none();
    }
    SetaeValue v = args[0];
    if (setae_is_bool(v)) {
        return setae_from_float(setae_to_bool(v) ? 1.0 : 0.0);
    }
    if (setae_is_int(v) || setae_obj_type(v) == SETAE_T_BIGINT) {
        return setae_from_float(setae_int_to_double(v));
    }
    if (setae_is_float(v)) {
        return v;
    }
    if (setae_obj_type(v) == SETAE_T_STR) {
        size_t n = setae_str_len(v);
        char buf[64];
        if (n >= sizeof(buf)) {
            setae_vm_raise(vm, "ValueError", "could not convert string to float");
            return setae_none();
        }
        memcpy(buf, setae_str_data(v), n);
        buf[n] = '\0';
        char *end;
        double r = strtod(buf, &end);
        while (*end == ' ' || *end == '\t' || *end == '\n' || *end == '\r') {
            end++;
        }
        if (end == buf || *end != '\0') {
            setae_vm_raise(vm, "ValueError", "could not convert string to float: '%s'", buf);
            return setae_none();
        }
        return setae_from_float(r);
    }
    setae_vm_raise(vm, "TypeError", "float() argument must be a string or a number");
    return setae_none();
}

static SetaeValue builtin_list(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs == 0) {
        return setae_list_new(vm->heap, 0);
    }
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "list() takes at most 1 argument");
        return setae_none();
    }
    return setae_iter_collect(vm, args[0]);
}

static SetaeValue builtin_tuple(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs == 0) {
        return setae_tuple_new(vm->heap, NULL, 0);
    }
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "tuple() takes at most 1 argument");
        return setae_none();
    }
    SetaeValue lst = setae_iter_collect(vm, args[0]);
    if (vm->error) {
        return setae_none();
    }
    setae_vm_push_tmp(vm, lst);
    SetaeList *l = setae_to_ptr(lst);
    SetaeValue t = setae_tuple_new(vm->heap, l->items, l->len);
    setae_vm_pop_tmp(vm);
    return t;
}

static SetaeValue builtin_dict(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs > 1) {
        setae_vm_raise(vm, "TypeError", "dict expected at most 1 argument, got %d", nargs);
        return setae_none();
    }
    SetaeValue dv = setae_dict_new(vm->heap);
    setae_vm_push_tmp(vm, dv);
    SetaeDict *d = setae_to_ptr(dv);
    if (nargs == 1) {
        SetaeValue src = args[0];
        if (setae_obj_type(src) == SETAE_T_DICT) {
            SetaeDict *sd = setae_to_ptr(src);
            for (uint32_t i = 0; i < sd->len; i++) {
                setae_dict_set(d, sd->entries[i].key, sd->entries[i].value);
            }
        } else {
            SetaeValue it = setae_make_iter(vm, src);
            if (vm->error) {
                setae_vm_pop_tmp(vm);
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
                    setae_vm_raise(vm, "ValueError",
                                   "dictionary update sequence element has length %u; 2 is "
                                   "required",
                                   pll->len);
                    break;
                }
                setae_dict_set(d, pll->items[0], pll->items[1]);
            }
            setae_vm_pop_tmp(vm);
            if (vm->error) {
                setae_vm_pop_tmp(vm);
                return setae_none();
            }
        }
    }
    if (vm->cur_kwargs != 0) {
        SetaeDict *kw = setae_to_ptr(vm->cur_kwargs);
        for (uint32_t i = 0; i < kw->len; i++) {
            setae_dict_set(d, kw->entries[i].key, kw->entries[i].value);
        }
    }
    setae_vm_pop_tmp(vm);
    return dv;
}

static SetaeValue builtin_set(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs > 1) {
        setae_vm_raise(vm, "TypeError", "set expected at most 1 argument, got %d", nargs);
        return setae_none();
    }
    SetaeValue sv = setae_set_new(vm->heap);
    if (nargs == 1) {
        setae_vm_push_tmp(vm, sv);
        int at = setae_obj_type(args[0]);
        if (at == SETAE_T_SET) {
            setae_set_presize(setae_to_ptr(sv), ((SetaeSet *)setae_to_ptr(args[0]))->used);
        } else if (at == SETAE_T_DICT) {
            setae_set_presize(setae_to_ptr(sv), ((SetaeDict *)setae_to_ptr(args[0]))->len);
        }
        SetaeValue it = setae_make_iter(vm, args[0]);
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
        if (vm->error) {
            return setae_none();
        }
    }
    return sv;
}

static SetaeValue builtin_frozenset(SetaeVM *vm, SetaeValue *args, int nargs) {
    SetaeValue sv = builtin_set(vm, args, nargs);
    if (!vm->error) {
        ((SetaeSet *)setae_to_ptr(sv))->frozen = 1;
    }
    return sv;
}

static SetaeValue builtin_sum(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs < 1 || nargs > 2) {
        setae_vm_raise(vm, "TypeError", "sum() takes 1 or 2 arguments");
        return setae_none();
    }
    SetaeValue lst = setae_iter_collect(vm, args[0]);
    if (vm->error) {
        return setae_none();
    }
    setae_vm_push_tmp(vm, lst);
    SetaeList *l = setae_to_ptr(lst);
    SetaeValue acc = nargs == 2 ? args[1] : setae_from_int(0);
    for (uint32_t i = 0; i < l->len; i++) {
        acc = setae_value_add(vm, acc, l->items[i]);
        if (vm->error) {
            setae_vm_pop_tmp(vm);
            return setae_none();
        }
    }
    setae_vm_pop_tmp(vm);
    return acc;
}

static SetaeValue apply_key(SetaeVM *vm, SetaeValue keyfn, SetaeValue v) {
    if (setae_is_none(keyfn)) {
        return v;
    }
    return setae_call(vm, keyfn, &v, 1);
}

static SetaeValue min_max(SetaeVM *vm, SetaeValue *args, int nargs, int want_max) {
    SetaeValue keyfn = setae_none();
    kwarg_get(vm, "key", &keyfn);
    SetaeValue defval;
    int has_def = kwarg_get(vm, "default", &defval);

    SetaeValue *items;
    uint32_t n;
    SetaeValue lst = 0;
    if (nargs == 1) {
        lst = setae_iter_collect(vm, args[0]);
        if (vm->error) {
            return setae_none();
        }
        setae_vm_push_tmp(vm, lst);
        SetaeList *l = setae_to_ptr(lst);
        items = l->items;
        n = l->len;
    } else if (nargs >= 2) {
        items = args;
        n = (uint32_t)nargs;
    } else {
        setae_vm_raise(vm, "TypeError", "expected at least 1 argument");
        return setae_none();
    }
    if (n == 0) {
        if (lst != 0) {
            setae_vm_pop_tmp(vm);
        }
        if (has_def) {
            return defval;
        }
        setae_vm_raise(vm, "ValueError", "%s() arg is an empty sequence",
                       want_max ? "max" : "min");
        return setae_none();
    }
    SetaeValue best = items[0];
    SetaeValue bestkey = apply_key(vm, keyfn, best);
    if (vm->error) {
        if (lst != 0) {
            setae_vm_pop_tmp(vm);
        }
        return setae_none();
    }
    setae_vm_push_tmp(vm, bestkey);
    for (uint32_t i = 1; i < n; i++) {
        SetaeValue k = apply_key(vm, keyfn, items[i]);
        if (vm->error) {
            break;
        }
        int swap = want_max ? setae_value_lt(vm, bestkey, k)
                            : setae_value_lt(vm, k, bestkey);
        if (vm->error) {
            break;
        }
        if (swap) {
            best = items[i];
            setae_vm_pop_tmp(vm);
            bestkey = k;
            setae_vm_push_tmp(vm, bestkey);
        }
    }
    setae_vm_pop_tmp(vm);
    if (lst != 0) {
        setae_vm_pop_tmp(vm);
    }
    return vm->error ? setae_none() : best;
}

static SetaeValue builtin_min(SetaeVM *vm, SetaeValue *args, int nargs) {
    return min_max(vm, args, nargs, 0);
}

static SetaeValue builtin_max(SetaeVM *vm, SetaeValue *args, int nargs) {
    return min_max(vm, args, nargs, 1);
}

static SetaeValue builtin_abs(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "abs() takes exactly one argument");
        return setae_none();
    }
    SetaeValue v = args[0];
    if (setae_is_int(v)) {
        int64_t x = setae_to_int(v);
        return setae_from_int((int32_t)(x < 0 ? -x : x));
    }
    if (setae_obj_type(v) == SETAE_T_BIGINT) {
        return setae_int_sign(v) < 0 ? setae_int_neg(vm->heap, v) : v;
    }
    if (setae_is_bool(v)) {
        return setae_from_int(setae_to_bool(v) ? 1 : 0);
    }
    if (setae_is_float(v)) {
        double x = setae_to_float(v);
        return setae_from_float(x < 0 ? -x : x);
    }
    setae_vm_raise(vm, "TypeError", "bad operand type for abs(): '%s'", setae_type_name(v));
    return setae_none();
}

static double round_half_even(double d) {
    double f = floor(d);
    double diff = d - f;
    if (diff < 0.5) {
        return f;
    }
    if (diff > 0.5) {
        return f + 1.0;
    }
    return fmod(f, 2.0) == 0.0 ? f : f + 1.0;
}

static SetaeValue int_or_float(double d) {
    if (d >= -2147483648.0 && d <= 2147483647.0) {
        return setae_from_int((int32_t)d);
    }
    return setae_from_float(d);
}

static SetaeValue builtin_round(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs < 1 || nargs > 2) {
        setae_vm_raise(vm, "TypeError", "round() takes 1 or 2 arguments");
        return setae_none();
    }
    int has_n = nargs == 2 && !setae_is_none(args[1]);
    int ndigits = has_n && setae_is_int(args[1]) ? setae_to_int(args[1]) : 0;
    SetaeValue v = args[0];
    if (setae_is_int(v) || setae_is_bool(v)) {
        int64_t x = setae_is_bool(v) ? (setae_to_bool(v) ? 1 : 0) : setae_to_int(v);
        if (!has_n || ndigits >= 0) {
            return setae_from_int((int32_t)x);
        }
        double p = pow(10.0, -ndigits);
        return int_or_float(round_half_even((double)x / p) * p);
    }
    if (setae_is_float(v)) {
        double x = setae_to_float(v);
        if (!has_n) {
            return int_or_float(round_half_even(x));
        }
        double p = pow(10.0, ndigits);
        return setae_from_float(round_half_even(x * p) / p);
    }
    setae_vm_raise(vm, "TypeError", "type %s doesn't define __round__ method",
                   setae_type_name(v));
    return setae_none();
}

static SetaeValue builtin_divmod(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 2) {
        setae_vm_raise(vm, "TypeError", "divmod expected 2 arguments, got %d", nargs);
        return setae_none();
    }
    SetaeValue a = args[0], b = args[1];
    int ai = setae_is_integer(a);
    int bi = setae_is_integer(b);
    if (ai && bi) {
        if (setae_int_sign(b) == 0) {
            setae_vm_raise(vm, "ZeroDivisionError", "integer division or modulo by zero");
            return setae_none();
        }
        SetaeValue q, r;
        setae_int_divmod(vm->heap, a, b, &q, &r);
        setae_vm_push_tmp(vm, q);
        setae_vm_push_tmp(vm, r);
        SetaeValue pair[2] = {q, r};
        SetaeValue t = setae_tuple_new(vm->heap, pair, 2);
        setae_vm_pop_tmp(vm);
        setae_vm_pop_tmp(vm);
        return t;
    }
    if ((ai || setae_is_float(a)) && (bi || setae_is_float(b))) {
        double x = setae_is_float(a) ? setae_to_float(a)
                                     : (double)(setae_is_bool(a) ? setae_to_bool(a)
                                                                 : setae_to_int(a));
        double y = setae_is_float(b) ? setae_to_float(b)
                                     : (double)(setae_is_bool(b) ? setae_to_bool(b)
                                                                 : setae_to_int(b));
        if (y == 0.0) {
            setae_vm_raise(vm, "ZeroDivisionError", "float divmod()");
            return setae_none();
        }
        double q = floor(x / y);
        double r = x - q * y;
        SetaeValue pair[2] = {setae_from_float(q), setae_from_float(r)};
        return setae_tuple_new(vm->heap, pair, 2);
    }
    setae_vm_raise(vm, "TypeError", "unsupported operand type(s) for divmod()");
    return setae_none();
}

static SetaeValue builtin_ord(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1 || !setae_is_str(args[0]) || setae_str_count(args[0]) != 1) {
        setae_vm_raise(vm, "TypeError", "ord() expected a character");
        return setae_none();
    }
    const unsigned char *p = (const unsigned char *)setae_str_data(args[0]);
    int32_t cp;
    if (p[0] < 0x80) {
        cp = p[0];
    } else if (p[0] < 0xe0) {
        cp = ((p[0] & 0x1f) << 6) | (p[1] & 0x3f);
    } else if (p[0] < 0xf0) {
        cp = ((p[0] & 0x0f) << 12) | ((p[1] & 0x3f) << 6) | (p[2] & 0x3f);
    } else {
        cp = ((p[0] & 0x07) << 18) | ((p[1] & 0x3f) << 12) | ((p[2] & 0x3f) << 6) |
             (p[3] & 0x3f);
    }
    return setae_from_int(cp);
}

static SetaeValue builtin_chr(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1 || !setae_is_int(args[0])) {
        setae_vm_raise(vm, "TypeError", "chr() takes exactly one integer");
        return setae_none();
    }
    int64_t cp = setae_to_int(args[0]);
    if (cp < 0 || cp > 0x10ffff) {
        setae_vm_raise(vm, "ValueError", "chr() arg not in range(0x110000)");
        return setae_none();
    }
    char buf[4];
    int n;
    if (cp < 0x80) {
        buf[0] = (char)cp;
        n = 1;
    } else if (cp < 0x800) {
        buf[0] = (char)(0xc0 | (cp >> 6));
        buf[1] = (char)(0x80 | (cp & 0x3f));
        n = 2;
    } else if (cp < 0x10000) {
        buf[0] = (char)(0xe0 | (cp >> 12));
        buf[1] = (char)(0x80 | ((cp >> 6) & 0x3f));
        buf[2] = (char)(0x80 | (cp & 0x3f));
        n = 3;
    } else {
        buf[0] = (char)(0xf0 | (cp >> 18));
        buf[1] = (char)(0x80 | ((cp >> 12) & 0x3f));
        buf[2] = (char)(0x80 | ((cp >> 6) & 0x3f));
        buf[3] = (char)(0x80 | (cp & 0x3f));
        n = 4;
    }
    return setae_str_new(vm->heap, buf, (size_t)n);
}

static SetaeValue int_base(SetaeVM *vm, SetaeValue *args, int nargs, int base,
                           const char *prefix) {
    if (nargs != 1 || !(setae_is_int(args[0]) || setae_is_bool(args[0]))) {
        setae_vm_raise(vm, "TypeError", "'%s' object cannot be interpreted as an integer",
                       nargs == 1 ? setae_type_name(args[0]) : "");
        return setae_none();
    }
    int64_t x = setae_is_bool(args[0]) ? (setae_to_bool(args[0]) ? 1 : 0)
                                       : setae_to_int(args[0]);
    char digits[70];
    int neg = x < 0;
    uint64_t u = neg ? (uint64_t)(-(x + 1)) + 1 : (uint64_t)x;
    int di = 0;
    if (u == 0) {
        digits[di++] = '0';
    }
    while (u > 0) {
        int d = (int)(u % (uint64_t)base);
        digits[di++] = (char)(d < 10 ? '0' + d : 'a' + d - 10);
        u /= (uint64_t)base;
    }
    char out[80];
    int oi = 0;
    if (neg) {
        out[oi++] = '-';
    }
    out[oi++] = prefix[0];
    out[oi++] = prefix[1];
    while (di > 0) {
        out[oi++] = digits[--di];
    }
    return setae_str_new(vm->heap, out, (size_t)oi);
}

static SetaeValue builtin_hex(SetaeVM *vm, SetaeValue *args, int nargs) {
    return int_base(vm, args, nargs, 16, "0x");
}

static SetaeValue builtin_oct(SetaeVM *vm, SetaeValue *args, int nargs) {
    return int_base(vm, args, nargs, 8, "0o");
}

static SetaeValue builtin_bin(SetaeVM *vm, SetaeValue *args, int nargs) {
    return int_base(vm, args, nargs, 2, "0b");
}

static SetaeValue builtin_repr(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "repr() takes exactly one argument (%d given)",
                       nargs);
        return setae_none();
    }
    return setae_format_value(vm, args[0], 1);
}

static int isinstance_one(SetaeValue x, SetaeValue cls) {
    int xt = setae_obj_type(x);
    int ct = setae_obj_type(cls);
    if (ct == SETAE_T_BUILTIN) {
        SetaeBuiltin *b = setae_to_ptr(cls);
        if (!b->is_type) {
            return 0;
        }
        const char *nm = b->name;
        if (strcmp(nm, "int") == 0) {
            return setae_is_integer(x);
        }
        if (strcmp(nm, "bool") == 0) {
            return setae_is_bool(x);
        }
        if (strcmp(nm, "float") == 0) {
            return setae_is_float(x);
        }
        if (strcmp(nm, "str") == 0) {
            return xt == SETAE_T_STR;
        }
        if (strcmp(nm, "list") == 0) {
            return xt == SETAE_T_LIST;
        }
        if (strcmp(nm, "tuple") == 0) {
            return xt == SETAE_T_TUPLE;
        }
        if (strcmp(nm, "dict") == 0) {
            return xt == SETAE_T_DICT;
        }
        if (strcmp(nm, "set") == 0) {
            return xt == SETAE_T_SET && !((SetaeSet *)setae_to_ptr(x))->frozen;
        }
        if (strcmp(nm, "frozenset") == 0) {
            return xt == SETAE_T_SET && ((SetaeSet *)setae_to_ptr(x))->frozen;
        }
        if (strcmp(nm, "range") == 0) {
            return xt == SETAE_T_RANGE;
        }
        return 0;
    }
    if (ct == SETAE_T_CLASS) {
        if (xt != SETAE_T_INSTANCE) {
            return 0;
        }
        SetaeValue c = ((SetaeInstance *)setae_to_ptr(x))->cls;
        while (setae_obj_type(c) == SETAE_T_CLASS) {
            if (c == cls) {
                return 1;
            }
            c = ((SetaeClass *)setae_to_ptr(c))->base;
        }
        return 0;
    }
    if (ct == SETAE_T_EXCTYPE) {
        if (xt != SETAE_T_EXC) {
            return 0;
        }
        const char *want = ((SetaeExcType *)setae_to_ptr(cls))->name;
        if (strcmp(want, "Exception") == 0 || strcmp(want, "BaseException") == 0) {
            return 1;
        }
        return strcmp(((SetaeExc *)setae_to_ptr(x))->kind, want) == 0;
    }
    return 0;
}

static SetaeValue builtin_isinstance(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 2) {
        setae_vm_raise(vm, "TypeError", "isinstance expected 2 arguments, got %d", nargs);
        return setae_none();
    }
    if (setae_obj_type(args[1]) == SETAE_T_TUPLE) {
        SetaeTuple *t = setae_to_ptr(args[1]);
        for (uint32_t i = 0; i < t->len; i++) {
            if (isinstance_one(args[0], t->items[i])) {
                return setae_bool(1);
            }
        }
        return setae_bool(0);
    }
    return setae_bool(isinstance_one(args[0], args[1]));
}

static SetaeValue builtin_iter(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "iter() takes exactly one argument (%d given)",
                       nargs);
        return setae_none();
    }
    return setae_make_iter(vm, args[0]);
}

static SetaeValue builtin_hash(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "hash() takes exactly one argument (%d given)",
                       nargs);
        return setae_none();
    }
    int t = setae_obj_type(args[0]);
    if (t == SETAE_T_LIST || t == SETAE_T_DICT ||
        (t == SETAE_T_SET && !((SetaeSet *)setae_to_ptr(args[0]))->frozen)) {
        setae_vm_raise(vm, "TypeError", "unhashable type: '%s'", setae_type_name(args[0]));
        return setae_none();
    }
    return setae_from_int((int32_t)setae_value_hash(args[0]));
}

static SetaeValue builtin_id(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "id() takes exactly one argument (%d given)", nargs);
        return setae_none();
    }
    if (setae_is_ptr(args[0])) {
        return int_or_float((double)(uintptr_t)setae_to_ptr(args[0]));
    }
    return int_or_float((double)(int64_t)args[0]);
}

static int is_subclass_of(SetaeValue sub, SetaeValue cls) {
    if (setae_obj_type(sub) != SETAE_T_CLASS) {
        return 0;
    }
    SetaeValue c = sub;
    while (setae_obj_type(c) == SETAE_T_CLASS) {
        if (c == cls) {
            return 1;
        }
        c = ((SetaeClass *)setae_to_ptr(c))->base;
    }
    return 0;
}

static int type_builtin_named(SetaeValue v, const char *name) {
    return setae_obj_type(v) == SETAE_T_BUILTIN &&
           ((SetaeBuiltin *)setae_to_ptr(v))->is_type &&
           strcmp(((SetaeBuiltin *)setae_to_ptr(v))->name, name) == 0;
}

static int subclass_matches(SetaeValue sub, SetaeValue cls) {
    if (sub == cls) {
        return 1;
    }
    if (type_builtin_named(sub, "bool") && type_builtin_named(cls, "int")) {
        return 1;
    }
    return is_subclass_of(sub, cls);
}

static SetaeValue builtin_issubclass(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 2) {
        setae_vm_raise(vm, "TypeError", "issubclass expected 2 arguments, got %d", nargs);
        return setae_none();
    }
    if (setae_obj_type(args[0]) != SETAE_T_CLASS &&
        setae_obj_type(args[0]) != SETAE_T_BUILTIN) {
        setae_vm_raise(vm, "TypeError", "issubclass() arg 1 must be a class");
        return setae_none();
    }
    if (setae_obj_type(args[1]) == SETAE_T_TUPLE) {
        SetaeTuple *t = setae_to_ptr(args[1]);
        for (uint32_t i = 0; i < t->len; i++) {
            if (subclass_matches(args[0], t->items[i])) {
                return setae_bool(1);
            }
        }
        return setae_bool(0);
    }
    return setae_bool(subclass_matches(args[0], args[1]));
}

static SetaeValue builtin_pow(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs < 2 || nargs > 3) {
        setae_vm_raise(vm, "TypeError", "pow() takes 2 or 3 arguments");
        return setae_none();
    }
    if (nargs == 3) {
        if (!(setae_is_int(args[0]) || setae_is_bool(args[0])) ||
            !(setae_is_int(args[1]) || setae_is_bool(args[1])) ||
            !(setae_is_int(args[2]) || setae_is_bool(args[2]))) {
            setae_vm_raise(vm, "TypeError", "pow() 3rd argument requires integers");
            return setae_none();
        }
        int64_t base = setae_to_int(args[0]);
        int64_t exp = setae_to_int(args[1]);
        int64_t mod = setae_to_int(args[2]);
        if (mod == 0) {
            setae_vm_raise(vm, "ValueError", "pow() 3rd argument cannot be 0");
            return setae_none();
        }
        if (exp < 0) {
            setae_vm_raise(vm, "ValueError", "pow() negative exponent with modulus");
            return setae_none();
        }
        int64_t result = 1 % mod;
        int64_t b = base % mod;
        if (b < 0) {
            b += mod;
        }
        while (exp > 0) {
            if (exp & 1) {
                result = (result * b) % mod;
            }
            exp >>= 1;
            b = (b * b) % mod;
        }
        return setae_from_int((int32_t)result);
    }
    double base = setae_is_float(args[0]) ? setae_to_float(args[0])
                                          : (double)setae_to_int(args[0]);
    double exp = setae_is_float(args[1]) ? setae_to_float(args[1])
                                         : (double)setae_to_int(args[1]);
    if ((setae_is_int(args[0]) || setae_is_bool(args[0])) &&
        (setae_is_int(args[1]) || setae_is_bool(args[1])) && exp >= 0) {
        return int_or_float(pow(base, exp));
    }
    return setae_from_float(pow(base, exp));
}

static SetaeValue builtin_callable(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "callable() takes exactly one argument (%d given)",
                       nargs);
        return setae_none();
    }
    int t = setae_obj_type(args[0]);
    int ok = t == SETAE_T_FUNCTION || t == SETAE_T_BUILTIN || t == SETAE_T_CLASS ||
             t == SETAE_T_BOUND || t == SETAE_T_EXCTYPE;
    return setae_bool(ok);
}

static SetaeValue builtin_sorted(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "sorted expected 1 positional argument, got %d",
                       nargs);
        return setae_none();
    }
    SetaeValue keyfn = setae_none();
    kwarg_get(vm, "key", &keyfn);
    SetaeValue rv;
    int reverse = 0;
    if (kwarg_get(vm, "reverse", &rv)) {
        reverse = setae_truthy(rv);
    }
    SetaeValue lst = setae_iter_collect(vm, args[0]);
    if (vm->error) {
        return setae_none();
    }
    setae_vm_push_tmp(vm, lst);
    SetaeList *l = setae_to_ptr(lst);
    SetaeValue keysv = setae_list_new(vm->heap, l->len);
    setae_vm_push_tmp(vm, keysv);
    SetaeList *keys = setae_to_ptr(keysv);
    for (uint32_t i = 0; i < l->len; i++) {
        SetaeValue k = apply_key(vm, keyfn, l->items[i]);
        if (vm->error) {
            setae_vm_pop_tmp(vm);
            setae_vm_pop_tmp(vm);
            return setae_none();
        }
        setae_list_push(keys, k);
    }
    for (uint32_t i = 1; i < l->len; i++) {
        SetaeValue kk = keys->items[i];
        SetaeValue vv = l->items[i];
        uint32_t j = i;
        while (j > 0) {
            int lt = reverse ? setae_value_lt(vm, keys->items[j - 1], kk)
                             : setae_value_lt(vm, kk, keys->items[j - 1]);
            if (vm->error) {
                setae_vm_pop_tmp(vm);
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
    setae_vm_pop_tmp(vm);
    return lst;
}

static SetaeValue single_source_op(SetaeVM *vm, uint8_t kind, SetaeValue func,
                                   SetaeValue iterable) {
    SetaeValue src = setae_make_iter(vm, iterable);
    if (vm->error) {
        return setae_none();
    }
    setae_vm_push_tmp(vm, src);
    SetaeValue srcs = setae_list_new(vm->heap, 1);
    setae_list_push(setae_to_ptr(srcs), src);
    setae_vm_pop_tmp(vm);
    setae_vm_push_tmp(vm, srcs);
    SetaeValue op = setae_iterop_new(vm->heap, kind, func, srcs);
    setae_vm_pop_tmp(vm);
    return op;
}

static SetaeValue builtin_reversed(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "reversed() takes exactly one argument");
        return setae_none();
    }
    SetaeValue lst = setae_iter_collect(vm, args[0]);
    if (vm->error) {
        return setae_none();
    }
    setae_vm_push_tmp(vm, lst);
    SetaeValue srcs = setae_list_new(vm->heap, 1);
    setae_list_push(setae_to_ptr(srcs), lst);
    setae_vm_pop_tmp(vm);
    setae_vm_push_tmp(vm, srcs);
    SetaeValue op = setae_iterop_new(vm->heap, ITEROP_REVERSED, setae_none(), srcs);
    setae_vm_pop_tmp(vm);
    return op;
}

static SetaeValue builtin_enumerate(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs < 1 || nargs > 2) {
        setae_vm_raise(vm, "TypeError", "enumerate() takes 1 or 2 arguments");
        return setae_none();
    }
    int32_t start = 0;
    if (nargs == 2 && setae_is_int(args[1])) {
        start = setae_to_int(args[1]);
    }
    SetaeValue sv;
    if (kwarg_get(vm, "start", &sv) && setae_is_int(sv)) {
        start = setae_to_int(sv);
    }
    SetaeValue op = single_source_op(vm, ITEROP_ENUMERATE, setae_none(), args[0]);
    if (vm->error) {
        return setae_none();
    }
    ((SetaeIterOp *)setae_to_ptr(op))->index = start;
    return op;
}

static SetaeValue builtin_zip(SetaeVM *vm, SetaeValue *args, int nargs) {
    SetaeValue srcs = setae_list_new(vm->heap, (uint32_t)nargs);
    setae_vm_push_tmp(vm, srcs);
    for (int i = 0; i < nargs; i++) {
        SetaeValue s = setae_make_iter(vm, args[i]);
        if (vm->error) {
            setae_vm_pop_tmp(vm);
            return setae_none();
        }
        setae_list_push(setae_to_ptr(srcs), s);
    }
    SetaeValue op = setae_iterop_new(vm->heap, ITEROP_ZIP, setae_none(), srcs);
    setae_vm_pop_tmp(vm);
    return op;
}

static SetaeValue builtin_map(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 2) {
        setae_vm_raise(vm, "TypeError", "map() takes a function and one iterable");
        return setae_none();
    }
    return single_source_op(vm, ITEROP_MAP, args[0], args[1]);
}

static SetaeValue builtin_filter(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 2) {
        setae_vm_raise(vm, "TypeError", "filter() takes a function and one iterable");
        return setae_none();
    }
    return single_source_op(vm, ITEROP_FILTER, args[0], args[1]);
}

static SetaeValue builtin_any(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "any() takes exactly one argument");
        return setae_none();
    }
    SetaeValue lst = setae_iter_collect(vm, args[0]);
    if (vm->error) {
        return setae_none();
    }
    SetaeList *l = setae_to_ptr(lst);
    for (uint32_t i = 0; i < l->len; i++) {
        if (setae_truthy(l->items[i])) {
            return setae_bool(1);
        }
    }
    return setae_bool(0);
}

static SetaeValue builtin_all(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "all() takes exactly one argument");
        return setae_none();
    }
    SetaeValue lst = setae_iter_collect(vm, args[0]);
    if (vm->error) {
        return setae_none();
    }
    SetaeList *l = setae_to_ptr(lst);
    for (uint32_t i = 0; i < l->len; i++) {
        if (!setae_truthy(l->items[i])) {
            return setae_bool(0);
        }
    }
    return setae_bool(1);
}

static void register_gecko(SetaeVM *vm) {
    SetaeHeap *h = setae_vm_heap(vm);
    SetaeValue sdict = setae_dict_new(h);
    SetaeValue run = setae_builtin_new(h, builtin_sandbox_run, "run");
    setae_dict_push(setae_to_ptr(sdict), setae_str_new(h, "run", 3), run);
    SetaeValue sandbox =
        setae_module_new(h, setae_str_new(h, "_gecko.sandbox", 14), sdict);

    SetaeValue adict = setae_dict_new(h);
    SetaeValue actor = setae_module_new(h, setae_str_new(h, "_gecko.actor", 12), adict);

    SetaeValue array = setae_builtin_new(h, setae_array_build, "array");
    ((SetaeBuiltin *)setae_to_ptr(array))->kwargs_ok = 1;
    ((SetaeBuiltin *)setae_to_ptr(array))->is_type = 1;

    SetaeValue gdict = setae_dict_new(h);
    setae_dict_push(setae_to_ptr(gdict), setae_str_new(h, "sandbox", 7), sandbox);
    setae_dict_push(setae_to_ptr(gdict), setae_str_new(h, "actor", 5), actor);
    setae_dict_push(setae_to_ptr(gdict), setae_str_new(h, "array", 5), array);
    SetaeValue gecko = setae_module_new(h, setae_str_new(h, "_gecko", 6), gdict);
    setae_vm_register_builtin(vm, "_gecko", gecko);
}

SetaeValue setae_gecko_actor_module(SetaeVM *vm) {
    for (size_t i = 0; i < vm->nbuiltins; i++) {
        if (strcmp(vm->builtins[i].name, "_gecko") != 0) {
            continue;
        }
        SetaeModule *g = setae_to_ptr(vm->builtins[i].value);
        SetaeDict *gd = setae_to_ptr(g->dict);
        for (uint32_t j = 0; j < gd->len; j++) {
            SetaeValue k = gd->entries[j].key;
            if (setae_is_str(k) && setae_str_len(k) == 5 &&
                memcmp(setae_str_data(k), "actor", 5) == 0) {
                return gd->entries[j].value;
            }
        }
        break;
    }
    return setae_none();
}

void setae_gecko_actor_register(SetaeVM *vm, const char *name, SetaeValue value) {
    for (size_t i = 0; i < vm->nbuiltins; i++) {
        if (strcmp(vm->builtins[i].name, "_gecko") != 0) {
            continue;
        }
        SetaeModule *g = setae_to_ptr(vm->builtins[i].value);
        SetaeDict *gd = setae_to_ptr(g->dict);
        for (uint32_t j = 0; j < gd->len; j++) {
            SetaeValue k = gd->entries[j].key;
            if (setae_is_str(k) && setae_str_len(k) == 5 &&
                memcmp(setae_str_data(k), "actor", 5) == 0) {
                SetaeModule *am = setae_to_ptr(gd->entries[j].value);
                setae_dict_push(setae_to_ptr(am->dict),
                                setae_str_new(vm->heap, name, strlen(name)), value);
                return;
            }
        }
        return;
    }
}

static SetaeValue reg(SetaeVM *vm, SetaeHeap *h, const char *name, SetaeCFunc fn) {
    SetaeValue b = setae_builtin_new(h, fn, name);
    setae_vm_register_builtin(vm, name, b);
    return b;
}

static SetaeValue as_type(SetaeValue b) {
    ((SetaeBuiltin *)setae_to_ptr(b))->is_type = 1;
    return b;
}

static SetaeValue takes_kwargs(SetaeValue b) {
    ((SetaeBuiltin *)setae_to_ptr(b))->kwargs_ok = 1;
    return b;
}

void setae_vm_register_builtins(SetaeVM *vm) {
    SetaeHeap *h = setae_vm_heap(vm);
    reg(vm, h, "print", builtin_print);
    reg(vm, h, "len", builtin_len);
    as_type(reg(vm, h, "range", builtin_range));
    for (size_t i = 0; i < sizeof(EXC_KINDS) / sizeof(EXC_KINDS[0]); i++) {
        setae_vm_register_builtin(vm, EXC_KINDS[i], setae_exctype_new(h, EXC_KINDS[i]));
    }
    for (size_t i = 0; i < sizeof(TYPE_NAMES) / sizeof(TYPE_NAMES[0]); i++) {
        setae_vm_register_builtin(vm, TYPE_NAMES[i], setae_exctype_new(h, TYPE_NAMES[i]));
    }
    reg(vm, h, "type", builtin_type);
    reg(vm, h, "next", builtin_next);
    as_type(reg(vm, h, "str", builtin_str));
    as_type(reg(vm, h, "int", builtin_int));
    as_type(reg(vm, h, "float", builtin_float));
    as_type(reg(vm, h, "bool", builtin_bool));
    as_type(reg(vm, h, "list", builtin_list));
    as_type(reg(vm, h, "tuple", builtin_tuple));
    as_type(takes_kwargs(reg(vm, h, "dict", builtin_dict)));
    as_type(takes_kwargs(reg(vm, h, "set", builtin_set)));
    as_type(reg(vm, h, "frozenset", builtin_frozenset));
    reg(vm, h, "sum", builtin_sum);
    takes_kwargs(reg(vm, h, "min", builtin_min));
    takes_kwargs(reg(vm, h, "max", builtin_max));
    reg(vm, h, "abs", builtin_abs);
    reg(vm, h, "round", builtin_round);
    reg(vm, h, "divmod", builtin_divmod);
    reg(vm, h, "ord", builtin_ord);
    reg(vm, h, "chr", builtin_chr);
    reg(vm, h, "hex", builtin_hex);
    reg(vm, h, "oct", builtin_oct);
    reg(vm, h, "bin", builtin_bin);
    reg(vm, h, "repr", builtin_repr);
    reg(vm, h, "isinstance", builtin_isinstance);
    reg(vm, h, "issubclass", builtin_issubclass);
    reg(vm, h, "callable", builtin_callable);
    reg(vm, h, "iter", builtin_iter);
    reg(vm, h, "hash", builtin_hash);
    reg(vm, h, "id", builtin_id);
    reg(vm, h, "pow", builtin_pow);
    reg(vm, h, "getattr", setae_builtin_getattr);
    reg(vm, h, "hasattr", setae_builtin_hasattr);
    reg(vm, h, "setattr", setae_builtin_setattr);
    takes_kwargs(reg(vm, h, "sorted", builtin_sorted));
    as_type(reg(vm, h, "reversed", builtin_reversed));
    as_type(takes_kwargs(reg(vm, h, "enumerate", builtin_enumerate)));
    as_type(reg(vm, h, "zip", builtin_zip));
    as_type(reg(vm, h, "map", builtin_map));
    as_type(reg(vm, h, "filter", builtin_filter));
    reg(vm, h, "any", builtin_any);
    reg(vm, h, "all", builtin_all);
    register_gecko(vm);
}
