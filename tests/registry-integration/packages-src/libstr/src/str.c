#include "str.h"
#include <stdlib.h>
#include <string.h>

int str_count_char(const char *s, char c) {
    int n = 0;
    while (*s) { if (*s++ == c) n++; }
    return n;
}

char *str_repeat(const char *s, int n) {
    size_t len = strlen(s);
    char *out = malloc(len * (size_t)n + 1);
    if (!out) return NULL;
    char *p = out;
    for (int i = 0; i < n; i++) { memcpy(p, s, len); p += len; }
    *p = '\0';
    return out;
}

char *str_reverse(char *s) {
    size_t len = strlen(s);
    for (size_t i = 0, j = len - 1; i < j; i++, j--) {
        char t = s[i]; s[i] = s[j]; s[j] = t;
    }
    return s;
}
