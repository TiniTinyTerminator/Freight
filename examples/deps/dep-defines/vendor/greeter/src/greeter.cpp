#include "greeter.h"
#include <iostream>

void greet() {
#ifdef GREETER_FANCY
    std::cout << "*** Hello from the FANCY greeter! ***\n";
#else
    std::cout << "Hello from the greeter.\n";
#endif
}
