#include <stdio.h>
#include <stdlib.h>
#include "vec.h"
#include "str.h"

int main(void) {
    /* --- libvec --- */
    Vec3 a = {1.0, 2.0, 3.0};
    Vec3 b = {4.0, 5.0, 6.0};

    Vec3 sum = vec3_add(a, b);
    printf("vec3_add:   (%.1f, %.1f, %.1f)\n", sum.x, sum.y, sum.z);

    double dot = vec3_dot(a, b);
    printf("vec3_dot:   %.1f\n", dot);

    double len = vec3_len(a);
    printf("vec3_len:   %.4f\n", len);

    /* --- libstr --- */
    int count = str_count_char("mississippi", 's');
    printf("count 's':  %d\n", count);

    char *rep = str_repeat("ab", 3);
    printf("repeat:     %s\n", rep);
    free(rep);

    char buf[] = "hello";
    printf("reverse:    %s\n", str_reverse(buf));

    puts("PASS");
    return 0;
}
