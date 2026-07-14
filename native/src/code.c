#include "internal.h"

#include <stdlib.h>
#include <string.h>

struct GkCode {
    GkValue *consts;
    uint32_t nconsts;
    uint32_t consts_cap;

    char **names;
    uint32_t nnames;
    uint32_t names_cap;

    uint8_t *code;
    uint32_t ncode;
    uint32_t code_cap;

    uint32_t nlocals;
};

GkCode *gk_code_new(void) {
    return calloc(1, sizeof(GkCode));
}

void gk_code_free(GkCode *c) {
    if (c == NULL) {
        return;
    }
    for (uint32_t i = 0; i < c->nnames; i++) {
        free(c->names[i]);
    }
    free(c->names);
    free(c->consts);
    free(c->code);
    free(c);
}

uint32_t gk_code_add_const(GkCode *c, GkValue v) {
    if (c->nconsts == c->consts_cap) {
        c->consts_cap = c->consts_cap ? c->consts_cap * 2 : 8;
        c->consts = realloc(c->consts, c->consts_cap * sizeof(GkValue));
    }
    c->consts[c->nconsts] = v;
    return c->nconsts++;
}

uint32_t gk_code_add_name(GkCode *c, const char *name) {
    if (c->nnames == c->names_cap) {
        c->names_cap = c->names_cap ? c->names_cap * 2 : 8;
        c->names = realloc(c->names, c->names_cap * sizeof(char *));
    }
    size_t n = strlen(name) + 1;
    c->names[c->nnames] = malloc(n);
    memcpy(c->names[c->nnames], name, n);
    return c->nnames++;
}

void gk_code_emit(GkCode *c, uint8_t op, uint8_t arg) {
    if (c->ncode + 2 > c->code_cap) {
        c->code_cap = c->code_cap ? c->code_cap * 2 : 32;
        c->code = realloc(c->code, c->code_cap);
    }
    c->code[c->ncode++] = op;
    c->code[c->ncode++] = arg;
}

void gk_code_set_nlocals(GkCode *c, uint32_t n) {
    c->nlocals = n;
}

const GkValue *gk_code_consts(const GkCode *c) {
    return c->consts;
}

const char *gk_code_name(const GkCode *c, uint32_t i) {
    return c->names[i];
}

const uint8_t *gk_code_bytes(const GkCode *c, uint32_t *n) {
    *n = c->ncode;
    return c->code;
}

uint32_t gk_code_nlocals(const GkCode *c) {
    return c->nlocals;
}
