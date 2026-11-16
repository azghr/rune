// SPDX-License-Identifier: MIT

#include "rune_rt.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void *rune_closure_new(void *code, int n_caps) {
  void **block = malloc(8 * (size_t)(n_caps + 1));
  if (!block) {
    return NULL;
  }
  block[0] = code;
  return block;
}

void rune_closure_set(void *closure, int idx, int64_t value) {
  ((int64_t *)closure)[idx + 1] = value;
}

/* Arrays: [length, elems...], one 8-byte slot per element. */
static void rune_arr_check(void *arr, int64_t idx) {
  int64_t len = arr ? ((int64_t *)arr)[0] : 0;
  if (!arr || idx < 0 || idx >= len) {
    fprintf(stderr, "rune: array index %lld out of bounds (length %lld)\n",
            (long long)idx, (long long)len);
    abort();
  }
}

void *rune_arr_new(int64_t n) {
  void **block = calloc((size_t)n + 1, 8);
  if (!block) { return NULL; }
  block[0] = (void *)(intptr_t)n;
  return block;
}

int64_t rune_arr_len(void *arr) { return arr ? ((int64_t *)arr)[0] : 0; }

int64_t rune_arr_get(void *arr, int64_t idx) {
  rune_arr_check(arr, idx);
  return ((int64_t *)arr)[idx + 1];
}

void rune_arr_set(void *arr, int64_t idx, int64_t value) {
  rune_arr_check(arr, idx);
  ((int64_t *)arr)[idx + 1] = value;
}

/* Concatenates two strings into freshly allocated storage. The result is
 * never freed: Rune strings are immutable `const char*` values, so ownership
 * is not tracked. Treat the compiler as garbage-producing for now. */
const char *rune_strcat(const char *a, const char *b) {
  size_t la = a ? strlen(a) : 0;
  size_t lb = b ? strlen(b) : 0;
  char *out = malloc(la + lb + 1);
  if (!out) {
    return "(out of memory)";
  }
  if (a) {
    memcpy(out, a, la);
  }
  if (b) {
    memcpy(out + la, b, lb);
  }
  out[la + lb] = '\0';
  return out;
}

void rune_print_int(int64_t value) { printf("%lld\n", (long long)value); }

void rune_print_bool(bool value) { printf("%s\n", value ? "true" : "false"); }

void rune_print_string(const char *value) {
  printf("%s\n", value ? value : "(null)");
}