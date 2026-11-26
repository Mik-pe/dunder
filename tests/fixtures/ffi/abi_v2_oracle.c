#include <stdint.h>
#include <stdarg.h>
#include <stddef.h>
#include <pthread.h>
#include <math.h>

typedef struct { uint8_t a, b, c; } S3;
typedef struct { int32_t x, y; } I32Pair;
typedef struct { int32_t a, b, c; } I32Triple;
typedef struct { int64_t a, b; } I64Pair;
typedef struct { float a, b; } F32Pair;
typedef struct { float a, b, c; } F32Triple;
typedef struct { float a, b, c, d; } F32Quad;
typedef struct { double r, g, b, a; } ClearColor;
typedef struct { int64_t tag; double value; } IntFloat;
typedef struct { double value; int64_t tag; } FloatInt;
typedef struct { float f; int32_t i; double d; } Mixed16;
typedef struct { uint16_t k[2]; float f; double d; } NestedMixed16;
typedef struct { uint64_t x, y, z; } Origin;
typedef struct { uint64_t w, h, d; } Extent;
typedef struct { Origin origin; Extent size; } Region;

I32Pair roundtrip_i32_pair(I32Pair p) { return (I32Pair){ p.x + 10, p.y - 20 }; }
I32Triple roundtrip_i32_triple(I32Triple t) { return (I32Triple){ t.c, t.a + 1, t.b + 2 }; }
I64Pair roundtrip_i64_pair(I64Pair p) { return (I64Pair){ p.b + 100, p.a - 50 }; }
F32Pair roundtrip_f32_pair(F32Pair p) { return (F32Pair){ p.b * 2.0f, p.a + 1.5f }; }
F32Quad roundtrip_f32_quad(F32Quad q) { return (F32Quad){ q.d, q.c, q.b, q.a }; }
ClearColor roundtrip_clear_color(ClearColor c) { return (ClearColor){ c.r + 0.5, c.g + 1.0, c.b + 1.5, c.a + 2.0 }; }
IntFloat roundtrip_int_float(IntFloat v) { return (IntFloat){ v.tag * 3, v.value + 7.25 }; }
FloatInt roundtrip_float_int(FloatInt v) { return (FloatInt){ v.value * 2.0, v.tag - 11 }; }
Mixed16 roundtrip_mixed16(Mixed16 m) { return (Mixed16){ m.f + 1.0f, m.i * 5, m.d - 0.5 }; }
NestedMixed16 roundtrip_nested_mixed16(NestedMixed16 n) {
    return (NestedMixed16){ { (uint16_t)(n.k[0] + 1), (uint16_t)(n.k[1] + 2) }, n.f * 2.0f, n.d + 3.0 };
}

Origin roundtrip_origin(Origin o) {
    Origin out = { o.x + 1, o.y + 2, o.z + 3 };
    o.x = 999999;
    return out;
}

Region roundtrip_region(Region r, uint64_t dx, uint64_t dy) {
    Region out = {
        { r.origin.x + dx, r.origin.y + dy, r.origin.z + 1 },
        { r.size.w * 2, r.size.h * 3, r.size.d + 4 }
    };
    r.origin.x = 777777;
    return out;
}

/* SysV atomic spill: 5 GPRs + 2-GPR I64Pair (must spill atomically!) + final i64 (gets r9!) */
int64_t spill_atomic_sysv(int64_t a0, int64_t a1, int64_t a2, int64_t a3, int64_t a4, I64Pair spilled, int64_t tail) {
    return a0 + a1 + a2 + a3 + a4 + spilled.a * 10 + spilled.b * 100 + tail * 1000;
}

/* AAPCS64 Darwin & Linux spilled HFA + spilled non-HFA + packed scalars */
double spill_hfa_and_structs(
    double d0, double d1, double d2, double d3, double d4, double d5, double d6,
    int64_t g0, int64_t g1, int64_t g2, int64_t g3, int64_t g4, int64_t g5, int64_t g6,
    I64Pair sp_pair, S3 sp_s3, uint8_t sp_u8, I32Triple sp_i12,
    F32Triple sp_hfa1, F32Triple sp_hfa2, float sp_f) {
    double s = d0 + d1 + d2 + d3 + d4 + d5 + d6;
    s += (double)(g0 + g1 + g2 + g3 + g4 + g5 + g6);
    s += (double)(sp_pair.a + sp_pair.b + sp_s3.a + sp_s3.b + sp_s3.c + sp_u8);
    s += (double)(sp_i12.a + sp_i12.b + sp_i12.c);
    s += (double)(sp_hfa1.a + sp_hfa1.b + sp_hfa1.c + sp_hfa2.a + sp_hfa2.b + sp_hfa2.c + sp_f);
    return s;
}

/* Clean upper bytes on sub-word stack arguments (after 8+ GPRs) */
int64_t subword_stack_args(
    int64_t r0, int64_t r1, int64_t r2, int64_t r3,
    int64_t r4, int64_t r5, int64_t r6, int64_t r7,
    uint8_t u8v, int16_t i16v, uint16_t u16v, int32_t i32v, uint32_t u32v,
    uint8_t u8v2, int16_t i16v2, int32_t i32v2) {
    if (r0 != 1 || r7 != 8) return -1;
    if (u8v != 254 || i16v != -32768 || u16v != 65000 || i32v != -123456 || u32v != 4000000000u) return -2;
    if (u8v2 != 5 || i16v2 != -7 || i32v2 != -9) return -3;
    return r0 + r1 + r2 + r3 + r4 + r5 + r6 + r7 +
           (int64_t)u8v + (int64_t)i16v + (int64_t)u16v + (int64_t)i32v + (int64_t)u32v +
           (int64_t)u8v2 + (int64_t)i16v2 + (int64_t)i32v2;
}

int64_t variadic_clean_upper(int32_t count, ...) {
    va_list ap;
    va_start(ap, count);
    int64_t sum = 0;
    for (int32_t i = 0; i < count; i++) {
        if (i % 2 == 0) {
            int32_t v = va_arg(ap, int32_t);
            sum += (int64_t)v;
        } else {
            uint32_t u = va_arg(ap, uint32_t);
            sum += (int64_t)u;
        }
    }
    va_end(ap);
    return sum;
}

/* Large outgoing stack frame > 4080 bytes: 100 Region (48B each = 4800B on stack!) */
uint64_t large_frame_sum_96_regions(
    Region r0,  Region r1,  Region r2,  Region r3,  Region r4,  Region r5,  Region r6,  Region r7,
    Region r8,  Region r9,  Region r10, Region r11, Region r12, Region r13, Region r14, Region r15,
    Region r16, Region r17, Region r18, Region r19, Region r20, Region r21, Region r22, Region r23,
    Region r24, Region r25, Region r26, Region r27, Region r28, Region r29, Region r30, Region r31,
    Region r32, Region r33, Region r34, Region r35, Region r36, Region r37, Region r38, Region r39,
    Region r40, Region r41, Region r42, Region r43, Region r44, Region r45, Region r46, Region r47,
    Region r48, Region r49, Region r50, Region r51, Region r52, Region r53, Region r54, Region r55,
    Region r56, Region r57, Region r58, Region r59, Region r60, Region r61, Region r62, Region r63,
    Region r64, Region r65, Region r66, Region r67, Region r68, Region r69, Region r70, Region r71,
    Region r72, Region r73, Region r74, Region r75, Region r76, Region r77, Region r78, Region r79,
    Region r80, Region r81, Region r82, Region r83, Region r84, Region r85, Region r86, Region r87,
    Region r88, Region r89, Region r90, Region r91, Region r92, Region r93, Region r94, Region r95) {
    Region all[96] = {
        r0,  r1,  r2,  r3,  r4,  r5,  r6,  r7,  r8,  r9,  r10, r11, r12, r13, r14, r15,
        r16, r17, r18, r19, r20, r21, r22, r23, r24, r25, r26, r27, r28, r29, r30, r31,
        r32, r33, r34, r35, r36, r37, r38, r39, r40, r41, r42, r43, r44, r45, r46, r47,
        r48, r49, r50, r51, r52, r53, r54, r55, r56, r57, r58, r59, r60, r61, r62, r63,
        r64, r65, r66, r67, r68, r69, r70, r71, r72, r73, r74, r75, r76, r77, r78, r79,
        r80, r81, r82, r83, r84, r85, r86, r87, r88, r89, r90, r91, r92, r93, r94, r95
    };
    uint64_t sum = 0;
    for (int i = 0; i < 96; i++) {
        sum += all[i].origin.x + all[i].origin.y + all[i].origin.z +
               all[i].size.w + all[i].size.h + all[i].size.d;
    }
    return sum;
}

/* Struct-by-value callbacks */
typedef I32Pair (*CbI32Pair)(I32Pair);
typedef I64Pair (*CbI64Pair)(I64Pair, int64_t);
typedef F32Quad (*CbF32Quad)(F32Quad);
typedef IntFloat (*CbIntFloat)(IntFloat, FloatInt);
typedef Mixed16 (*CbMixed16)(Mixed16);
typedef ClearColor (*CbClearColor)(ClearColor, double);
typedef Region (*CbRegion)(Region, Origin);
typedef IntFloat (*CbManyArgs)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, IntFloat);

I32Pair call_cb_i32_pair(CbI32Pair cb, I32Pair p) { return cb(p); }
I64Pair call_cb_i64_pair(CbI64Pair cb, I64Pair p, int64_t k) { return cb(p, k); }
F32Quad call_cb_f32_quad(CbF32Quad cb, F32Quad q) { return cb(q); }
IntFloat call_cb_int_float(CbIntFloat cb, IntFloat a, FloatInt b) { return cb(a, b); }
Mixed16 call_cb_mixed16(CbMixed16 cb, Mixed16 m) { return cb(m); }
ClearColor call_cb_clear_color(CbClearColor cb, ClearColor c, double s) { return cb(c, s); }
Region call_cb_region(CbRegion cb, Region r, Origin o) { return cb(r, o); }
IntFloat call_cb_many_args(CbManyArgs cb, IntFloat x) {
    return cb(1, 2, 3, 4, 5, 6, 7, 8, 9, x);
}

/* Callee-saved register preservation check across Dunder callback */
typedef int64_t (*CbWork)(int64_t);
int64_t verify_callee_saved_across_cb(CbWork cb, int64_t seed) {
    double f0 = (double)seed + 1.125;
    double f1 = (double)seed + 2.25;
    double f2 = (double)seed + 3.375;
    double f3 = (double)seed + 4.5;
    double f4 = (double)seed + 5.625;
    double f5 = (double)seed + 6.75;
    double f6 = (double)seed + 7.875;
    double f7 = (double)seed + 9.0;
    int64_t g0 = seed * 3 + 1;
    int64_t g1 = seed * 5 + 2;
    int64_t g2 = seed * 7 + 3;
    int64_t g3 = seed * 11 + 4;
    int64_t res = cb(seed);
    double fsum = f0 + f1 + f2 + f3 + f4 + f5 + f6 + f7;
    double expected_fsum = 8.0 * (double)seed + (1.125 + 2.25 + 3.375 + 4.5 + 5.625 + 6.75 + 7.875 + 9.0);
    if (fabs(fsum - expected_fsum) > 1e-9) return -100;
    if (g0 != seed * 3 + 1 || g1 != seed * 5 + 2 || g2 != seed * 7 + 3 || g3 != seed * 11 + 4) return -200;
    return res + g0 + g1 + g2 + g3;
}

/* Reentrant callback trampoline */
typedef int64_t (*CbReentrant)(int64_t);
int64_t bounce_reentrant(CbReentrant cb, int64_t depth) {
    return cb(depth);
}

/* Foreign pthread callback + concurrent multi-threaded callbacks */
typedef IntFloat (*CbForeignThread)(int64_t, int64_t);
typedef struct {
    CbForeignThread cb;
    int64_t thread_id;
    int64_t iters;
    int64_t out_sum;
    double out_fsum;
} WorkerArgs;

static void *foreign_worker_entry(void *raw) {
    WorkerArgs *w = (WorkerArgs *)raw;
    int64_t sum = 0;
    double fsum = 0.0;
    for (int64_t i = 0; i < w->iters; i++) {
        IntFloat r = w->cb(w->thread_id, i);
        sum += r.tag;
        fsum += r.value;
    }
    w->out_sum = sum;
    w->out_fsum = fsum;
    return 0;
}

IntFloat run_single_foreign_thread(CbForeignThread cb, int64_t tid, int64_t iters) {
    WorkerArgs args = { cb, tid, iters, 0, 0.0 };
    pthread_t th;
    if (pthread_create(&th, 0, foreign_worker_entry, &args) != 0) {
        return (IntFloat){ -1, -1.0 };
    }
    pthread_join(th, 0);
    return (IntFloat){ args.out_sum, args.out_fsum };
}

IntFloat run_concurrent_foreign_threads(CbForeignThread cb, int32_t nthreads, int64_t iters) {
    if (nthreads > 16) nthreads = 16;
    pthread_t threads[16];
    WorkerArgs args[16];
    for (int32_t t = 0; t < nthreads; t++) {
        args[t].cb = cb;
        args[t].thread_id = t + 1;
        args[t].iters = iters;
        args[t].out_sum = 0;
        args[t].out_fsum = 0.0;
        if (pthread_create(&threads[t], 0, foreign_worker_entry, &args[t]) != 0) {
            return (IntFloat){ -1, -1.0 };
        }
    }
    int64_t total_sum = 0;
    double total_fsum = 0.0;
    for (int32_t t = 0; t < nthreads; t++) {
        pthread_join(threads[t], 0);
        total_sum += args[t].out_sum;
        total_fsum += args[t].out_fsum;
    }
    return (IntFloat){ total_sum, total_fsum };
}
