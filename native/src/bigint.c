#include "internal.h"

#include <stdlib.h>
#include <string.h>

typedef struct {
    const uint32_t *d;
    uint32_t len;
    int sign;
    uint32_t tmp[2];
} IntView;

static void view(SetaeValue v, IntView *iv) {
    if (setae_obj_type(v) == SETAE_T_BIGINT) {
        SetaeBigInt *b = setae_to_ptr(v);
        iv->d = b->limbs;
        iv->len = b->len;
        iv->sign = b->sign;
        return;
    }
    int64_t x = setae_is_bool(v) ? (setae_to_bool(v) ? 1 : 0) : (int64_t)setae_to_int(v);
    if (x == 0) {
        iv->sign = 0;
        iv->len = 0;
        iv->d = iv->tmp;
        return;
    }
    iv->sign = x < 0 ? -1 : 1;
    uint64_t u = x < 0 ? (uint64_t)(-(x + 1)) + 1 : (uint64_t)x;
    iv->tmp[0] = (uint32_t)(u & 0xffffffffu);
    iv->tmp[1] = (uint32_t)(u >> 32);
    iv->len = iv->tmp[1] ? 2 : 1;
    iv->d = iv->tmp;
}

int setae_is_integer(SetaeValue v) {
    return setae_is_int(v) || setae_is_bool(v) || setae_obj_type(v) == SETAE_T_BIGINT;
}

static uint32_t mag_norm(const uint32_t *a, uint32_t len) {
    while (len > 0 && a[len - 1] == 0) {
        len--;
    }
    return len;
}

static SetaeValue make_int(SetaeHeap *h, int sign, const uint32_t *mag, uint32_t len) {
    len = mag_norm(mag, len);
    if (len == 0) {
        return setae_from_int(0);
    }
    if (len == 1) {
        uint32_t m = mag[0];
        if (sign >= 0 && m <= 0x7fffffffu) {
            return setae_from_int((int32_t)m);
        }
        if (sign < 0 && m <= 0x80000000u) {
            return setae_from_int((int32_t)(-(int64_t)m));
        }
    }
    SetaeValue bv = setae_bigint_alloc(h, sign < 0 ? -1 : 1, len);
    memcpy(((SetaeBigInt *)setae_to_ptr(bv))->limbs, mag, len * sizeof(uint32_t));
    return bv;
}

static int mag_cmp(const uint32_t *a, uint32_t alen, const uint32_t *b, uint32_t blen) {
    alen = mag_norm(a, alen);
    blen = mag_norm(b, blen);
    if (alen != blen) {
        return alen < blen ? -1 : 1;
    }
    for (uint32_t i = alen; i > 0; i--) {
        if (a[i - 1] != b[i - 1]) {
            return a[i - 1] < b[i - 1] ? -1 : 1;
        }
    }
    return 0;
}

static uint32_t mag_add(const uint32_t *a, uint32_t alen, const uint32_t *b, uint32_t blen,
                        uint32_t *out) {
    if (alen < blen) {
        const uint32_t *t = a;
        a = b;
        b = t;
        uint32_t tl = alen;
        alen = blen;
        blen = tl;
    }
    uint64_t carry = 0;
    for (uint32_t i = 0; i < alen; i++) {
        uint64_t s = (uint64_t)a[i] + carry + (i < blen ? b[i] : 0);
        out[i] = (uint32_t)(s & 0xffffffffu);
        carry = s >> 32;
    }
    uint32_t len = alen;
    if (carry) {
        out[len++] = (uint32_t)carry;
    }
    return len;
}

static uint32_t mag_sub(const uint32_t *a, uint32_t alen, const uint32_t *b, uint32_t blen,
                        uint32_t *out) {
    int64_t borrow = 0;
    for (uint32_t i = 0; i < alen; i++) {
        int64_t s = (int64_t)a[i] - borrow - (i < blen ? b[i] : 0);
        if (s < 0) {
            s += 0x100000000LL;
            borrow = 1;
        } else {
            borrow = 0;
        }
        out[i] = (uint32_t)s;
    }
    return mag_norm(out, alen);
}

static uint32_t mag_mul(const uint32_t *a, uint32_t alen, const uint32_t *b, uint32_t blen,
                        uint32_t *out) {
    for (uint32_t i = 0; i < alen + blen; i++) {
        out[i] = 0;
    }
    for (uint32_t i = 0; i < alen; i++) {
        uint64_t carry = 0;
        for (uint32_t j = 0; j < blen; j++) {
            uint64_t s = (uint64_t)a[i] * b[j] + out[i + j] + carry;
            out[i + j] = (uint32_t)(s & 0xffffffffu);
            carry = s >> 32;
        }
        out[i + blen] += (uint32_t)carry;
    }
    return mag_norm(out, alen + blen);
}

static SetaeValue addsub(SetaeHeap *h, SetaeValue av, SetaeValue bv, int negate_b) {
    IntView a, b;
    view(av, &a);
    view(bv, &b);
    int bsign = negate_b ? -b.sign : b.sign;
    if (a.sign == 0) {
        return make_int(h, bsign, b.d, b.len);
    }
    if (bsign == 0) {
        return make_int(h, a.sign, a.d, a.len);
    }
    uint32_t cap = (a.len > b.len ? a.len : b.len) + 1;
    uint32_t *out = malloc(cap * sizeof(uint32_t));
    SetaeValue r;
    if (a.sign == bsign) {
        uint32_t len = mag_add(a.d, a.len, b.d, b.len, out);
        r = make_int(h, a.sign, out, len);
    } else {
        int c = mag_cmp(a.d, a.len, b.d, b.len);
        if (c == 0) {
            r = setae_from_int(0);
        } else if (c > 0) {
            uint32_t len = mag_sub(a.d, a.len, b.d, b.len, out);
            r = make_int(h, a.sign, out, len);
        } else {
            uint32_t len = mag_sub(b.d, b.len, a.d, a.len, out);
            r = make_int(h, bsign, out, len);
        }
    }
    free(out);
    return r;
}

SetaeValue setae_int_add(SetaeHeap *h, SetaeValue a, SetaeValue b) {
    return addsub(h, a, b, 0);
}

SetaeValue setae_int_sub(SetaeHeap *h, SetaeValue a, SetaeValue b) {
    return addsub(h, a, b, 1);
}

SetaeValue setae_int_mul(SetaeHeap *h, SetaeValue a, SetaeValue b) {
    IntView x, y;
    view(a, &x);
    view(b, &y);
    if (x.sign == 0 || y.sign == 0) {
        return setae_from_int(0);
    }
    uint32_t *out = malloc((x.len + y.len) * sizeof(uint32_t));
    uint32_t len = mag_mul(x.d, x.len, y.d, y.len, out);
    SetaeValue r = make_int(h, x.sign * y.sign, out, len);
    free(out);
    return r;
}

SetaeValue setae_int_neg(SetaeHeap *h, SetaeValue a) {
    IntView x;
    view(a, &x);
    return make_int(h, -x.sign, x.d, x.len);
}

static int get_bit(const uint32_t *a, uint32_t i) {
    return (a[i >> 5] >> (i & 31)) & 1;
}

static void mag_sub_inplace(uint32_t *r, uint32_t rn, const uint32_t *b, uint32_t blen) {
    int64_t borrow = 0;
    for (uint32_t i = 0; i < rn; i++) {
        int64_t s = (int64_t)r[i] - borrow - (i < blen ? b[i] : 0);
        if (s < 0) {
            s += 0x100000000LL;
            borrow = 1;
        } else {
            borrow = 0;
        }
        r[i] = (uint32_t)s;
    }
}

static void mag_divmod(const uint32_t *a, uint32_t alen, const uint32_t *b, uint32_t blen,
                       uint32_t *q, uint32_t *qlen, uint32_t *r, uint32_t *rlen) {
    alen = mag_norm(a, alen);
    blen = mag_norm(b, blen);
    memset(q, 0, alen * sizeof(uint32_t));
    uint32_t rn = blen + 1;
    memset(r, 0, rn * sizeof(uint32_t));
    for (uint32_t bit = alen * 32; bit-- > 0;) {
        uint32_t carry = (uint32_t)get_bit(a, bit);
        for (uint32_t k = 0; k < rn; k++) {
            uint32_t nc = r[k] >> 31;
            r[k] = (r[k] << 1) | carry;
            carry = nc;
        }
        if (mag_cmp(r, rn, b, blen) >= 0) {
            mag_sub_inplace(r, rn, b, blen);
            q[bit >> 5] |= (1u << (bit & 31));
        }
    }
    *qlen = mag_norm(q, alen);
    *rlen = mag_norm(r, rn);
}

int setae_int_sign(SetaeValue v) {
    IntView x;
    view(v, &x);
    return x.sign;
}

int setae_int_divmod(SetaeHeap *h, SetaeValue av, SetaeValue bv, SetaeValue *outq,
                     SetaeValue *outr) {
    IntView a, b;
    view(av, &a);
    view(bv, &b);
    if (b.sign == 0) {
        return 0;
    }
    if (a.sign == 0) {
        *outq = setae_from_int(0);
        *outr = setae_from_int(0);
        return 1;
    }
    uint32_t *q = malloc((a.len + 1) * sizeof(uint32_t));
    uint32_t *r = malloc((b.len + 2) * sizeof(uint32_t));
    uint32_t ql, rl;
    mag_divmod(a.d, a.len, b.d, b.len, q, &ql, r, &rl);
    SetaeValue qt = make_int(h, a.sign * b.sign, q, ql);
    SetaeValue rt = make_int(h, a.sign, r, rl);
    free(q);
    free(r);
    if (setae_int_sign(rt) != 0 && setae_int_sign(rt) != b.sign) {
        *outq = setae_int_sub(h, qt, setae_from_int(1));
        *outr = setae_int_add(h, rt, bv);
    } else {
        *outq = qt;
        *outr = rt;
    }
    return 1;
}

SetaeValue setae_int_pow(SetaeHeap *h, SetaeValue a, int64_t e) {
    SetaeValue result = setae_from_int(1);
    SetaeValue base = a;
    while (e > 0) {
        if (e & 1) {
            result = setae_int_mul(h, result, base);
        }
        e >>= 1;
        if (e > 0) {
            base = setae_int_mul(h, base, base);
        }
    }
    return result;
}

SetaeValue setae_int_lshift(SetaeHeap *h, SetaeValue a, int64_t n) {
    IntView x;
    view(a, &x);
    if (x.sign == 0 || n == 0) {
        return make_int(h, x.sign, x.d, x.len);
    }
    uint32_t words = (uint32_t)(n / 32);
    uint32_t bits = (uint32_t)(n % 32);
    uint32_t outlen = x.len + words + 1;
    uint32_t *out = calloc(outlen, sizeof(uint32_t));
    for (uint32_t i = 0; i < x.len; i++) {
        uint64_t v = (uint64_t)x.d[i] << bits;
        out[i + words] |= (uint32_t)(v & 0xffffffffu);
        out[i + words + 1] |= (uint32_t)(v >> 32);
    }
    SetaeValue r = make_int(h, x.sign, out, outlen);
    free(out);
    return r;
}

SetaeValue setae_int_rshift(SetaeHeap *h, SetaeValue a, int64_t n) {
    SetaeValue shift = setae_int_lshift(h, setae_from_int(1), n);
    SetaeValue q, r;
    setae_int_divmod(h, a, shift, &q, &r);
    return q;
}

int setae_int_cmp(SetaeValue av, SetaeValue bv) {
    IntView a, b;
    view(av, &a);
    view(bv, &b);
    if (a.sign != b.sign) {
        return a.sign < b.sign ? -1 : 1;
    }
    if (a.sign == 0) {
        return 0;
    }
    int c = mag_cmp(a.d, a.len, b.d, b.len);
    return a.sign > 0 ? c : -c;
}

int setae_int_fits_i64(SetaeValue v, int64_t *out) {
    if (setae_obj_type(v) != SETAE_T_BIGINT) {
        *out = setae_is_bool(v) ? (setae_to_bool(v) ? 1 : 0) : (int64_t)setae_to_int(v);
        return 1;
    }
    SetaeBigInt *b = setae_to_ptr(v);
    if (b->len > 2) {
        return 0;
    }
    uint64_t u = b->limbs[0];
    if (b->len == 2) {
        u |= (uint64_t)b->limbs[1] << 32;
    }
    if (b->sign > 0 && u <= 0x7fffffffffffffffULL) {
        *out = (int64_t)u;
        return 1;
    }
    if (b->sign < 0 && u <= 0x8000000000000000ULL) {
        *out = -(int64_t)u;
        return 1;
    }
    return 0;
}

double setae_int_to_double(SetaeValue v) {
    if (setae_obj_type(v) != SETAE_T_BIGINT) {
        return setae_is_bool(v) ? (setae_to_bool(v) ? 1.0 : 0.0) : (double)setae_to_int(v);
    }
    SetaeBigInt *b = setae_to_ptr(v);
    double d = 0.0;
    for (uint32_t i = b->len; i > 0; i--) {
        d = d * 4294967296.0 + b->limbs[i - 1];
    }
    return b->sign < 0 ? -d : d;
}

uint64_t setae_bigint_hash(SetaeValue v) {
    SetaeBigInt *b = setae_to_ptr(v);
    uint64_t h = 1469598103934665603ULL;
    for (uint32_t i = 0; i < b->len; i++) {
        h = (h ^ b->limbs[i]) * 1099511628211ULL;
    }
    return b->sign < 0 ? ~h : h;
}

static uint32_t mag_divmod_small(uint32_t *a, uint32_t len, uint32_t d) {
    uint64_t r = 0;
    for (uint32_t i = len; i > 0; i--) {
        uint64_t cur = (r << 32) | a[i - 1];
        a[i - 1] = (uint32_t)(cur / d);
        r = cur % d;
    }
    return (uint32_t)r;
}

char *setae_bigint_decimal(SetaeValue v, size_t *len) {
    SetaeBigInt *b = setae_to_ptr(v);
    uint32_t *work = malloc(b->len * sizeof(uint32_t));
    memcpy(work, b->limbs, b->len * sizeof(uint32_t));
    uint32_t wlen = b->len;
    char *groups = malloc((size_t)b->len * 10 + 16);
    size_t gi = 0;
    while (wlen > 0) {
        uint32_t rem = mag_divmod_small(work, wlen, 1000000000u);
        wlen = mag_norm(work, wlen);
        if (wlen > 0) {
            for (int k = 0; k < 9; k++) {
                groups[gi++] = (char)('0' + rem % 10);
                rem /= 10;
            }
        } else {
            do {
                groups[gi++] = (char)('0' + rem % 10);
                rem /= 10;
            } while (rem > 0);
        }
    }
    free(work);
    size_t total = gi + (b->sign < 0 ? 1 : 0);
    char *out = malloc(total);
    size_t o = 0;
    if (b->sign < 0) {
        out[o++] = '-';
    }
    for (size_t k = gi; k > 0; k--) {
        out[o++] = groups[k - 1];
    }
    free(groups);
    *len = total;
    return out;
}

SetaeValue setae_bigint_to_str(SetaeHeap *h, SetaeValue v) {
    size_t total;
    char *out = setae_bigint_decimal(v, &total);
    SetaeValue r = setae_str_new(h, out, total);
    free(out);
    return r;
}

SetaeValue setae_int_from_i64(SetaeHeap *h, int64_t x) {
    if (x >= INT32_MIN && x <= INT32_MAX) {
        return setae_from_int((int32_t)x);
    }
    int sign = x < 0 ? -1 : 1;
    uint64_t u = x < 0 ? (uint64_t)(-(x + 1)) + 1 : (uint64_t)x;
    uint32_t mag[2] = {(uint32_t)(u & 0xffffffffu), (uint32_t)(u >> 32)};
    return make_int(h, sign, mag, 2);
}

SetaeValue setae_int_from_decimal(SetaeHeap *h, const char *s, size_t n, int neg) {
    size_t start = 0;
    if (n > 0 && (s[0] == '-' || s[0] == '+')) {
        if (s[0] == '-') {
            neg = !neg;
        }
        start = 1;
    }
    uint32_t cap = (uint32_t)(n / 9 + 2);
    uint32_t *mag = calloc(cap, sizeof(uint32_t));
    uint32_t len = 0;
    for (size_t i = start; i < n; i++) {
        if (s[i] < '0' || s[i] > '9') {
            continue;
        }
        uint64_t carry = (uint64_t)(s[i] - '0');
        for (uint32_t j = 0; j < len; j++) {
            uint64_t cur = (uint64_t)mag[j] * 10 + carry;
            mag[j] = (uint32_t)(cur & 0xffffffffu);
            carry = cur >> 32;
        }
        if (carry) {
            mag[len++] = (uint32_t)carry;
        }
    }
    SetaeValue r = make_int(h, neg ? -1 : 1, mag, len);
    free(mag);
    return r;
}
