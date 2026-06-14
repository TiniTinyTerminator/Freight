#include <stdio.h> /* stdlib — allowed */
#include <zlib.h>  /* undeclared dependency (zlib is not in freight.toml) — rejected */

int main(void) {
    printf("unreachable: the build is blocked before compiling\n");
    return 0;
}
