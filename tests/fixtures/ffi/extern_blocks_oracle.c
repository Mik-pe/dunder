#include <pthread.h>
#include <stdint.h>
#include <stddef.h>
#include <string.h>

typedef struct {
    const char *name;
    int32_t version;
    int32_t method_count;
} MockInterface;

typedef struct {
    int32_t a;
    double b;
} Pair;

typedef union {
    int32_t i;
    float f;
} ValueU;

const MockInterface demo_interface = {
    "wl_compositor_mock",
    5,
    2
};

int32_t global_counter = 100;

int32_t read_global_counter(void) {
    return global_counter;
}

int32_t inspect_interface_ptr(const MockInterface *iface) {
    if (iface != &demo_interface) return -1;
    if (strcmp(iface->name, "wl_compositor_mock") != 0) return -2;
    return iface->version * 10 + iface->method_count;
}

int32_t c_oracle_combine(Pair p, ValueU u, int32_t enum_val) {
    return p.a + (int32_t)p.b + u.i + enum_val;
}

typedef struct {
    int32_t (*cb)(int32_t, int32_t);
    int32_t x;
    int32_t y;
    int32_t out;
} ThreadArgs;

static void *thread_entry(void *raw) {
    ThreadArgs *args = (ThreadArgs *)raw;
    args->out = args->cb(args->x, args->y);
    return NULL;
}

int32_t run_on_pthread(int32_t (*cb)(int32_t, int32_t), int32_t x, int32_t y) {
    ThreadArgs args = { cb, x, y, 0 };
    pthread_t th;
    if (pthread_create(&th, NULL, thread_entry, &args) != 0) return -1;
    pthread_join(th, NULL);
    return args.out;
}

typedef struct {
    Pair (*cb)(Pair);
    Pair in_val;
    Pair out_val;
} StructThreadArgs;

static void *struct_thread_entry(void *raw) {
    StructThreadArgs *args = (StructThreadArgs *)raw;
    args->out_val = args->cb(args->in_val);
    return NULL;
}

Pair run_struct_cb_on_pthread(Pair (*cb)(Pair), Pair input) {
    StructThreadArgs args = { cb, input, { 0, 0.0 } };
    pthread_t th;
    if (pthread_create(&th, NULL, struct_thread_entry, &args) != 0) {
        Pair err = { -1, -1.0 };
        return err;
    }
    pthread_join(th, NULL);
    return args.out_val;
}

int32_t invoke_opt_cb(int32_t (*cb)(int32_t), int32_t x) {
    if (cb == NULL) return -999;
    return cb(x);
}

int32_t call_reentrant(int32_t (*cb)(int32_t), int32_t n) {
    return cb(n);
}
