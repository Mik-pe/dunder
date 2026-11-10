#include <stdint.h>
#include <pthread.h>
#include <stddef.h>
#include <string.h>
typedef void (*NoArgs)(void);
static void *from_worker(void *arg) { (*(NoArgs *)arg)(); return 0; }
void on_worker(NoArgs f) { pthread_t t; if (!pthread_create(&t, 0, from_worker, &f)) pthread_join(t, 0); }
typedef struct { float a, b, c; } Triple;
typedef struct { double a, b; } Pair;
typedef struct { Pair origin, size; } Rect;
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
