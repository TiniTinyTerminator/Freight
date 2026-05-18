#pragma once
#include <stddef.h>

/* Count occurrences of `c` in null-terminated string `s`. */
int str_count_char(const char *s, char c);

/* Return a newly allocated string that repeats `s` exactly `n` times.
   Caller owns the returned memory (free() it). */
char *str_repeat(const char *s, int n);

/* Reverse `s` in-place; returns `s`. */
char *str_reverse(char *s);
