#include <cmath>

// Use pointer params for structs > 16 bytes to guarantee C ABI compatibility
// across Zig ↔ C++ boundaries (SysV x86-64 MEMORY-class struct passing rules
// are implemented differently by the two compilers for by-value structs).

extern "C" {

struct Vec3 { double x, y, z; };

void vec3_add(const Vec3* a, const Vec3* b, Vec3* out) {
    *out = { a->x + b->x, a->y + b->y, a->z + b->z };
}

void vec3_sub(const Vec3* a, const Vec3* b, Vec3* out) {
    *out = { a->x - b->x, a->y - b->y, a->z - b->z };
}

void vec3_scale(const Vec3* v, double s, Vec3* out) {
    *out = { v->x * s, v->y * s, v->z * s };
}

double vec3_dot(const Vec3* a, const Vec3* b) {
    return a->x*b->x + a->y*b->y + a->z*b->z;
}

void vec3_cross(const Vec3* a, const Vec3* b, Vec3* out) {
    *out = {
        a->y*b->z - a->z*b->y,
        a->z*b->x - a->x*b->z,
        a->x*b->y - a->y*b->x,
    };
}

double vec3_length(const Vec3* v) {
    return std::sqrt(v->x*v->x + v->y*v->y + v->z*v->z);
}

void vec3_normalize(const Vec3* v, Vec3* out) {
    double len = vec3_length(v);
    *out = { v->x / len, v->y / len, v->z / len };
}

} // extern "C"
