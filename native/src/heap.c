#include "gecko.h"

#include <stdlib.h>
#include <string.h>

struct GkHeap {
    GkObject **objs;
    size_t count;
    size_t cap;
};

GkHeap *gk_heap_new(void) {
    GkHeap *h = calloc(1, sizeof(GkHeap));
    return h;
}

static void *heap_alloc(GkHeap *h, size_t size, GkType type) {
    GkObject *o = calloc(1, size);
    if (o == NULL) {
        return NULL;
    }
    o->type = (uint32_t)type;
    if (h->count == h->cap) {
        h->cap = h->cap ? h->cap * 2 : 16;
        h->objs = realloc(h->objs, h->cap * sizeof(GkObject *));
    }
    h->objs[h->count++] = o;
    return o;
}

void gk_heap_destroy(GkHeap *h) {
    if (h == NULL) {
        return;
    }
    for (size_t i = 0; i < h->count; i++) {
        free(h->objs[i]);
    }
    free(h->objs);
    free(h);
}

GkValue gk_str_new(GkHeap *h, const char *bytes, size_t len) {
    GkStr *s = heap_alloc(h, sizeof(GkStr) + len, GK_T_STR);
    s->len = (uint32_t)len;
    memcpy(s->data, bytes, len);
    return gk_from_ptr(s);
}

GkValue gk_builtin_new(GkHeap *h, GkCFunc fn, const char *name) {
    GkBuiltin *b = heap_alloc(h, sizeof(GkBuiltin), GK_T_BUILTIN);
    b->fn = fn;
    b->name = name;
    return gk_from_ptr(b);
}
