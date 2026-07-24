#include "internal.h"

#include <math.h>
#include <stdlib.h>
#include <string.h>

#define SET_LINEAR_PROBES 9
#define SET_PERTURB_SHIFT 5

static int64_t set_key_hash(SetaeValue v) {
    if (setae_is_bool(v)) {
        return setae_to_bool(v) ? 1 : 0;
    }
    if (setae_is_int(v)) {
        int64_t n = setae_to_int(v);
        return n == -1 ? -2 : n;
    }
    if (setae_is_float(v)) {
        double f = setae_to_float(v);
        if (f == floor(f) && f >= -9223372036854775808.0 && f < 9223372036854775808.0) {
            int64_t n = (int64_t)f;
            return n == -1 ? -2 : n;
        }
    }
    return (int64_t)setae_value_hash(v);
}

static void set_insert_clean(SetaeSetEntry *table, uint32_t mask, SetaeValue key,
                             int64_t hash) {
    uint64_t perturb = (uint64_t)hash;
    uint32_t i = (uint32_t)((uint64_t)hash & mask);
    while (1) {
        uint32_t limit = (i + SET_LINEAR_PROBES <= mask) ? SET_LINEAR_PROBES : 0;
        for (uint32_t k = 0;; k++) {
            SetaeSetEntry *e = &table[i + k];
            if (e->state == SET_EMPTY) {
                e->state = SET_ACTIVE;
                e->key = key;
                e->hash = hash;
                return;
            }
            if (k == limit) {
                break;
            }
        }
        perturb >>= SET_PERTURB_SHIFT;
        i = (uint32_t)(((uint64_t)i * 5 + 1 + perturb) & mask);
    }
}

static void set_resize(SetaeSet *s, uint32_t minused) {
    uint32_t newsize = 8;
    while (newsize <= minused) {
        newsize <<= 1;
    }
    SetaeSetEntry *old = s->table;
    uint32_t oldmask = s->mask;
    SetaeSetEntry *nt = calloc(newsize, sizeof(SetaeSetEntry));
    if (nt == NULL) {
        return;
    }
    s->table = nt;
    s->mask = newsize - 1;
    for (uint32_t i = 0; i <= oldmask; i++) {
        if (old[i].state == SET_ACTIVE) {
            set_insert_clean(nt, s->mask, old[i].key, old[i].hash);
        }
    }
    s->fill = s->used;
    free(old);
}

void setae_set_presize(SetaeSet *s, uint32_t incoming) {
    if (((uint64_t)s->fill + incoming) * 5 >= (uint64_t)s->mask * 3) {
        set_resize(s, (s->used + incoming) * 2);
    }
}

void setae_set_merge(SetaeSet *so, SetaeSet *other) {
    if (other == so || other->used == 0) {
        return;
    }
    if (((uint64_t)so->fill + other->used) * 5 >= (uint64_t)so->mask * 3) {
        set_resize(so, (so->used + other->used) * 2);
    }
    if (so->fill == 0 && so->mask == other->mask && other->fill == other->used) {
        memcpy(so->table, other->table, ((size_t)other->mask + 1) * sizeof(SetaeSetEntry));
        so->fill = other->fill;
        so->used = other->used;
        return;
    }
    if (so->fill == 0) {
        so->fill = other->used;
        so->used = other->used;
        for (uint32_t i = 0; i <= other->mask; i++) {
            if (other->table[i].state == SET_ACTIVE) {
                set_insert_clean(so->table, so->mask, other->table[i].key,
                                 other->table[i].hash);
            }
        }
        return;
    }
    for (uint32_t i = 0; i <= other->mask; i++) {
        if (other->table[i].state == SET_ACTIVE) {
            setae_set_add(so, other->table[i].key);
        }
    }
}

int setae_set_add(SetaeSet *s, SetaeValue key) {
    int64_t hash = set_key_hash(key);
    uint32_t mask = s->mask;
    uint64_t perturb = (uint64_t)hash;
    uint32_t i = (uint32_t)((uint64_t)hash & mask);
    int64_t freeslot = -1;
    while (1) {
        uint32_t limit = (i + SET_LINEAR_PROBES <= mask) ? SET_LINEAR_PROBES : 0;
        for (uint32_t k = 0;; k++) {
            uint32_t idx = i + k;
            SetaeSetEntry *e = &s->table[idx];
            if (e->state == SET_EMPTY) {
                if (freeslot >= 0) {
                    SetaeSetEntry *f = &s->table[freeslot];
                    f->state = SET_ACTIVE;
                    f->key = key;
                    f->hash = hash;
                    s->used++;
                    return 1;
                }
                e->state = SET_ACTIVE;
                e->key = key;
                e->hash = hash;
                s->fill++;
                s->used++;
                if ((uint64_t)s->fill * 5 >= (uint64_t)mask * 3) {
                    set_resize(s, s->used > 50000 ? s->used * 2 : s->used * 4);
                }
                return 1;
            }
            if (e->state == SET_ACTIVE && e->hash == hash && setae_value_eq(e->key, key)) {
                return 0;
            }
            if (e->state == SET_DUMMY && freeslot < 0) {
                freeslot = idx;
            }
            if (k == limit) {
                break;
            }
        }
        perturb >>= SET_PERTURB_SHIFT;
        i = (uint32_t)(((uint64_t)i * 5 + 1 + perturb) & mask);
    }
}

int setae_set_contains(const SetaeSet *s, SetaeValue key) {
    int64_t hash = set_key_hash(key);
    uint32_t mask = s->mask;
    uint64_t perturb = (uint64_t)hash;
    uint32_t i = (uint32_t)((uint64_t)hash & mask);
    while (1) {
        uint32_t limit = (i + SET_LINEAR_PROBES <= mask) ? SET_LINEAR_PROBES : 0;
        for (uint32_t k = 0;; k++) {
            const SetaeSetEntry *e = &s->table[i + k];
            if (e->state == SET_EMPTY) {
                return 0;
            }
            if (e->state == SET_ACTIVE && e->hash == hash && setae_value_eq(e->key, key)) {
                return 1;
            }
            if (k == limit) {
                break;
            }
        }
        perturb >>= SET_PERTURB_SHIFT;
        i = (uint32_t)(((uint64_t)i * 5 + 1 + perturb) & mask);
    }
}

int setae_set_discard(SetaeSet *s, SetaeValue key) {
    int64_t hash = set_key_hash(key);
    uint32_t mask = s->mask;
    uint64_t perturb = (uint64_t)hash;
    uint32_t i = (uint32_t)((uint64_t)hash & mask);
    while (1) {
        uint32_t limit = (i + SET_LINEAR_PROBES <= mask) ? SET_LINEAR_PROBES : 0;
        for (uint32_t k = 0;; k++) {
            SetaeSetEntry *e = &s->table[i + k];
            if (e->state == SET_EMPTY) {
                return 0;
            }
            if (e->state == SET_ACTIVE && e->hash == hash && setae_value_eq(e->key, key)) {
                e->state = SET_DUMMY;
                e->key = 0;
                s->used--;
                return 1;
            }
            if (k == limit) {
                break;
            }
        }
        perturb >>= SET_PERTURB_SHIFT;
        i = (uint32_t)(((uint64_t)i * 5 + 1 + perturb) & mask);
    }
}
