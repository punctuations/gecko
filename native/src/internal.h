#ifndef SETAE_INTERNAL_H
#define SETAE_INTERNAL_H

#include "setae.h"

typedef struct SetaeList {
    SetaeObject obj;
    uint32_t len;
    uint32_t cap;
    SetaeValue *items;
} SetaeList;

typedef struct SetaeDictEntry {
    SetaeValue key;
    SetaeValue value;
} SetaeDictEntry;

typedef struct SetaeDict {
    SetaeObject obj;
    uint32_t len;
    uint32_t cap;
    SetaeDictEntry *entries;
} SetaeDict;

typedef struct SetaeRange {
    SetaeObject obj;
    int64_t start;
    int64_t stop;
    int64_t step;
} SetaeRange;

typedef struct SetaeIter {
    SetaeObject obj;
    SetaeValue target;
    uint64_t index;
} SetaeIter;

typedef struct SetaeFunc {
    SetaeObject obj;
    const SetaeCode *code;
} SetaeFunc;

SetaeValue setae_list_new(SetaeHeap *h, uint32_t cap);
void setae_list_push(SetaeList *l, SetaeValue v);
SetaeValue setae_dict_new(SetaeHeap *h);
void setae_dict_push(SetaeDict *d, SetaeValue key, SetaeValue value);
SetaeValue setae_range_new(SetaeHeap *h, int64_t start, int64_t stop, int64_t step);
SetaeValue setae_iter_new(SetaeHeap *h, SetaeValue target);
SetaeValue setae_func_new(SetaeHeap *h, const SetaeCode *code);

const char *setae_type_name(SetaeValue v);
int setae_value_eq(SetaeValue a, SetaeValue b);
int64_t setae_range_len(const SetaeRange *r);
size_t setae_str_count(SetaeValue v);

const SetaeValue *setae_code_consts(const SetaeCode *c);
const char *setae_code_name(const SetaeCode *c, uint32_t i);
const uint8_t *setae_code_bytes(const SetaeCode *c, uint32_t *n);
uint32_t setae_code_nlocals(const SetaeCode *c);
uint32_t setae_code_nparams(const SetaeCode *c);
const char *setae_code_fname(const SetaeCode *c);
const SetaeCode *setae_code_child(const SetaeCode *c, uint32_t i);

void setae_vm_append_output(SetaeVM *vm, const char *bytes, size_t len);
SetaeHeap *setae_vm_heap(SetaeVM *vm);
void setae_vm_failf(SetaeVM *vm, const char *fmt, ...);

#endif /* SETAE_INTERNAL_H */
