#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

/* ---- 1. i8, isize, and bool normalization ---- */
int8_t echo_i8(int8_t v) { return v; }
int32_t sum_i8_pair(int8_t a, int8_t b) { return (int32_t)a + (int32_t)b; }
int8_t ret_neg_i8(void) { return -42; }

intptr_t echo_isize(intptr_t v) { return v; }
intptr_t add_isize(intptr_t a, intptr_t b) { return a + b; }

/* Return raw byte 2 / 255 / 0 (same 1-byte register ABI as C _Bool)
   to verify Dunder normalizes any non-zero inbound byte to true (1) and 0 to false (0). */
uint8_t ret_dirty_bool(uint8_t raw) {
    return raw;
}

int32_t inspect_bool(uint8_t b) {
    return (int32_t)b;
}

typedef uint8_t (*dirty_bool_cb_t)(uint8_t);
int32_t call_bool_cb(dirty_bool_cb_t cb, uint8_t raw_in) {
    return (int32_t)cb(raw_in);
}

/* ---- 2. Typed raw pointers & opaque handles ---- */
int32_t sum_const_i32(const int32_t *p, size_t n) {
    int32_t acc = 0;
    for (size_t i = 0; i < n; i++) acc += p[i];
    return acc;
}

void scale_mut_i32(int32_t *p, size_t n, int32_t k) {
    for (size_t i = 0; i < n; i++) p[i] *= k;
}

const int32_t *maybe_ptr(const int32_t *p, int nonnull) {
    return nonnull ? p : NULL;
}

struct HandleA { int64_t magic; int32_t value; };
struct HandleB { int64_t magic; int32_t value; };

struct HandleA *create_handle_a(int32_t val) {
    struct HandleA *h = (struct HandleA *)malloc(sizeof(struct HandleA));
    if (!h) return NULL;
    h->magic = 0x41414141LL;
    h->value = val;
    return h;
}

int32_t read_handle_a(const struct HandleA *h) {
    if (!h || h->magic != 0x41414141LL) return -1;
    return h->value;
}

void destroy_handle_a(struct HandleA *h) { free(h); }

/* ---- 3. @repr(C, packed), @repr(C, packed(2)), @repr(C, align(16)) ---- */
struct __attribute__((packed)) Packed1 {
    uint8_t tag;
    uint32_t id;
    int16_t delta;
};

#pragma pack(push, 2)
struct Packed2 {
    uint8_t tag;
    uint32_t id;
    int16_t delta;
};
#pragma pack(pop)

struct __attribute__((aligned(16))) Aligned16 {
    int32_t a;
    int32_t b;
};

typedef struct LayoutMetrics {
    size_t size;
    size_t align;
    size_t off0;
    size_t off1;
    size_t off2;
} LayoutMetrics;

LayoutMetrics packed1_metrics(void) {
    LayoutMetrics m = {
        sizeof(struct Packed1), _Alignof(struct Packed1),
        offsetof(struct Packed1, tag), offsetof(struct Packed1, id), offsetof(struct Packed1, delta)
    };
    return m;
}

LayoutMetrics packed2_metrics(void) {
    LayoutMetrics m = {
        sizeof(struct Packed2), _Alignof(struct Packed2),
        offsetof(struct Packed2, tag), offsetof(struct Packed2, id), offsetof(struct Packed2, delta)
    };
    return m;
}

LayoutMetrics aligned16_metrics(void) {
    LayoutMetrics m = {
        sizeof(struct Aligned16), _Alignof(struct Aligned16),
        offsetof(struct Aligned16, a), offsetof(struct Aligned16, b), 0
    };
    return m;
}

int64_t inspect_packed1(const struct Packed1 *p) {
    uint32_t id = 0;
    int16_t delta = 0;
    memcpy(&id, &p->id, sizeof(id));
    memcpy(&delta, &p->delta, sizeof(delta));
    return (int64_t)p->tag * 100000 + (int64_t)id + (int64_t)delta;
}

/* ---- 4. First-class @repr(C) unions (SysV INTEGER > SSE merge + AAPCS64) ---- */
typedef union ClearValue {
    float f32x4[4];
    int32_t i32x4[4];
    uint32_t u32x4[4];
} ClearValue;

typedef union WlArg {
    int32_t i;
    uint32_t u;
    double d;
    const char *s;
} WlArg;

ClearValue roundtrip_clear_value_i(ClearValue v) {
    ClearValue out;
    out.i32x4[0] = v.i32x4[0] + 1;
    out.i32x4[1] = v.i32x4[1] + 2;
    out.i32x4[2] = v.i32x4[2] + 3;
    out.i32x4[3] = v.i32x4[3] + 4;
    return out;
}

ClearValue roundtrip_clear_value_f(ClearValue v) {
    ClearValue out;
    out.f32x4[0] = v.f32x4[0] * 2.0f;
    out.f32x4[1] = v.f32x4[1] * 2.0f;
    out.f32x4[2] = v.f32x4[2] * 2.0f;
    out.f32x4[3] = v.f32x4[3] * 2.0f;
    return out;
}

WlArg roundtrip_wl_arg_d(WlArg a) {
    WlArg out;
    out.d = a.d + 1.5;
    return out;
}

WlArg roundtrip_wl_arg_i(WlArg a) {
    WlArg out;
    memset(&out, 0, sizeof(out));
    out.i = a.i * 3;
    return out;
}

/* ---- 5. C-ABI open enums ---- */
int32_t open_enum_i32_echo(int32_t v) { return v; }
int32_t open_enum_i32_unknown(void) { return -1000001004; }
uint32_t open_enum_u32_unknown(void) { return 0xDEADBEEFu; }
int8_t open_enum_i8_unknown(void) { return -120; }

/* ---- 6. Partial ..zeroed() padding verification ---- */
typedef struct PaddedZero {
    uint8_t tag;
    int64_t big;
    uint16_t tail;
} PaddedZero;

int32_t verify_padded_zeroed(const PaddedZero *p, uint8_t expected_tag) {
    PaddedZero expected;
    memset(&expected, 0, sizeof(expected));
    expected.tag = expected_tag;
    return memcmp(p, &expected, sizeof(PaddedZero)) == 0 ? 1 : 0;
}

/* ---- 7. Nullable extern fn (Option<extern fn(...) -> ...>) ---- */
typedef int32_t (*int_fn_t)(int32_t);

static int32_t c_double_i32(int32_t x) { return x * 2; }

int32_t invoke_optional_cb(int_fn_t cb, int32_t x) {
    if (!cb) return -999;
    return cb(x);
}

int_fn_t get_optional_cb(int want_nonnull) {
    return want_nonnull ? c_double_i32 : NULL;
}

typedef struct CallbackTable {
    int32_t tag;
    int_fn_t handler;
} CallbackTable;

int32_t call_table_handler(CallbackTable tbl, int32_t x) {
    if (!tbl.handler) return -tbl.tag;
    return tbl.handler(x) + tbl.tag;
}

/* ---- 8. c"..." and w"..." static literals ---- */
int32_t check_cstr_eq(const char *a, const char *b) {
    return strcmp(a, b) == 0 ? 1 : 0;
}

size_t cstr_strlen(const char *s) {
    return strlen(s);
}

int32_t check_wstr_units(const uint16_t *w, size_t expected_len, uint16_t first, uint16_t last) {
    size_t n = 0;
    while (w[n] != 0) n++;
    if (n != expected_len) return -1;
    if (expected_len > 0 && w[0] != first) return -2;
    if (expected_len > 0 && w[expected_len - 1] != last) return -3;
    return 1;
}
