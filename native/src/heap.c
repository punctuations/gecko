#include "internal.h"

#include <stdlib.h>
#include <string.h>

#define GC_MIN_THRESHOLD 1024

static void obj_free(SetaeObject *o);

struct SetaeHeap {
    SetaeObject **objs;
    size_t count;
    size_t cap;
    size_t threshold;
    SetaeVM *vm;
};

SetaeHeap *setae_heap_new(void) {
    SetaeHeap *h = calloc(1, sizeof(SetaeHeap));
    h->threshold = GC_MIN_THRESHOLD;
    return h;
}

void setae_heap_bind(SetaeHeap *h, SetaeVM *vm) {
    h->vm = vm;
}

size_t setae_heap_live(const SetaeHeap *h) {
    return h->count;
}

static void *heap_alloc(SetaeHeap *h, size_t size, SetaeType type) {
    if (h->vm != NULL && h->vm->depth > 0 && h->count >= h->threshold) {
        setae_gc_collect(h->vm);
    }
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

void setae_heap_sweep(SetaeHeap *h) {
    size_t live = 0;
    for (size_t i = 0; i < h->count; i++) {
        if (h->objs[i]->gc & 2) {
            h->objs[i]->gc &= ~2u;
            h->objs[live++] = h->objs[i];
        } else {
            obj_free(h->objs[i]);
        }
    }
    h->count = live;
    h->threshold = live * 2 > GC_MIN_THRESHOLD ? live * 2 : GC_MIN_THRESHOLD;
}

static void obj_free(SetaeObject *o) {
    switch (o->type) {
    case SETAE_T_LIST:
        free(((SetaeList *)o)->items);
        break;
    case SETAE_T_DICT:
        free(((SetaeDict *)o)->entries);
        break;
    case SETAE_T_FUNCTION:
        free(((SetaeFunc *)o)->cells);
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

SetaeValue setae_func_new(SetaeHeap *h, const SetaeCode *code, const SetaeValue *cells,
                          uint32_t nfree, SetaeValue module) {
    SetaeFunc *f = heap_alloc(h, sizeof(SetaeFunc), SETAE_T_FUNCTION);
    f->code = code;
    f->nfree = nfree;
    f->module = module;
    if (nfree > 0) {
        f->cells = malloc(nfree * sizeof(SetaeValue));
        memcpy(f->cells, cells, nfree * sizeof(SetaeValue));
    }
    return setae_from_ptr(f);
}

SetaeValue setae_module_new(SetaeHeap *h, SetaeValue name, SetaeValue dict) {
    SetaeModule *m = heap_alloc(h, sizeof(SetaeModule), SETAE_T_MODULE);
    m->name = name;
    m->dict = dict;
    return setae_from_ptr(m);
}

SetaeValue setae_cell_new(SetaeHeap *h) {
    SetaeCell *c = heap_alloc(h, sizeof(SetaeCell), SETAE_T_CELL);
    return setae_from_ptr(c);
}

SetaeValue setae_exctype_new(SetaeHeap *h, const char *name) {
    SetaeExcType *t = heap_alloc(h, sizeof(SetaeExcType), SETAE_T_EXCTYPE);
    t->name = name;
    return setae_from_ptr(t);
}

SetaeValue setae_exc_new(SetaeHeap *h, const char *kind, SetaeValue message) {
    SetaeExc *e = heap_alloc(h, sizeof(SetaeExc), SETAE_T_EXC);
    e->kind = kind;
    e->message = message;
    return setae_from_ptr(e);
}

SetaeValue setae_class_new(SetaeHeap *h, SetaeValue name, SetaeValue base,
                           SetaeValue dict) {
    SetaeClass *c = heap_alloc(h, sizeof(SetaeClass), SETAE_T_CLASS);
    c->name = name;
    c->base = base;
    c->dict = dict;
    return setae_from_ptr(c);
}

SetaeValue setae_instance_new(SetaeHeap *h, SetaeValue cls, SetaeValue attrs) {
    SetaeInstance *i = heap_alloc(h, sizeof(SetaeInstance), SETAE_T_INSTANCE);
    i->cls = cls;
    i->attrs = attrs;
    return setae_from_ptr(i);
}

SetaeValue setae_bound_new(SetaeHeap *h, SetaeValue func, SetaeValue self) {
    SetaeBound *b = heap_alloc(h, sizeof(SetaeBound), SETAE_T_BOUND);
    b->func = func;
    b->self = self;
    return setae_from_ptr(b);
}

SetaeValue setae_tuple_new(SetaeHeap *h, const SetaeValue *items, uint32_t n) {
    SetaeTuple *t =
        heap_alloc(h, sizeof(SetaeTuple) + n * sizeof(SetaeValue), SETAE_T_TUPLE);
    t->len = n;
    if (n > 0 && items != NULL) {
        memcpy(t->items, items, n * sizeof(SetaeValue));
    }
    return setae_from_ptr(t);
}
