#include "internal.h"

#include <stdlib.h>
#include <string.h>

typedef enum {
    MSG_NONE,
    MSG_BOOL,
    MSG_INT,
    MSG_FLOAT,
    MSG_STR,
    MSG_LIST,
    MSG_TUPLE,
    MSG_DICT,
} SetaeMsgTag;

typedef struct {
    SetaeMsgTag tag;
    union {
        int b;
        int32_t i;
        double f;
        struct {
            char *data;
            uint32_t len;
        } str;
        struct {
            uint32_t *items;
            uint32_t len;
        } seq;
        struct {
            uint32_t *keys;
            uint32_t *vals;
            uint32_t len;
        } dict;
    } as;
} SetaeMsgNode;

struct SetaeMsg {
    SetaeMsgNode *nodes;
    uint32_t nnodes;
    uint32_t cap;
    uint32_t root;
};

typedef struct {
    SetaeValue *keys;
    uint32_t *idx;
    uint32_t len;
    uint32_t cap;
} IdMap;

static uint32_t msg_add(SetaeMsg *m) {
    if (m->nnodes == m->cap) {
        m->cap = m->cap ? m->cap * 2 : 16;
        m->nodes = realloc(m->nodes, m->cap * sizeof(SetaeMsgNode));
    }
    memset(&m->nodes[m->nnodes], 0, sizeof(SetaeMsgNode));
    return m->nnodes++;
}

static int idmap_find(IdMap *map, SetaeValue v, uint32_t *out) {
    for (uint32_t i = 0; i < map->len; i++) {
        if (map->keys[i] == v) {
            *out = map->idx[i];
            return 1;
        }
    }
    return 0;
}

static void idmap_put(IdMap *map, SetaeValue v, uint32_t idx) {
    if (map->len == map->cap) {
        map->cap = map->cap ? map->cap * 2 : 16;
        map->keys = realloc(map->keys, map->cap * sizeof(SetaeValue));
        map->idx = realloc(map->idx, map->cap * sizeof(uint32_t));
    }
    map->keys[map->len] = v;
    map->idx[map->len] = idx;
    map->len++;
}

static int64_t msg_read(SetaeMsg *m, IdMap *map, SetaeVM *vm, SetaeValue v) {
    if (setae_is_none(v)) {
        uint32_t n = msg_add(m);
        m->nodes[n].tag = MSG_NONE;
        return n;
    }
    if (setae_is_bool(v)) {
        uint32_t n = msg_add(m);
        m->nodes[n].tag = MSG_BOOL;
        m->nodes[n].as.b = setae_to_bool(v);
        return n;
    }
    if (setae_is_int(v)) {
        uint32_t n = msg_add(m);
        m->nodes[n].tag = MSG_INT;
        m->nodes[n].as.i = setae_to_int(v);
        return n;
    }
    if (setae_is_float(v)) {
        uint32_t n = msg_add(m);
        m->nodes[n].tag = MSG_FLOAT;
        m->nodes[n].as.f = setae_to_float(v);
        return n;
    }
    int t = setae_obj_type(v);
    if (t == SETAE_T_STR) {
        uint32_t n = msg_add(m);
        uint32_t len = (uint32_t)setae_str_len(v);
        m->nodes[n].tag = MSG_STR;
        m->nodes[n].as.str.len = len;
        m->nodes[n].as.str.data = malloc(len);
        memcpy(m->nodes[n].as.str.data, setae_str_data(v), len);
        return n;
    }
    uint32_t existing;
    if (idmap_find(map, v, &existing)) {
        return existing;
    }
    if (t == SETAE_T_LIST) {
        SetaeList *l = setae_to_ptr(v);
        uint32_t len = l->len;
        uint32_t n = msg_add(m);
        idmap_put(map, v, n);
        m->nodes[n].tag = MSG_LIST;
        m->nodes[n].as.seq.len = len;
        m->nodes[n].as.seq.items = len ? malloc(len * sizeof(uint32_t)) : NULL;
        for (uint32_t i = 0; i < len; i++) {
            int64_t ci = msg_read(m, map, vm, l->items[i]);
            if (ci < 0) {
                return -1;
            }
            m->nodes[n].as.seq.items[i] = (uint32_t)ci;
        }
        return n;
    }
    if (t == SETAE_T_TUPLE) {
        SetaeTuple *tp = setae_to_ptr(v);
        uint32_t len = tp->len;
        uint32_t n = msg_add(m);
        idmap_put(map, v, n);
        m->nodes[n].tag = MSG_TUPLE;
        m->nodes[n].as.seq.len = len;
        m->nodes[n].as.seq.items = len ? malloc(len * sizeof(uint32_t)) : NULL;
        for (uint32_t i = 0; i < len; i++) {
            int64_t ci = msg_read(m, map, vm, tp->items[i]);
            if (ci < 0) {
                return -1;
            }
            m->nodes[n].as.seq.items[i] = (uint32_t)ci;
        }
        return n;
    }
    if (t == SETAE_T_DICT) {
        SetaeDict *d = setae_to_ptr(v);
        uint32_t len = d->len;
        uint32_t n = msg_add(m);
        idmap_put(map, v, n);
        m->nodes[n].tag = MSG_DICT;
        m->nodes[n].as.dict.len = len;
        m->nodes[n].as.dict.keys = len ? malloc(len * sizeof(uint32_t)) : NULL;
        m->nodes[n].as.dict.vals = len ? malloc(len * sizeof(uint32_t)) : NULL;
        for (uint32_t i = 0; i < len; i++) {
            int64_t ki = msg_read(m, map, vm, d->entries[i].key);
            if (ki < 0) {
                return -1;
            }
            m->nodes[n].as.dict.keys[i] = (uint32_t)ki;
            int64_t vi = msg_read(m, map, vm, d->entries[i].value);
            if (vi < 0) {
                return -1;
            }
            m->nodes[n].as.dict.vals[i] = (uint32_t)vi;
        }
        return n;
    }
    setae_vm_raise(vm, "TypeError", "cannot send a %s value across actors",
                   setae_type_name(v));
    return -1;
}

static SetaeValue msg_write(const SetaeMsg *m, SetaeValue *built, SetaeVM *vm, uint32_t idx) {
    const SetaeMsgNode *nd = &m->nodes[idx];
    switch (nd->tag) {
    case MSG_NONE:
        return setae_none();
    case MSG_BOOL:
        return setae_bool(nd->as.b);
    case MSG_INT:
        return setae_from_int(nd->as.i);
    case MSG_FLOAT:
        return setae_from_float(nd->as.f);
    case MSG_STR:
        return setae_str_new(vm->heap, nd->as.str.data, nd->as.str.len);
    case MSG_LIST: {
        if (built[idx] != 0) {
            return built[idx];
        }
        SetaeValue lv = setae_list_new(vm->heap, nd->as.seq.len);
        built[idx] = lv;
        SetaeList *l = setae_to_ptr(lv);
        for (uint32_t i = 0; i < nd->as.seq.len; i++) {
            setae_list_push(l, msg_write(m, built, vm, nd->as.seq.items[i]));
        }
        return lv;
    }
    case MSG_TUPLE: {
        if (built[idx] != 0) {
            return built[idx];
        }
        uint32_t len = nd->as.seq.len;
        SetaeValue small[16];
        SetaeValue *tmp = len <= 16 ? small : malloc(len * sizeof(SetaeValue));
        for (uint32_t i = 0; i < len; i++) {
            tmp[i] = msg_write(m, built, vm, nd->as.seq.items[i]);
        }
        SetaeValue tv = setae_tuple_new(vm->heap, tmp, len);
        if (tmp != small) {
            free(tmp);
        }
        built[idx] = tv;
        return tv;
    }
    case MSG_DICT: {
        if (built[idx] != 0) {
            return built[idx];
        }
        SetaeValue dv = setae_dict_new(vm->heap);
        built[idx] = dv;
        SetaeDict *d = setae_to_ptr(dv);
        for (uint32_t i = 0; i < nd->as.dict.len; i++) {
            SetaeValue key = msg_write(m, built, vm, nd->as.dict.keys[i]);
            SetaeValue val = msg_write(m, built, vm, nd->as.dict.vals[i]);
            setae_dict_push(d, key, val);
        }
        return dv;
    }
    }
    return setae_none();
}

SetaeMsg *setae_msg_read(SetaeVM *vm, SetaeValue v) {
    SetaeMsg *m = calloc(1, sizeof(SetaeMsg));
    IdMap map = {0};
    int64_t root = msg_read(m, &map, vm, v);
    free(map.keys);
    free(map.idx);
    if (root < 0) {
        setae_msg_free(m);
        return NULL;
    }
    m->root = (uint32_t)root;
    return m;
}

SetaeValue setae_msg_write(SetaeVM *vm, const SetaeMsg *m) {
    SetaeValue *built = calloc(m->nnodes ? m->nnodes : 1, sizeof(SetaeValue));
    vm->gc_disabled++;
    SetaeValue r = msg_write(m, built, vm, m->root);
    vm->gc_disabled--;
    free(built);
    return r;
}

void setae_list_append(SetaeValue lv, SetaeValue v) {
    setae_list_push(setae_to_ptr(lv), v);
}

void *setae_subject_mailbox(SetaeValue v) {
    return ((SetaeSubject *)setae_to_ptr(v))->mailbox;
}

uint32_t setae_list_len(SetaeValue lv) {
    return ((SetaeList *)setae_to_ptr(lv))->len;
}

SetaeValue setae_list_get(SetaeValue lv, uint32_t i) {
    return ((SetaeList *)setae_to_ptr(lv))->items[i];
}

void setae_dict_put(SetaeValue dv, SetaeValue key, SetaeValue val) {
    setae_dict_push(setae_to_ptr(dv), key, val);
}

void setae_msg_free(SetaeMsg *m) {
    if (m == NULL) {
        return;
    }
    for (uint32_t i = 0; i < m->nnodes; i++) {
        SetaeMsgNode *nd = &m->nodes[i];
        if (nd->tag == MSG_STR) {
            free(nd->as.str.data);
        } else if (nd->tag == MSG_LIST || nd->tag == MSG_TUPLE) {
            free(nd->as.seq.items);
        } else if (nd->tag == MSG_DICT) {
            free(nd->as.dict.keys);
            free(nd->as.dict.vals);
        }
    }
    free(m->nodes);
    free(m);
}
