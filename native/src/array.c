#include "internal.h"

#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct SetaeBuffer {
    atomic_size_t refcount;
    uint8_t dtype;
    size_t len;
    void *data;
};

size_t setae_dtype_size(uint8_t dtype) {
    switch (dtype) {
    case DTYPE_F64:
    case DTYPE_I64:
        return 8;
    case DTYPE_F32:
    case DTYPE_I32:
        return 4;
    default:
        return 8;
    }
}

const char *setae_dtype_name(uint8_t dtype) {
    switch (dtype) {
    case DTYPE_F64:
        return "f64";
    case DTYPE_F32:
        return "f32";
    case DTYPE_I64:
        return "i64";
    case DTYPE_I32:
        return "i32";
    default:
        return "f64";
    }
}

static int dtype_parse(const char *name, size_t len, uint8_t *out) {
    if (len == 3 && memcmp(name, "f64", 3) == 0) {
        *out = DTYPE_F64;
        return 1;
    }
    if (len == 3 && memcmp(name, "f32", 3) == 0) {
        *out = DTYPE_F32;
        return 1;
    }
    if (len == 3 && memcmp(name, "i64", 3) == 0) {
        *out = DTYPE_I64;
        return 1;
    }
    if (len == 3 && memcmp(name, "i32", 3) == 0) {
        *out = DTYPE_I32;
        return 1;
    }
    return 0;
}

static int dtype_is_float(uint8_t dtype) {
    return dtype == DTYPE_F64 || dtype == DTYPE_F32;
}

SetaeBuffer *setae_buffer_alloc(uint8_t dtype, size_t len) {
    SetaeBuffer *b = malloc(sizeof(SetaeBuffer));
    if (b == NULL) {
        return NULL;
    }
    atomic_init(&b->refcount, 1);
    b->dtype = dtype;
    b->len = len;
    size_t bytes = len * setae_dtype_size(dtype);
    if (bytes == 0) {
        bytes = setae_dtype_size(dtype);
    }
    void *data = NULL;
    if (posix_memalign(&data, 64, bytes) != 0) {
        free(b);
        return NULL;
    }
    memset(data, 0, bytes);
    b->data = data;
    return b;
}

void setae_buffer_retain(SetaeBuffer *b) {
    atomic_fetch_add_explicit(&b->refcount, 1, memory_order_relaxed);
}

void setae_buffer_release(SetaeBuffer *b) {
    if (atomic_fetch_sub_explicit(&b->refcount, 1, memory_order_acq_rel) == 1) {
        free(b->data);
        free(b);
    }
}

void *setae_buffer_data(SetaeBuffer *b) {
    return b->data;
}

static double value_to_double(SetaeVM *vm, SetaeValue v, int *ok) {
    *ok = 1;
    if (setae_is_int(v)) {
        return (double)setae_to_int(v);
    }
    if (setae_is_float(v)) {
        return setae_to_float(v);
    }
    if (setae_obj_type(v) == SETAE_T_BIGINT) {
        return setae_int_to_double(v);
    }
    *ok = 0;
    setae_vm_raise(vm, "TypeError", "array element must be a number, not '%s'",
                   setae_type_name(v));
    return 0.0;
}

static int64_t value_to_i64(SetaeVM *vm, SetaeValue v, int *ok) {
    *ok = 1;
    if (setae_is_int(v)) {
        return (int64_t)setae_to_int(v);
    }
    if (setae_obj_type(v) == SETAE_T_BIGINT) {
        int64_t out;
        if (setae_int_fits_i64(v, &out)) {
            return out;
        }
        *ok = 0;
        setae_vm_raise(vm, "OverflowError", "int too large for an integer array");
        return 0;
    }
    *ok = 0;
    setae_vm_raise(vm, "TypeError", "integer array element must be an int, not '%s'",
                   setae_type_name(v));
    return 0;
}

static void store_double(void *data, uint8_t dtype, size_t i, double x) {
    if (dtype == DTYPE_F64) {
        ((double *)data)[i] = x;
    } else {
        ((float *)data)[i] = (float)x;
    }
}

static void store_i64(void *data, uint8_t dtype, size_t i, int64_t x) {
    if (dtype == DTYPE_I64) {
        ((int64_t *)data)[i] = x;
    } else {
        ((int32_t *)data)[i] = (int32_t)x;
    }
}

SetaeValue setae_array_build(SetaeVM *vm, SetaeValue *args, int nargs) {
    if (nargs < 1) {
        setae_vm_raise(vm, "TypeError", "array() takes at least one argument");
        return setae_none();
    }
    uint8_t dtype = DTYPE_F64;
    SetaeValue dt = 0;
    if (nargs >= 2) {
        dt = args[1];
    }
    if (vm->cur_kwargs != 0) {
        SetaeDict *kw = setae_to_ptr(vm->cur_kwargs);
        for (uint32_t e = 0; e < kw->len; e++) {
            SetaeValue k = kw->entries[e].key;
            if (setae_is_str(k) && setae_str_len(k) == 5 &&
                memcmp(setae_str_data(k), "dtype", 5) == 0) {
                dt = kw->entries[e].value;
                break;
            }
        }
    }
    if (dt != 0) {
        if (!setae_is_str(dt) ||
            !dtype_parse(setae_str_data(dt), setae_str_len(dt), &dtype)) {
            setae_vm_raise(vm, "ValueError", "unknown array dtype");
            return setae_none();
        }
    }

    SetaeValue lst = setae_iter_collect(vm, args[0]);
    if (vm->error) {
        return setae_none();
    }
    setae_vm_push_tmp(vm, lst);
    SetaeList *l = setae_to_ptr(lst);
    SetaeBuffer *buf = setae_buffer_alloc(dtype, l->len);
    if (buf == NULL) {
        setae_vm_pop_tmp(vm);
        setae_vm_raise(vm, "MemoryError", "out of memory");
        return setae_none();
    }
    void *data = setae_buffer_data(buf);
    for (uint32_t i = 0; i < l->len; i++) {
        int ok;
        if (dtype_is_float(dtype)) {
            double x = value_to_double(vm, l->items[i], &ok);
            if (!ok) {
                setae_buffer_release(buf);
                setae_vm_pop_tmp(vm);
                return setae_none();
            }
            store_double(data, dtype, i, x);
        } else {
            int64_t x = value_to_i64(vm, l->items[i], &ok);
            if (!ok) {
                setae_buffer_release(buf);
                setae_vm_pop_tmp(vm);
                return setae_none();
            }
            store_i64(data, dtype, i, x);
        }
    }
    setae_vm_pop_tmp(vm);
    return setae_array_new(vm->heap, buf, dtype, 0, l->len);
}

SetaeValue setae_array_get(SetaeVM *vm, SetaeValue arr, int64_t i) {
    SetaeArray *a = setae_to_ptr(arr);
    if (i < 0) {
        i += a->len;
    }
    if (i < 0 || (uint32_t)i >= a->len) {
        setae_vm_raise(vm, "IndexError", "array index out of range");
        return setae_none();
    }
    void *data = setae_buffer_data(a->buf);
    size_t k = a->offset + (size_t)i;
    switch (a->dtype) {
    case DTYPE_F64:
        return setae_from_float(((double *)data)[k]);
    case DTYPE_F32:
        return setae_from_float((double)((float *)data)[k]);
    case DTYPE_I64:
        return setae_int_from_i64(vm->heap, ((int64_t *)data)[k]);
    case DTYPE_I32:
        return setae_int_from_i64(vm->heap, (int64_t)((int32_t *)data)[k]);
    default:
        return setae_none();
    }
}

SetaeValue setae_array_slice(SetaeVM *vm, SetaeValue arr, int64_t start, int64_t step,
                             int64_t count) {
    SetaeArray *a = setae_to_ptr(arr);
    if (step == 1) {
        setae_buffer_retain(a->buf);
        return setae_array_new(vm->heap, a->buf, a->dtype,
                               a->offset + (uint32_t)start, (uint32_t)count);
    }
    SetaeBuffer *buf = setae_buffer_alloc(a->dtype, (size_t)count);
    if (buf == NULL) {
        setae_vm_raise(vm, "MemoryError", "out of memory");
        return setae_none();
    }
    size_t sz = setae_dtype_size(a->dtype);
    char *src = (char *)setae_buffer_data(a->buf) + (size_t)a->offset * sz;
    char *dst = setae_buffer_data(buf);
    for (int64_t k = 0, i = start; k < count; k++, i += step) {
        memcpy(dst + (size_t)k * sz, src + (size_t)i * sz, sz);
    }
    return setae_array_new(vm->heap, buf, a->dtype, 0, (uint32_t)count);
}

void setae_array_repr(SetaeVM *vm, SetaeValue arr) {
    SetaeArray *a = setae_to_ptr(arr);
    setae_vm_append_output(vm, "array([", 7);
    char buf[64];
    void *data = setae_buffer_data(a->buf);
    for (uint32_t i = 0; i < a->len; i++) {
        if (i > 0) {
            setae_vm_append_output(vm, ", ", 2);
        }
        size_t k = a->offset + i;
        int n;
        if (a->dtype == DTYPE_F64 || a->dtype == DTYPE_F32) {
            double x = a->dtype == DTYPE_F64 ? ((double *)data)[k]
                                             : (double)((float *)data)[k];
            n = snprintf(buf, sizeof(buf), "%g", x);
            if (x == (double)(int64_t)x && n > 0 && !strchr(buf, '.') &&
                !strchr(buf, 'e') && !strchr(buf, 'n')) {
                n = snprintf(buf, sizeof(buf), "%.1f", x);
            }
        } else {
            int64_t x = a->dtype == DTYPE_I64 ? ((int64_t *)data)[k]
                                              : (int64_t)((int32_t *)data)[k];
            n = snprintf(buf, sizeof(buf), "%lld", (long long)x);
        }
        setae_vm_append_output(vm, buf, (size_t)n);
    }
    setae_vm_append_output(vm, "], dtype='", 10);
    const char *dn = setae_dtype_name(a->dtype);
    setae_vm_append_output(vm, dn, strlen(dn));
    setae_vm_append_output(vm, "')", 2);
}
