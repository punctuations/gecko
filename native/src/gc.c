#include "internal.h"

static void mark(SetaeValue v) {
    if (!setae_is_ptr(v)) {
        return;
    }
    SetaeObject *o = setae_to_ptr(v);
    if (o->gc & 2) {
        return;
    }
    o->gc |= 2;
    switch (o->type) {
    case SETAE_T_LIST: {
        SetaeList *l = (SetaeList *)o;
        for (uint32_t i = 0; i < l->len; i++) {
            mark(l->items[i]);
        }
        break;
    }
    case SETAE_T_DICT: {
        SetaeDict *d = (SetaeDict *)o;
        for (uint32_t i = 0; i < d->len; i++) {
            mark(d->entries[i].key);
            mark(d->entries[i].value);
        }
        break;
    }
    case SETAE_T_TUPLE: {
        SetaeTuple *t = (SetaeTuple *)o;
        for (uint32_t i = 0; i < t->len; i++) {
            mark(t->items[i]);
        }
        break;
    }
    case SETAE_T_ITER:
        mark(((SetaeIter *)o)->target);
        break;
    case SETAE_T_CELL:
        mark(((SetaeCell *)o)->value);
        break;
    case SETAE_T_EXC:
        mark(((SetaeExc *)o)->message);
        break;
    case SETAE_T_CLASS: {
        SetaeClass *c = (SetaeClass *)o;
        mark(c->name);
        mark(c->base);
        mark(c->dict);
        break;
    }
    case SETAE_T_INSTANCE: {
        SetaeInstance *i = (SetaeInstance *)o;
        mark(i->cls);
        mark(i->attrs);
        break;
    }
    case SETAE_T_BOUND: {
        SetaeBound *b = (SetaeBound *)o;
        mark(b->func);
        mark(b->self);
        break;
    }
    case SETAE_T_FUNCTION: {
        SetaeFunc *f = (SetaeFunc *)o;
        for (uint32_t i = 0; i < f->nfree; i++) {
            mark(f->cells[i]);
        }
        break;
    }
    }
}

static void mark_code(const SetaeCode *c) {
    const SetaeValue *consts = setae_code_consts(c);
    uint32_t n = setae_code_nconsts(c);
    for (uint32_t i = 0; i < n; i++) {
        mark(consts[i]);
    }
    for (uint32_t i = 0;; i++) {
        const SetaeCode *child = setae_code_child(c, i);
        if (child == NULL) {
            break;
        }
        mark_code(child);
    }
}

void setae_gc_collect(SetaeVM *vm) {
    for (size_t i = 0; i < vm->nglobals; i++) {
        mark(vm->globals[i].value);
    }
    for (const SetaeFrame *f = vm->frames; f != NULL; f = f->parent) {
        uint32_t n = f->fixed + (uint32_t)f->sp;
        for (uint32_t i = 0; i < n; i++) {
            mark(f->slots[i]);
        }
    }
    for (size_t i = 0; i < vm->ncodes; i++) {
        mark_code(vm->codes[i]);
    }
    for (int i = 0; i < vm->ntmp; i++) {
        mark(vm->tmp_roots[i]);
    }
    mark(vm->exc);
    setae_heap_sweep(vm->heap);
}
