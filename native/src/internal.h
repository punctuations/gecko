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
    uint32_t *index;
    uint32_t index_cap;
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
    SetaeValue *defaults;
    uint32_t ndefaults;
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
    char *name;
} SetaeExcType;

typedef struct SetaeExc {
    SetaeObject obj;
    char *kind;
    SetaeValue message;
} SetaeExc;

typedef struct SetaeClass {
    SetaeObject obj;
    SetaeValue name;
    SetaeValue base;
    SetaeValue dict;
} SetaeClass;

typedef struct SetaeShape {
    struct SetaeShape *parent;
    char *name;
    uint32_t nslots;
    struct SetaeShape **kids;
    uint32_t nkids;
    uint32_t kids_cap;
} SetaeShape;

typedef struct SetaeInstance {
    SetaeObject obj;
    SetaeValue cls;
    SetaeShape *shape;
    SetaeValue *slots;
    uint32_t slots_cap;
} SetaeInstance;

typedef struct SetaeInlineCache {
    SetaeShape *shape;
    SetaeShape *next;
    SetaeValue cls;
    SetaeValue method;
    uint32_t slot;
    uint32_t guard;
    uint8_t kind;
} SetaeInlineCache;

typedef struct SetaeBound {
    SetaeObject obj;
    SetaeValue func;
    SetaeValue self;
} SetaeBound;

typedef struct SetaeSubject {
    SetaeObject obj;
    void *mailbox;
} SetaeSubject;

typedef struct SetaeGen {
    SetaeObject obj;
    const SetaeCode *code;
    SetaeValue *frame;
    uint32_t frame_cap;
    uint32_t fixed;
    int sp;
    uint32_t ip;
    SetaeValue module;
    SetaeValue retval;
    int resumed;
    int done;
} SetaeGen;

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
uint64_t setae_value_hash(SetaeValue v);
uint64_t setae_hash_bytes(const char *data, size_t len);
void setae_dict_index_add(SetaeDict *d, uint32_t entry);
int64_t setae_dict_index_get(const SetaeDict *d, SetaeValue key);
int64_t setae_dict_index_get_cstr(const SetaeDict *d, const char *name, size_t len);
int setae_dict_del(SetaeDict *d, SetaeValue key);
int setae_dict_del_cstr(SetaeDict *d, const char *name);
SetaeValue setae_range_new(SetaeHeap *h, int64_t start, int64_t stop, int64_t step);
SetaeValue setae_iter_new(SetaeHeap *h, SetaeValue target);
SetaeValue setae_func_new(SetaeHeap *h, const SetaeCode *code, const SetaeValue *cells,
                          uint32_t nfree, const SetaeValue *defaults, uint32_t ndefaults,
                          SetaeValue module);
SetaeValue setae_module_new(SetaeHeap *h, SetaeValue name, SetaeValue dict);
SetaeValue setae_cell_new(SetaeHeap *h);
SetaeValue setae_tuple_new(SetaeHeap *h, const SetaeValue *items, uint32_t n);
SetaeValue setae_exctype_new(SetaeHeap *h, const char *name);
SetaeValue setae_exc_new(SetaeHeap *h, const char *kind, SetaeValue message);
SetaeValue setae_class_new(SetaeHeap *h, SetaeValue name, SetaeValue base,
                           SetaeValue dict);
SetaeValue setae_instance_new(SetaeHeap *h, SetaeValue cls);
int setae_instance_get(const SetaeInstance *inst, const char *name, SetaeValue *out);
int64_t setae_instance_slot(const SetaeInstance *inst, const char *name);
void setae_instance_set(SetaeHeap *h, SetaeInstance *inst, const char *name, SetaeValue v);
SetaeValue setae_bound_new(SetaeHeap *h, SetaeValue func, SetaeValue self);
SetaeValue setae_gen_new(SetaeHeap *h, const SetaeCode *code, SetaeValue module);
int setae_gen_next(SetaeVM *vm, SetaeValue genv, SetaeValue sent, SetaeValue *out);

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
    uint32_t *globals_index;
    uint32_t globals_index_cap;

    SetaeGlobal *builtins;
    size_t nbuiltins;
    size_t builtins_cap;
    uint32_t *builtins_index;
    uint32_t builtins_index_cap;

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

    uint64_t steps;
    uint64_t step_limit;
    uint64_t time_limit_ns;
    uint64_t deadline_ns;
    int interrupted;
    int gc_disabled;
    SetaeValue oom;
    SetaeSandboxHook sandbox_hook;
    uint32_t class_version;

    SetaeValue **frame_pool;
    uint32_t *frame_pool_caps;
    size_t frame_pool_n;
    size_t frame_pool_cap;

    SetaeValue exc;
};

void setae_vm_register_builtin(SetaeVM *vm, const char *name, SetaeValue v);

void setae_heap_bind(SetaeHeap *h, SetaeVM *vm);
void setae_heap_sweep(SetaeHeap *h);

const SetaeValue *setae_code_consts(const SetaeCode *c);
uint32_t setae_code_nconsts(const SetaeCode *c);
const char *setae_code_name(const SetaeCode *c, uint32_t i);
const uint8_t *setae_code_bytes(const SetaeCode *c, uint32_t *n);
SetaeInlineCache *setae_code_ic(SetaeCode *c);
uint32_t setae_code_nlocals(const SetaeCode *c);
uint32_t setae_code_nparams(const SetaeCode *c);
uint32_t setae_code_ndefaults(const SetaeCode *c);
const char *setae_code_param_name(const SetaeCode *c, uint32_t i);
int setae_code_varargs(const SetaeCode *c);
int setae_code_kwargs(const SetaeCode *c);
int setae_code_generator(const SetaeCode *c);
uint32_t setae_code_ncells(const SetaeCode *c);
uint32_t setae_code_nfrees(const SetaeCode *c);
const SetaeExcEntry *setae_code_excs(const SetaeCode *c, uint32_t *n);
const char *setae_code_fname(const SetaeCode *c);
const SetaeCode *setae_code_child(const SetaeCode *c, uint32_t i);
const SetaeCode *setae_code_module(const SetaeCode *c, uint32_t i);
uint32_t setae_code_nmodules(const SetaeCode *c);
uint32_t setae_code_nnames(const SetaeCode *c);
uint32_t setae_code_nparam_names(const SetaeCode *c);
uint32_t setae_code_nchildren(const SetaeCode *c);
int32_t setae_code_module_parent(const SetaeCode *c);

void setae_vm_append_output(SetaeVM *vm, const char *bytes, size_t len);
SetaeValue setae_format_value(SetaeVM *vm, SetaeValue v, int repr_mode);
SetaeHeap *setae_vm_heap(SetaeVM *vm);
void setae_vm_raise(SetaeVM *vm, const char *kind, const char *fmt, ...);
void setae_vm_oom(SetaeVM *vm);

#endif
