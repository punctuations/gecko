#include "internal.h"

static SetaeValue f64_bits(double d) {
    union { double d; uint64_t u; } x;
    x.d = d;
    return x.u;
}

static double bits_f64(SetaeValue v) {
    union { uint64_t u; double d; } x;
    x.u = v;
    return x.d;
}

int setae_is_int(SetaeValue v) {
    return (v & SETAE_NUMBER_TAG) == SETAE_NUMBER_TAG;
}

int setae_is_float(SetaeValue v) {
    return (v & SETAE_NUMBER_TAG) != 0 && (v & SETAE_NUMBER_TAG) != SETAE_NUMBER_TAG;
}

int setae_is_ptr(SetaeValue v) {
    return (v & SETAE_NUMBER_TAG) == 0 && (v & 0x7) == 0 && v != 0;
}

int setae_is_none(SetaeValue v) {
    return v == SETAE_VAL_NONE;
}

int setae_is_bool(SetaeValue v) {
    return v == SETAE_VAL_TRUE || v == SETAE_VAL_FALSE;
}

SetaeValue setae_from_float(double d) {
    SetaeValue bits = (d != d) ? 0x7ff8000000000000ULL : f64_bits(d);
    return bits + SETAE_DOUBLE_OFFSET;
}

double setae_to_float(SetaeValue v) {
    return bits_f64(v - SETAE_DOUBLE_OFFSET);
}

SetaeValue setae_from_int(int32_t i) {
    return SETAE_NUMBER_TAG | (uint32_t)i;
}

int32_t setae_to_int(SetaeValue v) {
    return (int32_t)(uint32_t)v;
}

SetaeValue setae_none(void) {
    return SETAE_VAL_NONE;
}

SetaeValue setae_bool(int b) {
    return b ? SETAE_VAL_TRUE : SETAE_VAL_FALSE;
}

int setae_to_bool(SetaeValue v) {
    return v == SETAE_VAL_TRUE;
}

SetaeValue setae_from_ptr(void *p) {
    return (SetaeValue)(uintptr_t)p;
}

void *setae_to_ptr(SetaeValue v) {
    return (void *)(uintptr_t)v;
}

int setae_obj_type(SetaeValue v) {
    if (!setae_is_ptr(v)) {
        return -1;
    }
    return (int)((SetaeObject *)setae_to_ptr(v))->type;
}

int setae_is_str(SetaeValue v) {
    return setae_obj_type(v) == SETAE_T_STR;
}

size_t setae_str_len(SetaeValue v) {
    return ((SetaeStr *)setae_to_ptr(v))->len;
}

const char *setae_str_data(SetaeValue v) {
    return ((SetaeStr *)setae_to_ptr(v))->data;
}

const char *setae_type_name(SetaeValue v) {
    if (setae_is_int(v)) {
        return "int";
    }
    if (setae_is_float(v)) {
        return "float";
    }
    if (setae_is_bool(v)) {
        return "bool";
    }
    if (setae_is_none(v)) {
        return "NoneType";
    }
    switch (setae_obj_type(v)) {
    case SETAE_T_BIGINT:
        return "int";
    case SETAE_T_STR:
        return "str";
    case SETAE_T_LIST:
        return "list";
    case SETAE_T_DICT:
        return "dict";
    case SETAE_T_TUPLE:
        return "tuple";
    case SETAE_T_FUNCTION:
        return "function";
    case SETAE_T_CODE:
        return "code";
    case SETAE_T_MODULE:
        return "module";
    case SETAE_T_BUILTIN:
        return "builtin_function_or_method";
    case SETAE_T_RANGE:
        return "range";
    case SETAE_T_ITER:
        return "iterator";
    case SETAE_T_CELL:
        return "cell";
    case SETAE_T_EXCTYPE:
        return "type";
    case SETAE_T_EXC:
        return ((SetaeExc *)setae_to_ptr(v))->kind;
    case SETAE_T_CLASS:
        return "type";
    case SETAE_T_INSTANCE:
        return "instance";
    case SETAE_T_BOUND:
        return "method";
    default:
        return "object";
    }
}

static int str_eq(SetaeValue a, SetaeValue b) {
    size_t n = setae_str_len(a);
    if (n != setae_str_len(b)) {
        return 0;
    }
    const char *pa = setae_str_data(a);
    const char *pb = setae_str_data(b);
    for (size_t i = 0; i < n; i++) {
        if (pa[i] != pb[i]) {
            return 0;
        }
    }
    return 1;
}

int setae_value_eq(SetaeValue a, SetaeValue b) {
    int an = setae_is_int(a) || setae_is_float(a);
    int bn = setae_is_int(b) || setae_is_float(b);
    if (an && bn) {
        double x = setae_is_int(a) ? (double)setae_to_int(a) : setae_to_float(a);
        double y = setae_is_int(b) ? (double)setae_to_int(b) : setae_to_float(b);
        return x == y;
    }
    if (setae_is_str(a) && setae_is_str(b)) {
        return str_eq(a, b);
    }
    if (setae_obj_type(a) == SETAE_T_LIST && setae_obj_type(b) == SETAE_T_LIST) {
        SetaeList *la = setae_to_ptr(a);
        SetaeList *lb = setae_to_ptr(b);
        if (la->len != lb->len) {
            return 0;
        }
        for (uint32_t i = 0; i < la->len; i++) {
            if (!setae_value_eq(la->items[i], lb->items[i])) {
                return 0;
            }
        }
        return 1;
    }
    if (setae_obj_type(a) == SETAE_T_TUPLE && setae_obj_type(b) == SETAE_T_TUPLE) {
        SetaeTuple *ta = setae_to_ptr(a);
        SetaeTuple *tb = setae_to_ptr(b);
        if (ta->len != tb->len) {
            return 0;
        }
        for (uint32_t i = 0; i < ta->len; i++) {
            if (!setae_value_eq(ta->items[i], tb->items[i])) {
                return 0;
            }
        }
        return 1;
    }
    if (setae_obj_type(a) == SETAE_T_DICT && setae_obj_type(b) == SETAE_T_DICT) {
        SetaeDict *da = setae_to_ptr(a);
        SetaeDict *db = setae_to_ptr(b);
        if (da->len != db->len) {
            return 0;
        }
        for (uint32_t i = 0; i < da->len; i++) {
            uint32_t j = 0;
            for (; j < db->len; j++) {
                if (setae_value_eq(da->entries[i].key, db->entries[j].key)) {
                    break;
                }
            }
            if (j == db->len || !setae_value_eq(da->entries[i].value, db->entries[j].value)) {
                return 0;
            }
        }
        return 1;
    }
    return a == b;
}

int64_t setae_range_len(const SetaeRange *r) {
    if (r->step > 0) {
        if (r->start >= r->stop) {
            return 0;
        }
        return (r->stop - r->start + r->step - 1) / r->step;
    }
    if (r->start <= r->stop) {
        return 0;
    }
    return (r->start - r->stop - r->step - 1) / -r->step;
}

size_t setae_str_count(SetaeValue v) {
    size_t n = setae_str_len(v);
    const char *p = setae_str_data(v);
    size_t count = 0;
    for (size_t i = 0; i < n; i++) {
        if (((unsigned char)p[i] & 0xc0) != 0x80) {
            count++;
        }
    }
    return count;
}
