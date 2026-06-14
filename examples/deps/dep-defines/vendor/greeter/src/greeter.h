#pragma once

// Print a greeting. The output depends on whether this library was compiled
// with -DGREETER_FANCY (the consumer can flip that via a feature without
// touching this source).
void greet();
