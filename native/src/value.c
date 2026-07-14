#include "gecko.h"

static GkValue f64_bits(double d) {
    union { double d; uint64_t u; } x;
    x.d = d;
    return x.u;
}

static double bits_f64(GkValue v) {
    union { uint64_t u; double d; } x;
    x.u = v;
    return x.d;
}

int gk_is_int(GkValue v) {
    return (v & GK_NUMBER_TAG) == GK_NUMBER_TAG;
}

int gk_is_float(GkValue v) {
    return (v & GK_NUMBER_TAG) != 0 && (v & GK_NUMBER_TAG) != GK_NUMBER_TAG;
}

int gk_is_ptr(GkValue v) {
    return (v & GK_NUMBER_TAG) == 0 && (v & 0x7) == 0 && v != 0;
}

int gk_is_none(GkValue v) {
    return v == GK_VAL_NONE;
}

int gk_is_bool(GkValue v) {
    return v == GK_VAL_TRUE || v == GK_VAL_FALSE;
}

GkValue gk_from_float(double d) {
    /* Canonicalize NaN so no double maps onto a tag pattern. */
    GkValue bits = (d != d) ? 0x7ff8000000000000ULL : f64_bits(d);
    return bits + GK_DOUBLE_OFFSET;
}

double gk_to_float(GkValue v) {
    return bits_f64(v - GK_DOUBLE_OFFSET);
}

GkValue gk_from_int(int32_t i) {
    return GK_NUMBER_TAG | (uint32_t)i;
}

int32_t gk_to_int(GkValue v) {
    return (int32_t)(uint32_t)v;
}

GkValue gk_none(void) {
    return GK_VAL_NONE;
}

GkValue gk_bool(int b) {
    return b ? GK_VAL_TRUE : GK_VAL_FALSE;
}

int gk_to_bool(GkValue v) {
    return v == GK_VAL_TRUE;
}

GkValue gk_from_ptr(void *p) {
    return (GkValue)(uintptr_t)p;
}

void *gk_to_ptr(GkValue v) {
    return (void *)(uintptr_t)v;
}

int gk_obj_type(GkValue v) {
    if (!gk_is_ptr(v)) {
        return -1;
    }
    return (int)((GkObject *)gk_to_ptr(v))->type;
}

int gk_is_str(GkValue v) {
    return gk_obj_type(v) == GK_T_STR;
}

size_t gk_str_len(GkValue v) {
    return ((GkStr *)gk_to_ptr(v))->len;
}

const char *gk_str_data(GkValue v) {
    return ((GkStr *)gk_to_ptr(v))->data;
}
