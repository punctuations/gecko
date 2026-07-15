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

    struct SetaeCode **children;
    uint32_t nchildren;
    uint32_t children_cap;

    char *fname;
    uint32_t nlocals;
    uint32_t nparams;
    uint32_t ncells;
    uint32_t nfrees;
};

SetaeCode *setae_code_new(void) {
    return calloc(1, sizeof(SetaeCode));
}

void setae_code_free(SetaeCode *c) {
    if (c == NULL) {
        return;
    }
    for (uint32_t i = 0; i < c->nchildren; i++) {
        setae_code_free(c->children[i]);
    }
    free(c->children);
    for (uint32_t i = 0; i < c->nnames; i++) {
        free(c->names[i]);
    }
    free(c->names);
    free(c->fname);
    free(c->consts);
    free(c->code);
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

void setae_code_set_nlocals(SetaeCode *c, uint32_t n) {
    c->nlocals = n;
}

void setae_code_set_nparams(SetaeCode *c, uint32_t n) {
    c->nparams = n;
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

uint32_t setae_code_nlocals(const SetaeCode *c) {
    return c->nlocals;
}

uint32_t setae_code_nparams(const SetaeCode *c) {
    return c->nparams;
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
