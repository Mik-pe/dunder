/* DEBUG TEST INTERPOSER ONLY. No Dunder production test hooks.
 * Each fault changes one foreign result, retaining the real process lifecycle.
 * wait_probe first reaps its child so its deliberate ECHILD leaves no orphan.
 */
#include <errno.h>
#include <dlfcn.h>
#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#ifndef __APPLE__
#error This focused fault interposer uses Darwin dyld interposition.
#endif
_Static_assert(sizeof(sigset_t) == 4, "Darwin signal-set ABI changed");
_Static_assert(SIGPIPE == 13 && SIG_BLOCK == 1, "Darwin signal constants changed");

enum fault { NONE, FORK_FAIL, START_CLOCK_FAIL, CHILD_GROUP_FAIL,
    PARENT_GROUP_FAIL, WAIT_PROBE_FAIL, WAIT_INTERRUPT, MASK_FAIL,
    CHILD_GROUP_MASK_FAIL, NO_ERRNO, CURRENT_CLOCK_FAIL, GROUP_KILL_FAIL,
    FALLBACK_KILL_FAIL, SLEEP_INTERRUPT };
static enum fault selected;
static pid_t parent;
static int interrupted;
static int clocks;

__attribute__((constructor)) static void configure(void) {
    parent = getpid();
    const char *name = getenv("DUNDER_PROCESS_DIAGNOSTIC_FAULT");
    if (!name) return;
    if (!strcmp(name, "fork")) selected = FORK_FAIL;
    if (!strcmp(name, "start_clock")) selected = START_CLOCK_FAIL;
    if (!strcmp(name, "child_group")) selected = CHILD_GROUP_FAIL;
    if (!strcmp(name, "parent_group")) selected = PARENT_GROUP_FAIL;
    if (!strcmp(name, "wait_probe")) selected = WAIT_PROBE_FAIL;
    if (!strcmp(name, "wait_interrupt")) selected = WAIT_INTERRUPT;
    if (!strcmp(name, "mask")) selected = MASK_FAIL;
    if (!strcmp(name, "child_group_mask")) selected = CHILD_GROUP_MASK_FAIL;
    if (!strcmp(name, "no_errno")) selected = NO_ERRNO;
    if (!strcmp(name, "current_clock")) selected = CURRENT_CLOCK_FAIL;
    if (!strcmp(name, "group_kill")) selected = GROUP_KILL_FAIL;
    if (!strcmp(name, "fallback_kill")) selected = FALLBACK_KILL_FAIL;
    if (!strcmp(name, "sleep_interrupt")) selected = SLEEP_INTERRUPT;
}
static pid_t fault_fork(void) {
    if (selected == FORK_FAIL) { errno = EAGAIN; return -1; }
    return fork();
}
static int fault_clock(clockid_t which, struct timespec *value) {
    if (selected == START_CLOCK_FAIL && getpid() == parent) { errno = EINVAL; return -1; }
    if (selected == CURRENT_CLOCK_FAIL && getpid() == parent && clocks++ > 0) { errno = EINVAL; return -1; }
    return clock_gettime(which, value);
}
static int fault_group(pid_t pid, pid_t group) {
    if ((selected == CHILD_GROUP_FAIL || selected == CHILD_GROUP_MASK_FAIL) && getpid() != parent && pid == 0) {
        errno = EPERM; return -1;
    }
    if (selected == PARENT_GROUP_FAIL && getpid() == parent) { errno = EACCES; return -1; }
    return setpgid(pid, group);
}
static pid_t fault_wait(pid_t pid, int *status, int options) {
    if (selected == WAIT_PROBE_FAIL && getpid() == parent) {
        pid_t result;
        do { result = waitpid(pid, status, 0); } while (result < 0 && errno == EINTR);
        if (result != pid) _exit(90);
        errno = ECHILD; return -1;
    }
    if (selected == WAIT_INTERRUPT && getpid() == parent && !interrupted) {
        interrupted = 1; errno = EINTR; return -1;
    }
    return waitpid(pid, status, options);
}
static int fault_mask(int how, const sigset_t *set, sigset_t *old) {
    if ((selected == MASK_FAIL || selected == CHILD_GROUP_MASK_FAIL) && getpid() != parent) {
        errno = EIO; return -1;
    }
    return sigprocmask(how, set, old);
}
static int fault_kill(pid_t pid, int signal) {
    if ((selected == GROUP_KILL_FAIL || selected == FALLBACK_KILL_FAIL) && signal == SIGKILL && pid < 0) {
        errno = EACCES; return -1;
    }
    if (selected == FALLBACK_KILL_FAIL && signal == SIGKILL && pid > 0) { errno = EPERM; return -1; }
    return kill(pid, signal);
}
static int fault_sleep(useconds_t interval) {
    if (selected == SLEEP_INTERRUPT && interval == 250 && !interrupted) {
        interrupted = 1; errno = EINTR; return -1;
    }
    return usleep(interval);
}

/* Fail only the optional dlopen(NULL) lookup, leaving named libc imports and
 * libc's own errno access intact. Execution must still retain its old code. */
static void *fault_open(const char *path, int flags) {
    if (selected == NO_ERRNO && path == NULL && getpid() == parent) return NULL;
    return dlopen(path, flags);
}
#define INTERPOSE(replacement, original) \
    __attribute__((used, section("__DATA,__interpose"))) \
    static const struct { const void *new_function; const void *old_function; } \
    interpose_##replacement = { (const void *)(replacement), (const void *)(original) }
INTERPOSE(fault_fork, fork);
INTERPOSE(fault_clock, clock_gettime);
INTERPOSE(fault_group, setpgid);
INTERPOSE(fault_wait, waitpid);
INTERPOSE(fault_mask, sigprocmask);
INTERPOSE(fault_kill, kill);
INTERPOSE(fault_sleep, usleep);
INTERPOSE(fault_open, dlopen);
