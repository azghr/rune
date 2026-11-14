#include "rune_rt.h"
#include <stdio.h>

void rune_print_int(int64_t value) { printf("%lld\n", (long long)value); }

void rune_print_bool(bool value) { printf("%s\n", value ? "true" : "false"); }

void rune_print_string(const char *value) {
  printf("%s\n", value ? value : "(null)");
}