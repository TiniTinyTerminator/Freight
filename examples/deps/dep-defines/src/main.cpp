#include "greeter.h"
#include <iostream>

int main() {
    // greet() prints differently depending on whether the greeter library was
    // compiled with -DGREETER_FANCY (forwarded by the `fancy` feature).
    greet();

    // The define is scoped to the greeter package's build — it never leaks into
    // this binary's own compilation, even when `--features fancy` is active.
#ifdef GREETER_FANCY
    std::cout << "(app sees GREETER_FANCY — this should never happen!)\n";
#else
    std::cout << "(app does NOT see GREETER_FANCY — defines are per-package)\n";
#endif
    return 0;
}
