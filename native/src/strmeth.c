#include "internal.h"

#include <stdlib.h>
#include <string.h>

static int is_ws(unsigned char c) {
    return c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\f' || c == '\v';
}

static SetaeValue new_str(SetaeVM *vm, const char *d, size_t n) {
    return setae_str_new(setae_vm_heap(vm), d, n);
}

static int64_t find_bytes(const char *h, size_t hn, const char *n, size_t nn, size_t start) {
    if (nn == 0) {
        return start <= hn ? (int64_t)start : (int64_t)hn;
    }
    if (nn > hn) {
        return -1;
    }
    for (size_t i = start; i + nn <= hn; i++) {
        if (memcmp(h + i, n, nn) == 0) {
            return (int64_t)i;
        }
    }
    return -1;
}

static int64_t rfind_bytes(const char *h, size_t hn, const char *n, size_t nn) {
    if (nn == 0) {
        return (int64_t)hn;
    }
    if (nn > hn) {
        return -1;
    }
    for (size_t i = hn - nn + 1; i > 0; i--) {
        if (memcmp(h + i - 1, n, nn) == 0) {
            return (int64_t)(i - 1);
        }
    }
    return -1;
}

static size_t byte_to_char(const char *p, size_t byteoff) {
    size_t ci = 0;
    for (size_t i = 0; i < byteoff; i++) {
        if (((unsigned char)p[i] & 0xc0) != 0x80) {
            ci++;
        }
    }
    return ci;
}

static int want_str_arg(SetaeVM *vm, const char *method, SetaeValue v) {
    if (!setae_is_str(v)) {
        setae_vm_raise(vm, "TypeError", "%s() argument must be str, not %s", method,
                       setae_type_name(v));
        return 0;
    }
    return 1;
}

static SetaeValue map_case(SetaeVM *vm, SetaeValue s, int mode) {
    size_t n = setae_str_len(s);
    const char *p = setae_str_data(s);
    char *buf = malloc(n ? n : 1);
    int prev_cased = 0;
    for (size_t i = 0; i < n; i++) {
        unsigned char c = (unsigned char)p[i];
        unsigned char o = c;
        if (mode == 0 && c >= 'a' && c <= 'z') {
            o = c - 32;
        } else if (mode == 1 && c >= 'A' && c <= 'Z') {
            o = c + 32;
        } else if (mode == 2) {
            if (i == 0 && c >= 'a' && c <= 'z') {
                o = c - 32;
            } else if (i > 0 && c >= 'A' && c <= 'Z') {
                o = c + 32;
            }
        } else if (mode == 3) {
            int alpha = (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z');
            if (alpha && !prev_cased && c >= 'a' && c <= 'z') {
                o = c - 32;
            } else if (alpha && prev_cased && c >= 'A' && c <= 'Z') {
                o = c + 32;
            }
            prev_cased = alpha;
        } else if (mode == 4) {
            if (c >= 'a' && c <= 'z') {
                o = c - 32;
            } else if (c >= 'A' && c <= 'Z') {
                o = c + 32;
            }
        }
        buf[i] = (char)o;
    }
    SetaeValue r = new_str(vm, buf, n);
    free(buf);
    return r;
}

static SetaeValue do_strip(SetaeVM *vm, SetaeValue s, SetaeValue *args, int nargs, int left,
                           int right) {
    size_t n = setae_str_len(s);
    const char *p = setae_str_data(s);
    const char *chars = NULL;
    size_t cn = 0;
    if (nargs == 1 && !setae_is_none(args[0])) {
        if (!want_str_arg(vm, "strip", args[0])) {
            return setae_none();
        }
        chars = setae_str_data(args[0]);
        cn = setae_str_len(args[0]);
    }
    size_t a = 0, b = n;
    while (left && a < b) {
        unsigned char c = (unsigned char)p[a];
        int strip = chars ? (memchr(chars, c, cn) != NULL) : is_ws(c);
        if (!strip) {
            break;
        }
        a++;
    }
    while (right && b > a) {
        unsigned char c = (unsigned char)p[b - 1];
        int strip = chars ? (memchr(chars, c, cn) != NULL) : is_ws(c);
        if (!strip) {
            break;
        }
        b--;
    }
    return new_str(vm, p + a, b - a);
}

static SetaeValue do_split(SetaeVM *vm, SetaeValue s, SetaeValue *args, int nargs) {
    size_t n = setae_str_len(s);
    const char *p = setae_str_data(s);
    int maxsplit = -1;
    if (nargs >= 2 && setae_is_int(args[1])) {
        maxsplit = setae_to_int(args[1]);
    }
    SetaeValue rv = setae_list_new(setae_vm_heap(vm), 0);
    setae_vm_push_tmp(vm, rv);
    SetaeList *r = setae_to_ptr(rv);
    if (nargs == 0 || setae_is_none(args[0])) {
        size_t i = 0;
        int splits = 0;
        while (i < n) {
            while (i < n && is_ws((unsigned char)p[i])) {
                i++;
            }
            if (i >= n) {
                break;
            }
            size_t start = i;
            if (maxsplit >= 0 && splits >= maxsplit) {
                i = n;
                while (i > start && is_ws((unsigned char)p[i - 1])) {
                    i--;
                }
            } else {
                while (i < n && !is_ws((unsigned char)p[i])) {
                    i++;
                }
            }
            setae_list_push(r, new_str(vm, p + start, i - start));
            splits++;
        }
        setae_vm_pop_tmp(vm);
        return rv;
    }
    if (!want_str_arg(vm, "split", args[0])) {
        setae_vm_pop_tmp(vm);
        return setae_none();
    }
    const char *sep = setae_str_data(args[0]);
    size_t sn = setae_str_len(args[0]);
    if (sn == 0) {
        setae_vm_raise(vm, "ValueError", "empty separator");
        setae_vm_pop_tmp(vm);
        return setae_none();
    }
    size_t start = 0;
    int splits = 0;
    for (;;) {
        if (maxsplit >= 0 && splits >= maxsplit) {
            break;
        }
        int64_t at = find_bytes(p, n, sep, sn, start);
        if (at < 0) {
            break;
        }
        setae_list_push(r, new_str(vm, p + start, (size_t)at - start));
        start = (size_t)at + sn;
        splits++;
    }
    setae_list_push(r, new_str(vm, p + start, n - start));
    setae_vm_pop_tmp(vm);
    return rv;
}

static SetaeValue do_join(SetaeVM *vm, SetaeValue s, SetaeValue *args, int nargs) {
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "join() takes exactly one argument (%d given)", nargs);
        return setae_none();
    }
    const char *sep = setae_str_data(s);
    size_t sn = setae_str_len(s);
    SetaeValue it = setae_make_iter(vm, args[0]);
    if (vm->error) {
        return setae_none();
    }
    setae_vm_push_tmp(vm, it);
    size_t cap = 64, len = 0;
    char *buf = malloc(cap);
    SetaeValue x;
    int first = 1;
    while (setae_iter_advance(vm, it, &x)) {
        if (vm->error) {
            break;
        }
        if (!setae_is_str(x)) {
            setae_vm_raise(vm, "TypeError",
                           "sequence item: expected str instance, %s found",
                           setae_type_name(x));
            break;
        }
        size_t xn = setae_str_len(x);
        size_t need = len + (first ? 0 : sn) + xn;
        while (need > cap) {
            cap *= 2;
            buf = realloc(buf, cap);
        }
        if (!first) {
            memcpy(buf + len, sep, sn);
            len += sn;
        }
        memcpy(buf + len, setae_str_data(x), xn);
        len += xn;
        first = 0;
    }
    setae_vm_pop_tmp(vm);
    if (vm->error) {
        free(buf);
        return setae_none();
    }
    SetaeValue r = new_str(vm, buf, len);
    free(buf);
    return r;
}

static SetaeValue do_replace(SetaeVM *vm, SetaeValue s, SetaeValue *args, int nargs) {
    if (nargs < 2 || !want_str_arg(vm, "replace", args[0]) ||
        !want_str_arg(vm, "replace", args[1])) {
        if (nargs < 2) {
            setae_vm_raise(vm, "TypeError", "replace() takes at least 2 arguments");
        }
        return setae_none();
    }
    int count = -1;
    if (nargs >= 3 && setae_is_int(args[2])) {
        count = setae_to_int(args[2]);
    }
    const char *p = setae_str_data(s);
    size_t n = setae_str_len(s);
    const char *o = setae_str_data(args[0]);
    size_t on = setae_str_len(args[0]);
    const char *w = setae_str_data(args[1]);
    size_t wn = setae_str_len(args[1]);
    size_t cap = n + 16, len = 0;
    char *buf = malloc(cap);
    size_t i = 0;
    int done = 0;
    while (i < n) {
        if (on > 0 && (count < 0 || done < count) && i + on <= n &&
            memcmp(p + i, o, on) == 0) {
            while (len + wn > cap) {
                cap *= 2;
                buf = realloc(buf, cap);
            }
            memcpy(buf + len, w, wn);
            len += wn;
            i += on;
            done++;
        } else {
            if (len + 1 > cap) {
                cap *= 2;
                buf = realloc(buf, cap);
            }
            buf[len++] = p[i++];
        }
    }
    SetaeValue r = new_str(vm, buf, len);
    free(buf);
    return r;
}

static int match_affix(SetaeValue s, SetaeValue affix, int end) {
    size_t n = setae_str_len(s);
    size_t an = setae_str_len(affix);
    if (an > n) {
        return 0;
    }
    const char *p = setae_str_data(s);
    const char *a = setae_str_data(affix);
    return memcmp(p + (end ? n - an : 0), a, an) == 0;
}

static SetaeValue do_affix(SetaeVM *vm, SetaeValue s, SetaeValue *args, int nargs,
                           const char *method, int end) {
    if (nargs != 1) {
        setae_vm_raise(vm, "TypeError", "%s() takes exactly one argument (%d given)", method,
                       nargs);
        return setae_none();
    }
    if (setae_obj_type(args[0]) == SETAE_T_TUPLE) {
        SetaeTuple *t = setae_to_ptr(args[0]);
        for (uint32_t i = 0; i < t->len; i++) {
            if (!want_str_arg(vm, method, t->items[i])) {
                return setae_none();
            }
            if (match_affix(s, t->items[i], end)) {
                return setae_bool(1);
            }
        }
        return setae_bool(0);
    }
    if (!want_str_arg(vm, method, args[0])) {
        return setae_none();
    }
    return setae_bool(match_affix(s, args[0], end));
}

static SetaeValue do_find(SetaeVM *vm, SetaeValue s, SetaeValue *args, int nargs,
                          const char *method, int reverse, int raise_missing) {
    if (nargs < 1 || !want_str_arg(vm, method, args[0])) {
        if (nargs < 1) {
            setae_vm_raise(vm, "TypeError", "%s() takes at least 1 argument", method);
        }
        return setae_none();
    }
    const char *p = setae_str_data(s);
    size_t n = setae_str_len(s);
    const char *sub = setae_str_data(args[0]);
    size_t sn = setae_str_len(args[0]);
    int64_t at = reverse ? rfind_bytes(p, n, sub, sn) : find_bytes(p, n, sub, sn, 0);
    if (at < 0) {
        if (raise_missing) {
            setae_vm_raise(vm, "ValueError", "substring not found");
            return setae_none();
        }
        return setae_from_int(-1);
    }
    return setae_from_int((int32_t)byte_to_char(p, (size_t)at));
}

static SetaeValue do_count(SetaeVM *vm, SetaeValue s, SetaeValue *args, int nargs) {
    if (nargs < 1 || !want_str_arg(vm, "count", args[0])) {
        return setae_none();
    }
    const char *p = setae_str_data(s);
    size_t n = setae_str_len(s);
    const char *sub = setae_str_data(args[0]);
    size_t sn = setae_str_len(args[0]);
    int32_t c = 0;
    if (sn == 0) {
        return setae_from_int((int32_t)setae_str_count(s) + 1);
    }
    size_t i = 0;
    while (i + sn <= n) {
        if (memcmp(p + i, sub, sn) == 0) {
            c++;
            i += sn;
        } else {
            i++;
        }
    }
    return setae_from_int(c);
}

static SetaeValue do_predicate(SetaeVM *vm, SetaeValue s, int kind) {
    (void)vm;
    size_t n = setae_str_len(s);
    const char *p = setae_str_data(s);
    if (n == 0) {
        return setae_bool(0);
    }
    int has_cased = 0;
    for (size_t i = 0; i < n; i++) {
        unsigned char c = (unsigned char)p[i];
        int lower = c >= 'a' && c <= 'z';
        int upper = c >= 'A' && c <= 'Z';
        int digit = c >= '0' && c <= '9';
        int alpha = lower || upper;
        int ok;
        switch (kind) {
        case 0:
            ok = digit;
            break;
        case 1:
            ok = alpha;
            break;
        case 2:
            ok = alpha || digit;
            break;
        case 3:
            ok = is_ws(c);
            break;
        case 4:
            if (alpha) {
                has_cased = 1;
            }
            ok = !lower;
            break;
        default:
            if (alpha) {
                has_cased = 1;
            }
            ok = !upper;
            break;
        }
        if (!ok) {
            return setae_bool(0);
        }
    }
    if (kind >= 4) {
        return setae_bool(has_cased);
    }
    return setae_bool(1);
}

static SetaeValue do_pad(SetaeVM *vm, SetaeValue s, SetaeValue *args, int nargs, int mode) {
    if (nargs < 1 || !setae_is_int(args[0])) {
        setae_vm_raise(vm, "TypeError", "width must be an integer");
        return setae_none();
    }
    int64_t width = setae_to_int(args[0]);
    char fill = mode == 3 ? '0' : ' ';
    if (nargs >= 2 && setae_is_str(args[1]) && setae_str_len(args[1]) == 1) {
        fill = setae_str_data(args[1])[0];
    }
    size_t chars = setae_str_count(s);
    size_t n = setae_str_len(s);
    const char *p = setae_str_data(s);
    if ((int64_t)chars >= width) {
        return new_str(vm, p, n);
    }
    size_t pad = (size_t)width - chars;
    size_t leftpad;
    if (mode == 0 || mode == 3) {
        leftpad = pad;
    } else if (mode == 2) {
        leftpad = pad / 2 + (pad & (size_t)width & 1);
    } else {
        leftpad = 0;
    }
    size_t rightpad = pad - leftpad;
    char *buf = malloc(n + pad);
    size_t off = 0;
    if (mode == 3 && n > 0 && (p[0] == '-' || p[0] == '+')) {
        buf[off++] = p[0];
        for (size_t i = 0; i < leftpad; i++) {
            buf[off++] = fill;
        }
        memcpy(buf + off, p + 1, n - 1);
        off += n - 1;
    } else {
        for (size_t i = 0; i < leftpad; i++) {
            buf[off++] = fill;
        }
        memcpy(buf + off, p, n);
        off += n;
        for (size_t i = 0; i < rightpad; i++) {
            buf[off++] = fill;
        }
    }
    SetaeValue r = new_str(vm, buf, off);
    free(buf);
    return r;
}

static SetaeValue do_remove_affix(SetaeVM *vm, SetaeValue s, SetaeValue *args, int nargs,
                                  const char *method, int end) {
    if (nargs != 1 || !want_str_arg(vm, method, args[0])) {
        return setae_none();
    }
    size_t n = setae_str_len(s);
    size_t an = setae_str_len(args[0]);
    const char *p = setae_str_data(s);
    if (an > 0 && match_affix(s, args[0], end)) {
        return end ? new_str(vm, p, n - an) : new_str(vm, p + an, n - an);
    }
    return new_str(vm, p, n);
}

static SetaeValue do_splitlines(SetaeVM *vm, SetaeValue s) {
    size_t n = setae_str_len(s);
    const char *p = setae_str_data(s);
    SetaeValue rv = setae_list_new(setae_vm_heap(vm), 0);
    setae_vm_push_tmp(vm, rv);
    SetaeList *r = setae_to_ptr(rv);
    size_t i = 0;
    while (i < n) {
        size_t start = i;
        while (i < n && p[i] != '\n' && p[i] != '\r') {
            i++;
        }
        setae_list_push(r, new_str(vm, p + start, i - start));
        if (i < n && p[i] == '\r' && i + 1 < n && p[i + 1] == '\n') {
            i += 2;
        } else if (i < n) {
            i++;
        }
    }
    setae_vm_pop_tmp(vm);
    return rv;
}

SetaeValue setae_str_method(SetaeVM *vm, SetaeValue s, const char *name, SetaeValue *args,
                            int nargs, int *found) {
    *found = 1;
    if (strcmp(name, "upper") == 0) {
        return map_case(vm, s, 0);
    }
    if (strcmp(name, "lower") == 0) {
        return map_case(vm, s, 1);
    }
    if (strcmp(name, "capitalize") == 0) {
        return map_case(vm, s, 2);
    }
    if (strcmp(name, "title") == 0) {
        return map_case(vm, s, 3);
    }
    if (strcmp(name, "swapcase") == 0) {
        return map_case(vm, s, 4);
    }
    if (strcmp(name, "strip") == 0) {
        return do_strip(vm, s, args, nargs, 1, 1);
    }
    if (strcmp(name, "lstrip") == 0) {
        return do_strip(vm, s, args, nargs, 1, 0);
    }
    if (strcmp(name, "rstrip") == 0) {
        return do_strip(vm, s, args, nargs, 0, 1);
    }
    if (strcmp(name, "split") == 0) {
        return do_split(vm, s, args, nargs);
    }
    if (strcmp(name, "splitlines") == 0) {
        return do_splitlines(vm, s);
    }
    if (strcmp(name, "join") == 0) {
        return do_join(vm, s, args, nargs);
    }
    if (strcmp(name, "replace") == 0) {
        return do_replace(vm, s, args, nargs);
    }
    if (strcmp(name, "startswith") == 0) {
        return do_affix(vm, s, args, nargs, "startswith", 0);
    }
    if (strcmp(name, "endswith") == 0) {
        return do_affix(vm, s, args, nargs, "endswith", 1);
    }
    if (strcmp(name, "find") == 0) {
        return do_find(vm, s, args, nargs, "find", 0, 0);
    }
    if (strcmp(name, "rfind") == 0) {
        return do_find(vm, s, args, nargs, "rfind", 1, 0);
    }
    if (strcmp(name, "index") == 0) {
        return do_find(vm, s, args, nargs, "index", 0, 1);
    }
    if (strcmp(name, "rindex") == 0) {
        return do_find(vm, s, args, nargs, "rindex", 1, 1);
    }
    if (strcmp(name, "count") == 0) {
        return do_count(vm, s, args, nargs);
    }
    if (strcmp(name, "isdigit") == 0 || strcmp(name, "isnumeric") == 0 ||
        strcmp(name, "isdecimal") == 0) {
        return do_predicate(vm, s, 0);
    }
    if (strcmp(name, "isalpha") == 0) {
        return do_predicate(vm, s, 1);
    }
    if (strcmp(name, "isalnum") == 0) {
        return do_predicate(vm, s, 2);
    }
    if (strcmp(name, "isspace") == 0) {
        return do_predicate(vm, s, 3);
    }
    if (strcmp(name, "isupper") == 0) {
        return do_predicate(vm, s, 4);
    }
    if (strcmp(name, "islower") == 0) {
        return do_predicate(vm, s, 5);
    }
    if (strcmp(name, "ljust") == 0) {
        return do_pad(vm, s, args, nargs, 1);
    }
    if (strcmp(name, "rjust") == 0) {
        return do_pad(vm, s, args, nargs, 0);
    }
    if (strcmp(name, "center") == 0) {
        return do_pad(vm, s, args, nargs, 2);
    }
    if (strcmp(name, "zfill") == 0) {
        return do_pad(vm, s, args, nargs, 3);
    }
    if (strcmp(name, "removeprefix") == 0) {
        return do_remove_affix(vm, s, args, nargs, "removeprefix", 0);
    }
    if (strcmp(name, "removesuffix") == 0) {
        return do_remove_affix(vm, s, args, nargs, "removesuffix", 1);
    }
    *found = 0;
    return setae_none();
}
