#ifndef GECKO_INTERNAL_H
#define GECKO_INTERNAL_H

#include "gecko.h"

const GkValue *gk_code_consts(const GkCode *c);
const char *gk_code_name(const GkCode *c, uint32_t i);
const uint8_t *gk_code_bytes(const GkCode *c, uint32_t *n);
uint32_t gk_code_nlocals(const GkCode *c);

void gk_vm_append_output(GkVM *vm, const char *bytes, size_t len);
GkHeap *gk_vm_heap(GkVM *vm);

#endif /* GECKO_INTERNAL_H */
