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

static double elem_double(void *data, uint8_t dtype, size_t i) {
    switch (dtype) {
    case DTYPE_F64:
        return ((double *)data)[i];
    case DTYPE_F32:
        return (double)((float *)data)[i];
    case DTYPE_I64:
        return (double)((int64_t *)data)[i];
    default:
        return (double)((int32_t *)data)[i];
    }
}

static int64_t elem_i64(void *data, uint8_t dtype, size_t i) {
    switch (dtype) {
    case DTYPE_I64:
        return ((int64_t *)data)[i];
    case DTYPE_I32:
        return (int64_t)((int32_t *)data)[i];
    case DTYPE_F64:
        return (int64_t)((double *)data)[i];
    default:
        return (int64_t)((float *)data)[i];
    }
}

typedef struct {
    void *out;
    void *da;
    void *db;
    uint32_t oa;
    uint32_t ob;
    uint8_t rd;
    uint8_t ca;
    uint8_t cb;
    int aa;
    int ba;
    int op;
    double sa;
    double sb;
    int64_t ia;
    int64_t ib;
} KernelCtx;

#if defined(__ARM_NEON) || defined(__ARM_NEON__)
#include <arm_neon.h>
#define SETAE_SIMD_NEON 1
#elif defined(__SSE2__)
#include <emmintrin.h>
#define SETAE_SIMD_SSE2 1
#endif

#define FAST_F64(NAME, EXPR, VEXPR)                                                      \
    static void NAME(const double *xa, const double *xb, double sb, double *o,           \
                     size_t start, size_t end, int vec_b) {                              \
        size_t i = start;                                                                \
        VEXPR                                                                            \
        for (; i < end; i++) {                                                           \
            double x = xa[i];                                                            \
            double y = vec_b ? xb[i] : sb;                                               \
            o[i] = EXPR;                                                                 \
        }                                                                                \
    }

#if defined(SETAE_SIMD_NEON)
#define VEC_F64(OPI)                                                                     \
    if (vec_b) {                                                                         \
        for (; i + 2 <= end; i += 2) {                                                   \
            vst1q_f64(o + i, OPI(vld1q_f64(xa + i), vld1q_f64(xb + i)));                 \
        }                                                                                \
    } else {                                                                             \
        float64x2_t vb = vdupq_n_f64(sb);                                                \
        for (; i + 2 <= end; i += 2) {                                                   \
            vst1q_f64(o + i, OPI(vld1q_f64(xa + i), vb));                                \
        }                                                                                \
    }
FAST_F64(f64_add, x + y, VEC_F64(vaddq_f64))
FAST_F64(f64_sub, x - y, VEC_F64(vsubq_f64))
FAST_F64(f64_mul, x *y, VEC_F64(vmulq_f64))
FAST_F64(f64_div, x / y, VEC_F64(vdivq_f64))
#elif defined(SETAE_SIMD_SSE2)
#define VEC_F64(OPI)                                                                     \
    if (vec_b) {                                                                         \
        for (; i + 2 <= end; i += 2) {                                                   \
            _mm_storeu_pd(o + i, OPI(_mm_loadu_pd(xa + i), _mm_loadu_pd(xb + i)));       \
        }                                                                                \
    } else {                                                                             \
        __m128d vb = _mm_set1_pd(sb);                                                    \
        for (; i + 2 <= end; i += 2) {                                                   \
            _mm_storeu_pd(o + i, OPI(_mm_loadu_pd(xa + i), vb));                         \
        }                                                                                \
    }
FAST_F64(f64_add, x + y, VEC_F64(_mm_add_pd))
FAST_F64(f64_sub, x - y, VEC_F64(_mm_sub_pd))
FAST_F64(f64_mul, x *y, VEC_F64(_mm_mul_pd))
FAST_F64(f64_div, x / y, VEC_F64(_mm_div_pd))
#else
FAST_F64(f64_add, x + y, )
FAST_F64(f64_sub, x - y, )
FAST_F64(f64_mul, x *y, )
FAST_F64(f64_div, x / y, )
#endif

#define FAST_I64(NAME, EXPR, VEXPR)                                                      \
    static void NAME(const int64_t *xa, const int64_t *xb, int64_t sb, int64_t *o,       \
                     size_t start, size_t end, int vec_b) {                              \
        size_t i = start;                                                                \
        VEXPR                                                                            \
        for (; i < end; i++) {                                                           \
            int64_t x = xa[i];                                                           \
            int64_t y = vec_b ? xb[i] : sb;                                              \
            o[i] = EXPR;                                                                 \
        }                                                                                \
    }

#if defined(SETAE_SIMD_NEON)
#define VEC_I64(OPI)                                                                     \
    if (vec_b) {                                                                         \
        for (; i + 2 <= end; i += 2) {                                                   \
            vst1q_s64(o + i, OPI(vld1q_s64(xa + i), vld1q_s64(xb + i)));                 \
        }                                                                                \
    } else {                                                                             \
        int64x2_t vb = vdupq_n_s64(sb);                                                  \
        for (; i + 2 <= end; i += 2) {                                                   \
            vst1q_s64(o + i, OPI(vld1q_s64(xa + i), vb));                                \
        }                                                                                \
    }
FAST_I64(i64_add, x + y, VEC_I64(vaddq_s64))
FAST_I64(i64_sub, x - y, VEC_I64(vsubq_s64))
#elif defined(SETAE_SIMD_SSE2)
#define VEC_I64(OPI)                                                                     \
    if (vec_b) {                                                                         \
        for (; i + 2 <= end; i += 2) {                                                   \
            _mm_storeu_si128((__m128i *)(o + i),                                         \
                             OPI(_mm_loadu_si128((const __m128i *)(xa + i)),             \
                                 _mm_loadu_si128((const __m128i *)(xb + i))));           \
        }                                                                                \
    } else {                                                                             \
        __m128i vb = _mm_set1_epi64x(sb);                                                \
        for (; i + 2 <= end; i += 2) {                                                   \
            _mm_storeu_si128((__m128i *)(o + i),                                         \
                             OPI(_mm_loadu_si128((const __m128i *)(xa + i)), vb));       \
        }                                                                                \
    }
FAST_I64(i64_add, x + y, VEC_I64(_mm_add_epi64))
FAST_I64(i64_sub, x - y, VEC_I64(_mm_sub_epi64))
#else
FAST_I64(i64_add, x + y, )
FAST_I64(i64_sub, x - y, )
#endif

FAST_I64(i64_mul, x *y, )

static int kernel_fast(KernelCtx *c, size_t start, size_t end) {
    int swap = !c->aa && c->ba && (c->op == BIN_ADD || c->op == BIN_MUL);
    int lhs_arr = c->aa || swap;
    const void *lhs_data = swap ? c->db : c->da;
    uint32_t lhs_off = swap ? c->ob : c->oa;
    uint8_t lhs_dtype = swap ? c->cb : c->ca;
    int rhs_arr = swap ? 0 : c->ba;
    const void *rhs_data = c->db;
    uint32_t rhs_off = c->ob;
    uint8_t rhs_dtype = swap ? c->ca : c->cb;
    double rhs_f = swap ? c->sa : c->sb;
    int64_t rhs_i = swap ? c->ia : c->ib;

    if (c->rd == DTYPE_F64 && lhs_arr && lhs_dtype == DTYPE_F64 &&
        (!rhs_arr || rhs_dtype == DTYPE_F64)) {
        const double *xa = (const double *)lhs_data + lhs_off;
        const double *xb = rhs_arr ? (const double *)rhs_data + rhs_off : NULL;
        double *o = c->out;
        switch (c->op) {
        case BIN_ADD:
            f64_add(xa, xb, rhs_f, o, start, end, rhs_arr);
            return 1;
        case BIN_SUB:
            f64_sub(xa, xb, rhs_f, o, start, end, rhs_arr);
            return 1;
        case BIN_MUL:
            f64_mul(xa, xb, rhs_f, o, start, end, rhs_arr);
            return 1;
        case BIN_DIV:
            f64_div(xa, xb, rhs_f, o, start, end, rhs_arr);
            return 1;
        default:
            return 0;
        }
    }
    if (c->rd == DTYPE_I64 && lhs_arr && lhs_dtype == DTYPE_I64 &&
        (!rhs_arr || rhs_dtype == DTYPE_I64)) {
        const int64_t *xa = (const int64_t *)lhs_data + lhs_off;
        const int64_t *xb = rhs_arr ? (const int64_t *)rhs_data + rhs_off : NULL;
        int64_t *o = c->out;
        switch (c->op) {
        case BIN_ADD:
            i64_add(xa, xb, rhs_i, o, start, end, rhs_arr);
            return 1;
        case BIN_SUB:
            i64_sub(xa, xb, rhs_i, o, start, end, rhs_arr);
            return 1;
        case BIN_MUL:
            i64_mul(xa, xb, rhs_i, o, start, end, rhs_arr);
            return 1;
        default:
            return 0;
        }
    }
    return 0;
}

static void kernel_float(void *vctx, size_t start, size_t end) {
    KernelCtx *c = vctx;
    if (kernel_fast(c, start, end)) {
        return;
    }
    for (size_t i = start; i < end; i++) {
        double x = c->aa ? elem_double(c->da, c->ca, c->oa + i) : c->sa;
        double y = c->ba ? elem_double(c->db, c->cb, c->ob + i) : c->sb;
        double r;
        switch (c->op) {
        case BIN_ADD:
            r = x + y;
            break;
        case BIN_SUB:
            r = x - y;
            break;
        case BIN_MUL:
            r = x * y;
            break;
        default:
            r = x / y;
            break;
        }
        store_double(c->out, c->rd, i, r);
    }
}

static void kernel_int(void *vctx, size_t start, size_t end) {
    KernelCtx *c = vctx;
    if (kernel_fast(c, start, end)) {
        return;
    }
    for (size_t i = start; i < end; i++) {
        int64_t x = c->aa ? elem_i64(c->da, c->ca, c->oa + i) : c->ia;
        int64_t y = c->ba ? elem_i64(c->db, c->cb, c->ob + i) : c->ib;
        int64_t r;
        switch (c->op) {
        case BIN_ADD:
            r = x + y;
            break;
        case BIN_SUB:
            r = x - y;
            break;
        default:
            r = x * y;
            break;
        }
        store_i64(c->out, c->rd, i, r);
    }
}

#define PARALLEL_MIN 8192

static void (*g_parallel_for)(void *, size_t, SetaeParallelBody) = NULL;

void setae_set_parallel_for(void (*fn)(void *, size_t, SetaeParallelBody)) {
    g_parallel_for = fn;
}

static void run_parallel(void *ctx, size_t n, SetaeParallelBody body) {
    if (n >= PARALLEL_MIN && g_parallel_for != NULL) {
        g_parallel_for(ctx, n, body);
        return;
    }
    body(ctx, 0, n);
}

static int scalar_class(SetaeVM *vm, SetaeValue v, uint8_t *out) {
    if (setae_is_int(v) || setae_obj_type(v) == SETAE_T_BIGINT || setae_is_bool(v)) {
        *out = DTYPE_I64;
        return 1;
    }
    if (setae_is_float(v)) {
        *out = DTYPE_F64;
        return 1;
    }
    (void)vm;
    return 0;
}

SetaeValue setae_array_binop(SetaeVM *vm, int op, SetaeValue a, SetaeValue b) {
    int aa = setae_obj_type(a) == SETAE_T_ARRAY;
    int ba = setae_obj_type(b) == SETAE_T_ARRAY;
    SetaeArray *arrA = aa ? setae_to_ptr(a) : NULL;
    SetaeArray *arrB = ba ? setae_to_ptr(b) : NULL;

    uint8_t ca, cb;
    if (aa) {
        ca = arrA->dtype;
    } else if (!scalar_class(vm, a, &ca)) {
        setae_vm_raise(vm, "TypeError", "unsupported operand type(s) for array op: '%s'",
                       setae_type_name(a));
        return setae_none();
    }
    if (ba) {
        cb = arrB->dtype;
    } else if (!scalar_class(vm, b, &cb)) {
        setae_vm_raise(vm, "TypeError", "unsupported operand type(s) for array op: '%s'",
                       setae_type_name(b));
        return setae_none();
    }

    uint32_t n;
    if (aa && ba) {
        if (arrA->len != arrB->len) {
            setae_vm_raise(vm, "ValueError", "operands could not be broadcast together");
            return setae_none();
        }
        n = arrA->len;
    } else {
        n = aa ? arrA->len : arrB->len;
    }

    uint8_t rd;
    if (op == BIN_DIV) {
        rd = DTYPE_F64;
    } else if (dtype_is_float(ca) || dtype_is_float(cb)) {
        rd = (ca == DTYPE_F64 || cb == DTYPE_F64) ? DTYPE_F64 : DTYPE_F32;
    } else {
        rd = (ca == DTYPE_I64 || cb == DTYPE_I64) ? DTYPE_I64 : DTYPE_I32;
    }

    SetaeBuffer *buf = setae_buffer_alloc(rd, n);
    if (buf == NULL) {
        setae_vm_raise(vm, "MemoryError", "out of memory");
        return setae_none();
    }
    void *out = setae_buffer_data(buf);
    void *da = aa ? setae_buffer_data(arrA->buf) : NULL;
    void *db = ba ? setae_buffer_data(arrB->buf) : NULL;
    uint32_t oa = aa ? arrA->offset : 0;
    uint32_t ob = ba ? arrB->offset : 0;

    int float_out = dtype_is_float(rd);
    if (float_out) {
        if (op != BIN_ADD && op != BIN_SUB && op != BIN_MUL && op != BIN_DIV) {
            setae_buffer_release(buf);
            setae_vm_raise(vm, "TypeError", "unsupported operator for arrays");
            return setae_none();
        }
    } else if (op != BIN_ADD && op != BIN_SUB && op != BIN_MUL) {
        setae_buffer_release(buf);
        setae_vm_raise(vm, "TypeError", "unsupported operator for integer arrays");
        return setae_none();
    }

    int ok = 1;
    KernelCtx ctx;
    ctx.out = out;
    ctx.da = da;
    ctx.db = db;
    ctx.oa = oa;
    ctx.ob = ob;
    ctx.rd = rd;
    ctx.ca = ca;
    ctx.cb = cb;
    ctx.aa = aa;
    ctx.ba = ba;
    ctx.op = op;
    ctx.sa = aa ? 0.0 : value_to_double(vm, a, &ok);
    ctx.sb = ba ? 0.0 : value_to_double(vm, b, &ok);
    if (!float_out) {
        ctx.ia = aa ? 0 : value_to_i64(vm, a, &ok);
        ctx.ib = ba ? 0 : value_to_i64(vm, b, &ok);
    }
    if (!ok) {
        setae_buffer_release(buf);
        return setae_none();
    }
    run_parallel(&ctx, n, float_out ? kernel_float : kernel_int);
    return setae_array_new(vm->heap, buf, rd, 0, n);
}

enum {
    RED_SUM,
    RED_PROD,
    RED_MIN,
    RED_MAX,
};

typedef struct {
    void *data;
    uint32_t offset;
    uint8_t dtype;
    int op;
    int is_float;
    atomic_flag lock;
    double facc;
    int64_t iacc;
} ReduceCtx;

static void reduce_body(void *vctx, size_t start, size_t end) {
    ReduceCtx *c = vctx;
    if (c->is_float) {
        double acc = elem_double(c->data, c->dtype, c->offset + start);
        if (c->op == RED_SUM) {
            acc = 0.0;
        } else if (c->op == RED_PROD) {
            acc = 1.0;
        }
        for (size_t i = start; i < end; i++) {
            double x = elem_double(c->data, c->dtype, c->offset + i);
            if (c->op == RED_SUM) {
                acc += x;
            } else if (c->op == RED_PROD) {
                acc *= x;
            } else if (c->op == RED_MIN) {
                if (x < acc) {
                    acc = x;
                }
            } else if (x > acc) {
                acc = x;
            }
        }
        while (atomic_flag_test_and_set(&c->lock)) {
        }
        if (c->op == RED_SUM) {
            c->facc += acc;
        } else if (c->op == RED_PROD) {
            c->facc *= acc;
        } else if (c->op == RED_MIN) {
            if (acc < c->facc) {
                c->facc = acc;
            }
        } else if (acc > c->facc) {
            c->facc = acc;
        }
        atomic_flag_clear(&c->lock);
        return;
    }
    int64_t acc = elem_i64(c->data, c->dtype, c->offset + start);
    if (c->op == RED_SUM) {
        acc = 0;
    } else if (c->op == RED_PROD) {
        acc = 1;
    }
    for (size_t i = start; i < end; i++) {
        int64_t x = elem_i64(c->data, c->dtype, c->offset + i);
        if (c->op == RED_SUM) {
            acc += x;
        } else if (c->op == RED_PROD) {
            acc *= x;
        } else if (c->op == RED_MIN) {
            if (x < acc) {
                acc = x;
            }
        } else if (x > acc) {
            acc = x;
        }
    }
    while (atomic_flag_test_and_set(&c->lock)) {
    }
    if (c->op == RED_SUM) {
        c->iacc += acc;
    } else if (c->op == RED_PROD) {
        c->iacc *= acc;
    } else if (c->op == RED_MIN) {
        if (acc < c->iacc) {
            c->iacc = acc;
        }
    } else if (acc > c->iacc) {
        c->iacc = acc;
    }
    atomic_flag_clear(&c->lock);
}

static SetaeValue array_reduce(SetaeVM *vm, SetaeArray *a, const char *name) {
    int op;
    if (strcmp(name, "sum") == 0) {
        op = RED_SUM;
    } else if (strcmp(name, "prod") == 0) {
        op = RED_PROD;
    } else if (strcmp(name, "min") == 0) {
        op = RED_MIN;
    } else {
        op = RED_MAX;
    }
    if (a->len == 0) {
        if (op == RED_SUM) {
            return setae_from_int(0);
        }
        if (op == RED_PROD) {
            return setae_from_int(1);
        }
        setae_vm_raise(vm, "ValueError", "%s() of an empty array", name);
        return setae_none();
    }
    ReduceCtx c;
    c.data = setae_buffer_data(a->buf);
    c.offset = a->offset;
    c.dtype = a->dtype;
    c.op = op;
    c.is_float = dtype_is_float(a->dtype);
    atomic_flag_clear(&c.lock);
    double seed_f = elem_double(c.data, c.dtype, c.offset);
    int64_t seed_i = elem_i64(c.data, c.dtype, c.offset);
    c.facc = op == RED_SUM ? 0.0 : (op == RED_PROD ? 1.0 : seed_f);
    c.iacc = op == RED_SUM ? 0 : (op == RED_PROD ? 1 : seed_i);

    if (c.is_float && (op == RED_SUM || op == RED_PROD)) {
        reduce_body(&c, 0, a->len);
    } else {
        run_parallel(&c, a->len, reduce_body);
    }
    return c.is_float ? setae_from_float(c.facc) : setae_int_from_i64(vm->heap, c.iacc);
}

static SetaeValue array_map(SetaeVM *vm, SetaeValue arr, SetaeValue fn) {
    SetaeArray *a = setae_to_ptr(arr);
    uint32_t n = a->len;
    SetaeValue lst = setae_list_new(vm->heap, n);
    setae_vm_push_tmp(vm, lst);
    int any_float = 0;
    for (uint32_t i = 0; i < n; i++) {
        SetaeValue x = setae_array_get(vm, arr, i);
        SetaeValue r = setae_call(vm, fn, &x, 1);
        if (vm->error) {
            setae_vm_pop_tmp(vm);
            return setae_none();
        }
        if (setae_is_float(r)) {
            any_float = 1;
        }
        setae_list_push(setae_to_ptr(lst), r);
    }
    uint8_t rd;
    if (dtype_is_float(a->dtype) || any_float) {
        rd = a->dtype == DTYPE_F32 ? DTYPE_F32 : DTYPE_F64;
    } else {
        rd = a->dtype;
    }
    SetaeBuffer *buf = setae_buffer_alloc(rd, n);
    if (buf == NULL) {
        setae_vm_pop_tmp(vm);
        setae_vm_raise(vm, "MemoryError", "out of memory");
        return setae_none();
    }
    void *out = setae_buffer_data(buf);
    SetaeList *l = setae_to_ptr(lst);
    for (uint32_t i = 0; i < n; i++) {
        int ok;
        if (dtype_is_float(rd)) {
            double x = value_to_double(vm, l->items[i], &ok);
            if (!ok) {
                setae_buffer_release(buf);
                setae_vm_pop_tmp(vm);
                return setae_none();
            }
            store_double(out, rd, i, x);
        } else {
            int64_t x = value_to_i64(vm, l->items[i], &ok);
            if (!ok) {
                setae_buffer_release(buf);
                setae_vm_pop_tmp(vm);
                return setae_none();
            }
            store_i64(out, rd, i, x);
        }
    }
    setae_vm_pop_tmp(vm);
    return setae_array_new(vm->heap, buf, rd, 0, n);
}

static SetaeValue array_filter(SetaeVM *vm, SetaeValue arr, SetaeValue pred) {
    SetaeArray *a = setae_to_ptr(arr);
    uint32_t n = a->len;
    uint32_t *keep = n ? malloc(n * sizeof(uint32_t)) : NULL;
    uint32_t count = 0;
    for (uint32_t i = 0; i < n; i++) {
        SetaeValue x = setae_array_get(vm, arr, i);
        SetaeValue r = setae_call(vm, pred, &x, 1);
        if (vm->error) {
            free(keep);
            return setae_none();
        }
        if (setae_truthy(r)) {
            keep[count++] = i;
        }
    }
    SetaeBuffer *buf = setae_buffer_alloc(a->dtype, count);
    if (buf == NULL) {
        free(keep);
        setae_vm_raise(vm, "MemoryError", "out of memory");
        return setae_none();
    }
    size_t sz = setae_dtype_size(a->dtype);
    char *src = (char *)setae_buffer_data(a->buf) + (size_t)a->offset * sz;
    char *dst = setae_buffer_data(buf);
    for (uint32_t k = 0; k < count; k++) {
        memcpy(dst + (size_t)k * sz, src + (size_t)keep[k] * sz, sz);
    }
    free(keep);
    return setae_array_new(vm->heap, buf, a->dtype, 0, count);
}

static SetaeValue array_reduce_fn(SetaeVM *vm, SetaeValue arr, SetaeValue fn,
                                  SetaeValue init, int has_init) {
    SetaeArray *a = setae_to_ptr(arr);
    uint32_t i = 0;
    SetaeValue acc;
    if (has_init) {
        acc = init;
    } else {
        if (a->len == 0) {
            setae_vm_raise(vm, "TypeError", "reduce() of an empty array with no initial value");
            return setae_none();
        }
        acc = setae_array_get(vm, arr, 0);
        i = 1;
    }
    setae_vm_push_tmp(vm, acc);
    for (; i < a->len; i++) {
        SetaeValue pair[2];
        pair[0] = acc;
        pair[1] = setae_array_get(vm, arr, i);
        acc = setae_call(vm, fn, pair, 2);
        if (vm->error) {
            setae_vm_pop_tmp(vm);
            return setae_none();
        }
        setae_vm_pop_tmp(vm);
        setae_vm_push_tmp(vm, acc);
    }
    setae_vm_pop_tmp(vm);
    return acc;
}

SetaeValue setae_array_method(SetaeVM *vm, SetaeValue arr, const char *name, SetaeValue *args,
                              int nargs, int *found) {
    *found = 1;
    if (strcmp(name, "map") == 0 || strcmp(name, "filter") == 0) {
        if (nargs != 1) {
            setae_vm_raise(vm, "TypeError", "%s() takes exactly one argument (%d given)",
                           name, nargs);
            return setae_none();
        }
        return strcmp(name, "map") == 0 ? array_map(vm, arr, args[0])
                                        : array_filter(vm, arr, args[0]);
    }
    if (strcmp(name, "reduce") == 0) {
        if (nargs != 1 && nargs != 2) {
            setae_vm_raise(vm, "TypeError", "reduce() takes one or two arguments (%d given)",
                           nargs);
            return setae_none();
        }
        return array_reduce_fn(vm, arr, args[0], nargs == 2 ? args[1] : setae_none(),
                               nargs == 2);
    }
    if (strcmp(name, "sum") == 0 || strcmp(name, "prod") == 0 || strcmp(name, "min") == 0 ||
        strcmp(name, "max") == 0) {
        if (nargs != 0) {
            setae_vm_raise(vm, "TypeError", "%s() takes no arguments", name);
            return setae_none();
        }
        return array_reduce(vm, setae_to_ptr(arr), name);
    }
    if (strcmp(name, "tolist") == 0) {
        SetaeArray *a = setae_to_ptr(arr);
        SetaeValue lst = setae_list_new(vm->heap, a->len);
        setae_vm_push_tmp(vm, lst);
        for (uint32_t i = 0; i < a->len; i++) {
            setae_list_push(setae_to_ptr(lst), setae_array_get(vm, arr, i));
        }
        setae_vm_pop_tmp(vm);
        return lst;
    }
    *found = 0;
    return setae_none();
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
