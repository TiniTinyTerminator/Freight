const std = @import("std");
const print = std.debug.print;

// ── C++ Vec3 bindings (pointer interface from vec3.cpp) ───────────────────────
// Pointer params avoid SysV x86-64 MEMORY-class struct ABI ambiguity for
// 24-byte structs across Zig ↔ C++ boundaries.

const Vec3 = extern struct { x: f64, y: f64, z: f64 };

extern fn vec3_add(a: *const Vec3, b: *const Vec3, out: *Vec3) void;
extern fn vec3_sub(a: *const Vec3, b: *const Vec3, out: *Vec3) void;
extern fn vec3_scale(v: *const Vec3, s: f64, out: *Vec3) void;
extern fn vec3_dot(a: *const Vec3, b: *const Vec3) f64;
extern fn vec3_cross(a: *const Vec3, b: *const Vec3, out: *Vec3) void;
extern fn vec3_length(v: *const Vec3) f64;
extern fn vec3_normalize(v: *const Vec3, out: *Vec3) void;

// ── Thin Zig wrapper with value-type methods ──────────────────────────────────

const V = struct {
    v: Vec3,

    fn init(x: f64, y: f64, z: f64) V { return .{ .v = .{ .x = x, .y = y, .z = z } }; }

    fn add(a: V, b: V) V {
        var out: Vec3 = undefined;
        vec3_add(&a.v, &b.v, &out);
        return .{ .v = out };
    }
    fn sub(a: V, b: V) V {
        var out: Vec3 = undefined;
        vec3_sub(&a.v, &b.v, &out);
        return .{ .v = out };
    }
    fn scale(a: V, s: f64) V {
        var out: Vec3 = undefined;
        vec3_scale(&a.v, s, &out);
        return .{ .v = out };
    }
    fn cross(a: V, b: V) V {
        var out: Vec3 = undefined;
        vec3_cross(&a.v, &b.v, &out);
        return .{ .v = out };
    }
    fn norm(a: V) V {
        var out: Vec3 = undefined;
        vec3_normalize(&a.v, &out);
        return .{ .v = out };
    }
    fn dot(a: V, b: V) f64 { return vec3_dot(&a.v, &b.v); }
    fn len(a: V)        f64 { return vec3_length(&a.v); }
};

pub fn main() void {
    const i = V.init(1, 0, 0);
    const j = V.init(0, 1, 0);
    const k = V.init(0, 0, 1);

    print("─── Arithmetic (C++ impl, Zig wrapper) ───\n", .{});
    const sum = i.add(j).add(k);
    print("i + j + k   = ({d}, {d}, {d})\n", .{ sum.v.x, sum.v.y, sum.v.z });
    print("|i + j + k| = {d:.6}\n",           .{sum.len()});

    const scaled = sum.scale(3.0);
    print("3*(i+j+k)   = ({d}, {d}, {d})\n", .{ scaled.v.x, scaled.v.y, scaled.v.z });

    print("\n─── Cross products ───\n", .{});
    const ij = i.cross(j); print("i × j = ({d}, {d}, {d})  (expect k)\n", .{ ij.v.x, ij.v.y, ij.v.z });
    const jk = j.cross(k); print("j × k = ({d}, {d}, {d})  (expect i)\n", .{ jk.v.x, jk.v.y, jk.v.z });
    const ki = k.cross(i); print("k × i = ({d}, {d}, {d})  (expect j)\n", .{ ki.v.x, ki.v.y, ki.v.z });

    print("\n─── Gram–Schmidt orthonormalisation ───\n", .{});
    const a = V.init(1, 1, 0);
    const b = V.init(0, 1, 1);
    const e1 = a.norm();
    const e2 = b.sub(e1.scale(e1.dot(b))).norm();
    const e3 = e1.cross(e2);
    print("e1 = ({d:.4}, {d:.4}, {d:.4})  |e1|={d:.4}\n", .{ e1.v.x, e1.v.y, e1.v.z, e1.len() });
    print("e2 = ({d:.4}, {d:.4}, {d:.4})  |e2|={d:.4}\n", .{ e2.v.x, e2.v.y, e2.v.z, e2.len() });
    print("e3 = ({d:.4}, {d:.4}, {d:.4})  |e3|={d:.4}\n", .{ e3.v.x, e3.v.y, e3.v.z, e3.len() });
    print("e1·e2={d:.6}  e2·e3={d:.6}  e1·e3={d:.6}  (all ≈0)\n",
        .{ e1.dot(e2), e2.dot(e3), e1.dot(e3) });
}
