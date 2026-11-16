// SPDX-License-Identifier: MIT

#ifndef RUNE_RT_H
#define RUNE_RT_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

const char *rune_strcat(const char *a, const char *b);

/* Arithmetic: division by zero aborts the program; `INT64_MIN / -1`
 * wraps instead of overflowing (matching Rune's constant folder). */
int64_t rune_div(int64_t a, int64_t b);
int64_t rune_mod(int64_t a, int64_t b);

/* Closures: a heap block whose first word is the wrapper's code pointer,
 * followed by the captured values. Function values are closure pointers,
 * so calls read the code word and pass the closure itself as the env
 * argument. */
void *rune_closure_new(void *code, int n_caps);
void rune_closure_set(void *closure, int idx, int64_t value);

/* Arrays: a heap block whose first word is the length, followed by one
 * 8-byte slot per element (narrow values widened, pointers stored whole).
 * Accesses are bounds-checked and abort on violation. */
void *rune_arr_new(int64_t n);
int64_t rune_arr_len(void *arr);
int64_t rune_arr_get(void *arr, int64_t idx);
void rune_arr_set(void *arr, int64_t idx, int64_t value);

void rune_print_int(int64_t value);
void rune_print_bool(bool value);
void rune_print_string(const char *value);

#ifdef __cplusplus
}
#endif

#endif /* RUNE_RT_H */