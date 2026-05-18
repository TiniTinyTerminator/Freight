#include "vec.h"
#include <math.h>

Vec3 vec3_add(Vec3 a, Vec3 b) {
    return (Vec3){ a.x + b.x, a.y + b.y, a.z + b.z };
}

Vec3 vec3_sub(Vec3 a, Vec3 b) {
    return (Vec3){ a.x - b.x, a.y - b.y, a.z - b.z };
}

Vec3 vec3_scale(Vec3 v, double s) {
    return (Vec3){ v.x * s, v.y * s, v.z * s };
}

double vec3_dot(Vec3 a, Vec3 b) {
    return a.x * b.x + a.y * b.y + a.z * b.z;
}

double vec3_len(Vec3 v) {
    return sqrt(vec3_dot(v, v));
}
