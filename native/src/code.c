#include "internal.h"

#include <stdlib.h>
#include <string.h>

struct SetaeCode {
    SetaeValue *consts;
    uint32_t nconsts;
    uint32_t consts_cap;

    char **names;
    uint32_t nnames;
    uint32_t names_cap;

    uint8_t *code;
    uint32_t ncode;
    uint32_t code_cap;

    SetaeInlineCache *ic;

    struct SetaeCode **children;
    uint32_t nchildren;
    uint32_t children_cap;

    struct SetaeCode **modules;
    uint32_t nmodules;
    uint32_t modules_cap;

    SetaeExcEntry *excs;
    uint32_t nexcs;
    uint32_t excs_cap;

    char **param_names;
    uint32_t nparam_names;
    uint32_t param_names_cap;

    char *fname;
    uint32_t nlocals;
    uint32_t nparams;
    uint32_t ndefaults;
    uint32_t ncells;
    uint32_t nfrees;
    int varargs;
    int kwargs;
    int generator;
    int coroutine;
    int32_t module_parent;
};

SetaeCode *setae_code_new(void) {
    SetaeCode *c = calloc(1, sizeof(SetaeCode));
    c->module_parent = -1;
    return c;
}

void setae_code_free(SetaeCode *c) {
    if (c == NULL) {
        return;
    }
    for (uint32_t i = 0; i < c->nchildren; i++) {
        setae_code_free(c->children[i]);
    }
    free(c->children);
    for (uint32_t i = 0; i < c->nmodules; i++) {
        setae_code_free(c->modules[i]);
    }
    free(c->modules);
    for (uint32_t i = 0; i < c->nnames; i++) {
        free(c->names[i]);
    }
    free(c->names);
    for (uint32_t i = 0; i < c->nparam_names; i++) {
        free(c->param_names[i]);
    }
    free(c->param_names);
    free(c->fname);
    free(c->consts);
    free(c->excs);
    free(c->code);
    free(c->ic);
    free(c);
}

SetaeCode *setae_code_new_child(SetaeCode *parent) {
    if (parent->nchildren == parent->children_cap) {
        parent->children_cap = parent->children_cap ? parent->children_cap * 2 : 4;
        parent->children =
            realloc(parent->children, parent->children_cap * sizeof(SetaeCode *));
    }
    SetaeCode *child = setae_code_new();
    parent->children[parent->nchildren++] = child;
    return child;
}

SetaeCode *setae_code_new_module(SetaeCode *parent) {
    if (parent->nmodules == parent->modules_cap) {
        parent->modules_cap = parent->modules_cap ? parent->modules_cap * 2 : 4;
        parent->modules =
            realloc(parent->modules, parent->modules_cap * sizeof(SetaeCode *));
    }
    SetaeCode *m = setae_code_new();
    parent->modules[parent->nmodules++] = m;
    return m;
}

uint32_t setae_code_add_const(SetaeCode *c, SetaeValue v) {
    if (c->nconsts == c->consts_cap) {
        c->consts_cap = c->consts_cap ? c->consts_cap * 2 : 8;
        c->consts = realloc(c->consts, c->consts_cap * sizeof(SetaeValue));
    }
    c->consts[c->nconsts] = v;
    return c->nconsts++;
}

uint32_t setae_code_add_name(SetaeCode *c, const char *name) {
    if (c->nnames == c->names_cap) {
        c->names_cap = c->names_cap ? c->names_cap * 2 : 8;
        c->names = realloc(c->names, c->names_cap * sizeof(char *));
    }
    size_t n = strlen(name) + 1;
    c->names[c->nnames] = malloc(n);
    memcpy(c->names[c->nnames], name, n);
    return c->nnames++;
}

void setae_code_emit(SetaeCode *c, uint8_t op, uint8_t arg) {
    if (c->ncode + 2 > c->code_cap) {
        c->code_cap = c->code_cap ? c->code_cap * 2 : 32;
        c->code = realloc(c->code, c->code_cap);
    }
    c->code[c->ncode++] = op;
    c->code[c->ncode++] = arg;
}

void setae_code_add_exc(SetaeCode *c, uint32_t start, uint32_t end, uint32_t target,
                        uint32_t depth) {
    if (c->nexcs == c->excs_cap) {
        c->excs_cap = c->excs_cap ? c->excs_cap * 2 : 4;
        c->excs = realloc(c->excs, c->excs_cap * sizeof(SetaeExcEntry));
    }
    c->excs[c->nexcs].start = start;
    c->excs[c->nexcs].end = end;
    c->excs[c->nexcs].target = target;
    c->excs[c->nexcs].depth = depth;
    c->nexcs++;
}

const SetaeExcEntry *setae_code_excs(const SetaeCode *c, uint32_t *n) {
    *n = c->nexcs;
    return c->excs;
}

void setae_code_set_nlocals(SetaeCode *c, uint32_t n) {
    c->nlocals = n;
}

void setae_code_set_nparams(SetaeCode *c, uint32_t n) {
    c->nparams = n;
}

void setae_code_set_ndefaults(SetaeCode *c, uint32_t n) {
    c->ndefaults = n;
}

void setae_code_add_param_name(SetaeCode *c, const char *name) {
    if (c->nparam_names == c->param_names_cap) {
        c->param_names_cap = c->param_names_cap ? c->param_names_cap * 2 : 4;
        c->param_names = realloc(c->param_names, c->param_names_cap * sizeof(char *));
    }
    size_t n = strlen(name) + 1;
    c->param_names[c->nparam_names] = malloc(n);
    memcpy(c->param_names[c->nparam_names], name, n);
    c->nparam_names++;
}

void setae_code_set_variadic(SetaeCode *c, uint8_t varargs, uint8_t kwargs) {
    c->varargs = varargs;
    c->kwargs = kwargs;
}

void setae_code_set_generator(SetaeCode *c, uint8_t generator) {
    c->generator = generator;
}

int setae_code_generator(const SetaeCode *c) {
    return c->generator;
}

void setae_code_set_coroutine(SetaeCode *c, uint8_t coroutine) {
    c->coroutine = coroutine;
}

int setae_code_coroutine(const SetaeCode *c) {
    return c->coroutine;
}

const char *setae_code_param_name(const SetaeCode *c, uint32_t i) {
    return c->param_names[i];
}

int setae_code_varargs(const SetaeCode *c) {
    return c->varargs;
}

int setae_code_kwargs(const SetaeCode *c) {
    return c->kwargs;
}

void setae_code_set_ncells(SetaeCode *c, uint32_t n) {
    c->ncells = n;
}

void setae_code_set_nfrees(SetaeCode *c, uint32_t n) {
    c->nfrees = n;
}

void setae_code_set_name(SetaeCode *c, const char *name) {
    free(c->fname);
    size_t n = strlen(name) + 1;
    c->fname = malloc(n);
    memcpy(c->fname, name, n);
}

const SetaeValue *setae_code_consts(const SetaeCode *c) {
    return c->consts;
}

uint32_t setae_code_nconsts(const SetaeCode *c) {
    return c->nconsts;
}

const char *setae_code_name(const SetaeCode *c, uint32_t i) {
    return c->names[i];
}

const uint8_t *setae_code_bytes(const SetaeCode *c, uint32_t *n) {
    *n = c->ncode;
    return c->code;
}

SetaeInlineCache *setae_code_ic(SetaeCode *c) {
    if (c->ic == NULL && c->ncode > 0) {
        c->ic = calloc(c->ncode / 2, sizeof(SetaeInlineCache));
    }
    return c->ic;
}

uint32_t setae_code_nlocals(const SetaeCode *c) {
    return c->nlocals;
}

uint32_t setae_code_nparams(const SetaeCode *c) {
    return c->nparams;
}

uint32_t setae_code_ndefaults(const SetaeCode *c) {
    return c->ndefaults;
}

uint32_t setae_code_ncells(const SetaeCode *c) {
    return c->ncells;
}

uint32_t setae_code_nfrees(const SetaeCode *c) {
    return c->nfrees;
}

const char *setae_code_fname(const SetaeCode *c) {
    return c->fname ? c->fname : "<module>";
}

const SetaeCode *setae_code_child(const SetaeCode *c, uint32_t i) {
    return i < c->nchildren ? c->children[i] : NULL;
}

const SetaeCode *setae_code_module(const SetaeCode *c, uint32_t i) {
    return i < c->nmodules ? c->modules[i] : NULL;
}

uint32_t setae_code_nmodules(const SetaeCode *c) {
    return c->nmodules;
}

uint32_t setae_code_nnames(const SetaeCode *c) {
    return c->nnames;
}

uint32_t setae_code_nparam_names(const SetaeCode *c) {
    return c->nparam_names;
}

uint32_t setae_code_nchildren(const SetaeCode *c) {
    return c->nchildren;
}

void setae_code_set_module_parent(SetaeCode *c, int32_t parent) {
    c->module_parent = parent;
}

int32_t setae_code_module_parent(const SetaeCode *c) {
    return c->module_parent;
}
