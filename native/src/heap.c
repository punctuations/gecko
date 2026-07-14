#include "internal.h"

#include <stdlib.h>
#include <string.h>

struct SetaeHeap {
    SetaeObject **objs;
    size_t count;
    size_t cap;
};

SetaeHeap *setae_heap_new(void) {
    SetaeHeap *h = calloc(1, sizeof(SetaeHeap));
    return h;
}

static void *heap_alloc(SetaeHeap *h, size_t size, SetaeType type) {
    SetaeObject *o = calloc(1, size);
    if (o == NULL) {
        return NULL;
    }
    o->type = (uint32_t)type;
    if (h->count == h->cap) {
        h->cap = h->cap ? h->cap * 2 : 16;
        h->objs = realloc(h->objs, h->cap * sizeof(SetaeObject *));
    }
    h->objs[h->count++] = o;
    return o;
}

static void obj_free(SetaeObject *o) {
    switch (o->type) {
    case SETAE_T_LIST:
        free(((SetaeList *)o)->items);
        break;
    case SETAE_T_DICT:
        free(((SetaeDict *)o)->entries);
        break;
    }
    free(o);
}

void setae_heap_destroy(SetaeHeap *h) {
    if (h == NULL) {
        return;
    }
    for (size_t i = 0; i < h->count; i++) {
        obj_free(h->objs[i]);
    }
    free(h->objs);
    free(h);
}

SetaeValue setae_str_new(SetaeHeap *h, const char *bytes, size_t len) {
    SetaeStr *s = heap_alloc(h, sizeof(SetaeStr) + len, SETAE_T_STR);
    s->len = (uint32_t)len;
    memcpy(s->data, bytes, len);
    return setae_from_ptr(s);
}

SetaeValue setae_builtin_new(SetaeHeap *h, SetaeCFunc fn, const char *name) {
    SetaeBuiltin *b = heap_alloc(h, sizeof(SetaeBuiltin), SETAE_T_BUILTIN);
    b->fn = fn;
    b->name = name;
    return setae_from_ptr(b);
}

SetaeValue setae_list_new(SetaeHeap *h, uint32_t cap) {
    SetaeList *l = heap_alloc(h, sizeof(SetaeList), SETAE_T_LIST);
    if (cap > 0) {
        l->cap = cap;
        l->items = malloc(cap * sizeof(SetaeValue));
    }
    return setae_from_ptr(l);
}

void setae_list_push(SetaeList *l, SetaeValue v) {
    if (l->len == l->cap) {
        l->cap = l->cap ? l->cap * 2 : 8;
        l->items = realloc(l->items, l->cap * sizeof(SetaeValue));
    }
    l->items[l->len++] = v;
}

SetaeValue setae_dict_new(SetaeHeap *h) {
    SetaeDict *d = heap_alloc(h, sizeof(SetaeDict), SETAE_T_DICT);
    return setae_from_ptr(d);
}

void setae_dict_push(SetaeDict *d, SetaeValue key, SetaeValue value) {
    if (d->len == d->cap) {
        d->cap = d->cap ? d->cap * 2 : 8;
        d->entries = realloc(d->entries, d->cap * sizeof(SetaeDictEntry));
    }
    d->entries[d->len].key = key;
    d->entries[d->len].value = value;
    d->len++;
}

SetaeValue setae_range_new(SetaeHeap *h, int64_t start, int64_t stop, int64_t step) {
    SetaeRange *r = heap_alloc(h, sizeof(SetaeRange), SETAE_T_RANGE);
    r->start = start;
    r->stop = stop;
    r->step = step;
    return setae_from_ptr(r);
}

SetaeValue setae_iter_new(SetaeHeap *h, SetaeValue target) {
    SetaeIter *it = heap_alloc(h, sizeof(SetaeIter), SETAE_T_ITER);
    it->target = target;
    return setae_from_ptr(it);
}

SetaeValue setae_func_new(SetaeHeap *h, const SetaeCode *code) {
    SetaeFunc *f = heap_alloc(h, sizeof(SetaeFunc), SETAE_T_FUNCTION);
    f->code = code;
    return setae_from_ptr(f);
}
