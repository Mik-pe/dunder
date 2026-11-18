#include <dispatch/dispatch.h>
#include <pthread.h>
#include <sys/mman.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdlib.h>
#include <errno.h>
#ifdef DUNDER_SCOPE_PROBE
#include <stdio.h>
#endif
typedef dispatch_function_t start_fn;
extern int abi_probe(start_fn, void *);
static _Atomic unsigned creates, joins, unmaps, starts, preserved;
#ifdef DUNDER_SCOPE_PROBE
static _Atomic unsigned active, peak, drops, bad_drop, threads;
static _Thread_local int seen_work;

static pthread_mutex_t gate_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t gate_cond = PTHREAD_COND_INITIALIZER;
static unsigned arrived;
static int gate_open;
static void begin_active(void) {
    if (!seen_work) { seen_work = 1; atomic_fetch_add(&threads, 1); }
    unsigned now = atomic_fetch_add(&active, 1) + 1;
    unsigned old = atomic_load(&peak);
    while (old < now && !atomic_compare_exchange_weak(&peak, &old, now)) {}
    const char *cap = getenv("TASK_SCOPE_CAP");
    if (cap && atoi(cap) >= 2) {
        pthread_mutex_lock(&gate_lock);
        if (!gate_open) {
            if (++arrived == 2) { gate_open = 1; pthread_cond_broadcast(&gate_cond); }
            else while (!gate_open) pthread_cond_wait(&gate_cond, &gate_lock);
        }
        pthread_mutex_unlock(&gate_lock);
    }
}
#endif
struct packet { start_fn start; void *control; };
static int fault(void) {
    const char *value = getenv("TASK_NATIVE_FAULT");
    return value ? atoi(value) : 0;
}
static void probe_start(void *raw) {
    struct packet *p = raw;
    atomic_fetch_add(&starts, 1);
#ifdef DUNDER_SCOPE_PROBE
    begin_active();
#endif
    int good = abi_probe(p->start, p->control);
    if (good) atomic_fetch_add(&preserved, 1);
#ifdef DUNDER_SCOPE_PROBE
    atomic_fetch_sub(&active, 1);
#endif
    free(p);
}
dispatch_group_t probe_create(void) {
    atomic_fetch_add(&creates, 1);
#ifdef DUNDER_SCOPE_PROBE
    if (fault() == 1 && atomic_load(&creates) == 2) return NULL;
#else
    if (fault() == 1) return NULL;
#endif
    return dispatch_group_create();
}
void probe_submit(dispatch_group_t group, dispatch_queue_t queue, void *arg, start_fn fn) {
    struct packet *p = malloc(sizeof(*p));
    if (!p) abort();
    *p = (struct packet){ fn, arg };
    dispatch_group_async_f(group, queue, p, probe_start);
}
long probe_wait(dispatch_group_t group, dispatch_time_t timeout) {
    atomic_fetch_add(&joins, 1);
    long error = dispatch_group_wait(group, timeout);
    if (!error && fault() == 2) return EINVAL;
    return error;
}
int probe_unmap(void *base, size_t bytes) {
    atomic_fetch_add(&unmaps, 1);
    if (fault() == 3) { errno = EINVAL; return -1; }
    return munmap(base, bytes);
}
int probe_check(int error, int jobs, int array) {
    unsigned expected_starts = error == 1 ? 0 : (unsigned)jobs;
    unsigned expected_unmaps = error == 1 || error == 2 ? 0 : (unsigned)jobs;
    return atomic_load(&creates) != (unsigned)jobs ||
        atomic_load(&joins) != expected_starts ||
        atomic_load(&unmaps) != expected_unmaps ||
        atomic_load(&starts) != expected_starts ||
        atomic_load(&preserved) != expected_starts;
}

#ifdef DUNDER_SCOPE_PROBE
void scope_drop(void) {
    atomic_fetch_add(&drops, 1);
    if (atomic_load(&active)) atomic_fetch_add(&bad_drop, 1);
}
int scope_check(int mode, int jobs, int array, int cap, int expected_drops) {
    unsigned c = atomic_load(&creates), j = atomic_load(&joins), u = atomic_load(&unmaps);
    unsigned d = atomic_load(&drops), p = atomic_load(&peak), a = atomic_load(&active);
    if (atomic_load(&bad_drop)) return 1;
    if (mode == 0) {
        if (c != (unsigned)jobs || j != c || u != (unsigned)cap || a ||
            atomic_load(&starts) != c || atomic_load(&preserved) != c ||
            d != (unsigned)expected_drops || !p || p > (unsigned)cap) return 2;
        if (cap == 1 ? p != 1 : p < 2) return 3;
        if (getenv("TASK_THREAD_REPORT")) { printf("%u\n", atomic_load(&threads)); fflush(stdout); }
        return 0;
    }
    if (mode == 1) return c != 2 || j || u || d;
    if (mode == 2) return c != 2 || j != 1 || u || d;
    if (mode == 3) return c != (unsigned)jobs || j != c - cap + 1 || u != 1 || d;
    if (mode == 4) return c != 1 || j || u || d;
    if (mode == 5) return c != 2 || j != 1 || u != 1 || d != (unsigned)array;
    if (mode == 6) return c || j || u || d;
    return 4;
}
#endif
