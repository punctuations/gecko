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
    SetaeValue *cells;
    uint32_t nfree;
    SetaeValue module;
} SetaeFunc;

typedef struct SetaeModule {
    SetaeObject obj;
    SetaeValue name;
    SetaeValue dict;
} SetaeModule;

typedef struct SetaeCell {
    SetaeObject obj;
    SetaeValue value;
} SetaeCell;

typedef struct SetaeTuple {
    SetaeObject obj;
    uint32_t len;
    SetaeValue items[];
} SetaeTuple;

typedef struct SetaeExcType {
    SetaeObject obj;
    const char *name;
} SetaeExcType;

typedef struct SetaeExc {
    SetaeObject obj;
    const char *kind;
    SetaeValue message;
} SetaeExc;

typedef struct SetaeClass {
    SetaeObject obj;
    SetaeValue name;
    SetaeValue base;
    SetaeValue dict;
} SetaeClass;

typedef struct SetaeInstance {
    SetaeObject obj;
    SetaeValue cls;
    SetaeValue attrs;
} SetaeInstance;

typedef struct SetaeBound {
    SetaeObject obj;
    SetaeValue func;
    SetaeValue self;
} SetaeBound;

typedef struct SetaeExcEntry {
    uint32_t start;
    uint32_t end;
    uint32_t target;
    uint32_t depth;
} SetaeExcEntry;

SetaeValue setae_list_new(SetaeHeap *h, uint32_t cap);
void setae_list_push(SetaeList *l, SetaeValue v);
SetaeValue setae_dict_new(SetaeHeap *h);
void setae_dict_push(SetaeDict *d, SetaeValue key, SetaeValue value);
SetaeValue setae_range_new(SetaeHeap *h, int64_t start, int64_t stop, int64_t step);
SetaeValue setae_iter_new(SetaeHeap *h, SetaeValue target);
SetaeValue setae_func_new(SetaeHeap *h, const SetaeCode *code, const SetaeValue *cells,
                          uint32_t nfree, SetaeValue module);
SetaeValue setae_module_new(SetaeHeap *h, SetaeValue name, SetaeValue dict);
SetaeValue setae_cell_new(SetaeHeap *h);
SetaeValue setae_tuple_new(SetaeHeap *h, const SetaeValue *items, uint32_t n);
SetaeValue setae_exctype_new(SetaeHeap *h, const char *name);
SetaeValue setae_exc_new(SetaeHeap *h, const char *kind, SetaeValue message);
SetaeValue setae_class_new(SetaeHeap *h, SetaeValue name, SetaeValue base,
                           SetaeValue dict);
SetaeValue setae_instance_new(SetaeHeap *h, SetaeValue cls, SetaeValue attrs);
SetaeValue setae_bound_new(SetaeHeap *h, SetaeValue func, SetaeValue self);

void setae_vm_push_tmp(SetaeVM *vm, SetaeValue v);
void setae_vm_pop_tmp(SetaeVM *vm);

const char *setae_type_name(SetaeValue v);
int setae_value_eq(SetaeValue a, SetaeValue b);
int64_t setae_range_len(const SetaeRange *r);
size_t setae_str_count(SetaeValue v);

typedef struct SetaeGlobal {
    char *name;
    SetaeValue value;
} SetaeGlobal;

typedef struct SetaeFrame {
    SetaeValue *slots;
    uint32_t fixed;
    int sp;
    SetaeValue module;
    struct SetaeFrame *parent;
} SetaeFrame;

struct SetaeVM {
    SetaeHeap *heap;

    SetaeGlobal *globals;
    size_t nglobals;
    size_t globals_cap;

    SetaeGlobal *builtins;
    size_t nbuiltins;
    size_t builtins_cap;

    char *out;
    size_t out_len;
    size_t out_cap;

    int error;
    char errmsg[128];
    int depth;

    SetaeFrame *frames;

    const SetaeCode **codes;
    size_t ncodes;
    size_t codes_cap;

    const SetaeCode *root;
    SetaeValue *module_cache;
    uint32_t nmodules;

    SetaeValue tmp_roots[8];
    int ntmp;

    SetaeValue exc;
};

void setae_vm_register_builtin(SetaeVM *vm, const char *name, SetaeValue v);

void setae_heap_bind(SetaeHeap *h, SetaeVM *vm);
void setae_heap_sweep(SetaeHeap *h);

const SetaeValue *setae_code_consts(const SetaeCode *c);
uint32_t setae_code_nconsts(const SetaeCode *c);
const char *setae_code_name(const SetaeCode *c, uint32_t i);
const uint8_t *setae_code_bytes(const SetaeCode *c, uint32_t *n);
uint32_t setae_code_nlocals(const SetaeCode *c);
uint32_t setae_code_nparams(const SetaeCode *c);
uint32_t setae_code_ncells(const SetaeCode *c);
uint32_t setae_code_nfrees(const SetaeCode *c);
const SetaeExcEntry *setae_code_excs(const SetaeCode *c, uint32_t *n);
const char *setae_code_fname(const SetaeCode *c);
const SetaeCode *setae_code_child(const SetaeCode *c, uint32_t i);
const SetaeCode *setae_code_module(const SetaeCode *c, uint32_t i);
uint32_t setae_code_nmodules(const SetaeCode *c);
int32_t setae_code_module_parent(const SetaeCode *c);

void setae_vm_append_output(SetaeVM *vm, const char *bytes, size_t len);
SetaeHeap *setae_vm_heap(SetaeVM *vm);
void setae_vm_raise(SetaeVM *vm, const char *kind, const char *fmt, ...);

#endif
