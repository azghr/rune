// SPDX-License-Identifier: MIT

#ifndef RUNE_RT_H
#define RUNE_RT_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

void rune_print_int(int64_t value);
void rune_print_bool(bool value);
void rune_print_string(const char *value);

#ifdef __cplusplus
}
#endif

#endif /* RUNE_RT_H */