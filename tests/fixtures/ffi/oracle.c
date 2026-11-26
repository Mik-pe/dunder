#include <stdint.h>
#include <stdlib.h>
#include <pthread.h>
#include <stddef.h>
#include <string.h>
typedef void (*NoArgs)(void);
static void *from_worker(void *arg) { (*(NoArgs *)arg)(); return 0; }
void on_worker(NoArgs f) { pthread_t t; if (!pthread_create(&t, 0, from_worker, &f)) pthread_join(t, 0); }
typedef struct { float a, b, c; } Triple;
typedef struct { double a, b; } Pair;
typedef struct { Pair origin, size; } Rect;
typedef struct { uint8_t tag; Triple samples[2]; uint64_t tail; } Samples;
int inspect_samples(Samples *s, Triple *values, size_t size, size_t alignment) {
    if (size != sizeof *s || alignment != _Alignof(Samples) ||
        s->tag != 7 || s->tail != 99 || s->samples[0].a != 1 || s->samples[1].c != 6 ||
        values[0].b != 2 || values[1].a != 4) return 0;
    s->samples[1].b = 42;
    values[1].c = 77;
    return 1;
}
Triple triple(Triple v) { return (Triple){v.b, v.c, v.a}; }
Rect rect(Rect a, double step, Rect b) {
    return (Rect){{a.origin.b + step, b.origin.a}, {a.size.b, b.size.a}};
}
typedef int32_t (*Callback)(int32_t, double, int32_t, int32_t, int32_t, int32_t, int32_t, int32_t, uint16_t, int32_t);
int32_t invoke(Callback f) {
    return f(-1, 2.5, -3, 4, 5, 6, 7, 8, 65000, -10) + f(-1, 2.5, -3, 4, 5, 6, 7, 8, 65000, -10);
}
double mixed(int64_t a, double f, int32_t b, float g, uint8_t c, int64_t d,
             int32_t e, int64_t h, int32_t i, int64_t j, uint8_t k, uint16_t l,
             float m, double n, float o, double p, float q, double r,
             float s, double t, uint32_t u) {
    return a == -1 && f == 2.5 && b == -3 && g == 4.5 && c == 250 && d == 6
        && e == -7 && h == 8 && i == -9 && j == 10 && k == 244 && l == 65000
        && m == 13.5 && n == 14.5 && o == 15.5 && p == 16.5 && q == 17.5
        && r == 18.5 && s == 19.5 && t == 20.5 && u == 4000000000u ? 42.25 : 0;
}

typedef struct { uint8_t byte; uint16_t short_value; } Small;
typedef struct {
    uint8_t lead; Small pair; uint8_t buffer[3]; size_t length;
    const char *data; uint64_t stamp;
} Packet;

size_t inspect_packet(Packet *p, size_t size, size_t alignment) {
    if (size != sizeof(Packet) || alignment != _Alignof(Packet) ||
        p->lead != 250 || p->pair.byte != 18 || p->pair.short_value != 61520 ||
        p->buffer[0] != 1 || p->buffer[2] != 27 || strcmp(p->data, "dunder") ||
        p->length != 6 || p->stamp != UINT64_C(0xfedcba9876543210)) return 0;
    p->pair.short_value = 57777;
    return offsetof(Packet, stamp);
}
uint32_t first_nibble(const uint8_t *data) { return data[0] & 15; }
int32_t invoke_inner(int32_t (*callback)(int32_t), int32_t n) { return callback(n); }

typedef struct { uint8_t tag, rgba[256 * 256 * 4]; uint32_t stamp; } Tile;
typedef struct { uint16_t first[20000]; uint32_t second[10000]; uint64_t tail; } Planes;

int tile_layout(Tile *tile, size_t size, size_t alignment, size_t pixels, size_t stamp) {
    if (size != sizeof *tile || alignment != _Alignof(Tile) ||
        pixels != offsetof(Tile, rgba) || stamp != offsetof(Tile, stamp) ||
        tile->rgba[sizeof tile->rgba - 1] != 17 || tile->stamp != 111) return 0;
    tile->rgba[0] = 23;
    return 1;
}

int planes_layout(Planes *p, size_t size, size_t alignment, size_t second, size_t tail) {
    if (size != sizeof *p || alignment != _Alignof(Planes) ||
        second != offsetof(Planes, second) || tail != offsetof(Planes, tail) ||
        p->first[19999] != 999 || p->tail != 88) return 0;
    p->second[9999] = 777;
    return 1;
}

size_t sum_bytes(const uint8_t *bytes, size_t length) {
    size_t sum = 0;
    for (size_t i = 0; i < length; i++) sum += bytes[i];
    return sum;
}

void *copy_bytes(const uint8_t *bytes, size_t length) {
    void *owned = malloc(length);
    if (owned && length) memcpy(owned, bytes, length);
    return owned;
}

void release_bytes(void *owned) { free(owned); }

int wide_counts(size_t count, ptrdiff_t distance) {
    return count == (size_t)2147483652ULL && distance == -(ptrdiff_t)1;
}
typedef int16_t (*SampleCallback)(int16_t, int16_t, int16_t, int16_t, int16_t,
                                  int16_t, int16_t, int16_t, int16_t);
int16_t call_samples(SampleCallback callback, int16_t lo, int16_t hi, int16_t a,
                     int16_t b, int16_t c, int16_t d, int16_t e, int16_t f, int16_t last) {
    if (lo != INT16_MIN || hi != INT16_MAX || a != -2 || b != 3 || c != -4
        || d != 5 || e != -6 || f != 7 || last != -8) return 0;
    return callback(hi, a, b, c, d, e, f, last, lo);
}

typedef struct { uint8_t v; } S1;
typedef struct { uint8_t a, b, c; } S3;
typedef struct { uint32_t a, b; } S8;
typedef struct { uint32_t a, b, c; } S12;
typedef struct { uint64_t location, length; } Range;
typedef struct { uint64_t x, y, z; } Origin;
typedef struct { uint64_t width, height, depth; } Extent;
typedef struct { Origin origin; Extent size; } Region;
typedef struct { uint64_t x, y, width, height; } Scissor;
typedef struct { double origin_x, origin_y, width, height, near_z, far_z; } Viewport;

uint64_t arm64_take_s1_s3(S1 a, S3 b) { return (uint64_t)a.v + b.a * 10 + b.b * 100 + b.c * 1000; }
uint64_t arm64_take_s8_s12(S8 a, S12 b) { return a.a + a.b + b.a + b.b + b.c; }
uint64_t arm64_take_range_x3(void *p0, void *p1, void *p2, Range r) {
    return (uint64_t)(uintptr_t)p0 + (uint64_t)(uintptr_t)p1 + (uint64_t)(uintptr_t)p2 + r.location * 10 + r.length;
}
uint64_t arm64_spill_range(uint64_t a0, uint64_t a1, uint64_t a2, uint64_t a3, uint64_t a4, uint64_t a5, uint64_t a6, Range spilled, S3 tail_s3, uint8_t tail_u8) {
    return a0 + a1 + a2 + a3 + a4 + a5 + a6 + spilled.location + spilled.length + tail_s3.c + tail_u8;
}
uint64_t arm64_take_region_scissor(Region reg, Scissor sc) {
    return reg.origin.x + reg.origin.y + reg.origin.z + reg.size.width + reg.size.height + reg.size.depth +
           sc.x + sc.y + sc.width + sc.height;
}
double arm64_take_viewport_and_mutate_local(Viewport v) {
    double sum = v.origin_x + v.origin_y + v.width + v.height + v.near_z + v.far_z;
    v.origin_x = -999.0;
    v.far_z = -999.0;
    return sum;
}
S3 arm64_ret_s3(uint8_t a, uint8_t b, uint8_t c) { return (S3){a, b, c}; }
S8 arm64_ret_s8(uint32_t a, uint32_t b) { return (S8){a, b}; }
S12 arm64_ret_s12(uint32_t a, uint32_t b, uint32_t c) { return (S12){a, b, c}; }
Range arm64_ret_range(uint64_t loc, uint64_t len) { return (Range){loc, len}; }
Origin arm64_ret_origin(uint64_t x, uint64_t y, uint64_t z) { return (Origin){x, y, z}; }
Region arm64_ret_region(uint64_t x, uint64_t y, uint64_t w, uint64_t h) {
    return (Region){{x, y, 7}, {w, h, 2}};
}
