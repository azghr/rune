// SPDX-License-Identifier: MIT

#ifndef RUNE_RT_H
#define RUNE_RT_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

const char *rune_strcat(const char *a, const char *b);

/* Closures: a heap block whose first word is the wrapper's code pointer,
 * followed by the captured values. Function values are closure pointers,
 * so calls read the code word and pass the closure itself as the env
 * argument. */
void *rune_closure_new(void *code, int n_caps);
void rune_closure_set(void *closure, int idx, int64_t value);

void rune_print_int(int64_t value);
void rune_print_bool(bool value);
void rune_print_string(const char *value);

#ifdef __cplusplus
}
#endif

#endif /* RUNE_RT_H */