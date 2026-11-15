// SPDX-License-Identifier: MIT

#include "rune_rt.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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