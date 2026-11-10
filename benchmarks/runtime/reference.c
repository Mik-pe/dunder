// Host-only C reference for the four frozen runtime benchmarks.
#include <stdint.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#if defined(CASE_numeric)
static int64_t run(int count, const char *arg) {
    int64_t value = strtoll(arg, NULL, 10), sum = 0;
    for (int i = 0; i < count; ++i) {
        value = (value * 1664525 + 1013904223) % 2147483647;
        sum += value;
    }
    return sum;
}
#elif defined(CASE_scan)
static int64_t run(int count, const char *arg) {
    int64_t seed = strtoll(arg, NULL, 10), result = 0;
    int64_t *values = malloc(1024 * sizeof(*values));
    if (!values) exit(109);
    for (int i = 0; i < 1024; ++i) values[i] = (i * 17 + seed) % 1009;
    for (int i = 0; i < count; ++i) {
        values[i % 1024] = i;
        int64_t sum = 0;
        for (int j = 0; j < 1024; ++j) sum += values[j];
        result += sum;
    }
    free(values);
    return result;
}
#elif defined(CASE_sort)
static int64_t run(int count, const char *arg) {
    int64_t seed = strtoll(arg, NULL, 10), sum = 0;
    int64_t *values = malloc(256 * sizeof(*values));
    if (!values) exit(109);
    for (int repeat = 0; repeat < count; ++repeat) {
        for (int i = 0; i < 256; ++i) values[i] = ((255-i)*17 + seed + repeat) % 1009;
        for (int i = 1; i < 256; ++i) {
            int j = i;
            int64_t v = values[j];
            while (j > 0 && values[j-1] > v) { values[j] = values[j-1]; --j; }
            values[j] = v;
        }
        for (int i = 0; i < 256; ++i) sum += values[i] * (i + 1);
    }
    free(values);
    return sum;
}
#elif defined(CASE_text)
static uint32_t scalar(const unsigned char **cursor) {
    unsigned byte = *(*cursor)++;
    if (byte < 128) return byte;
    if (byte < 0xc2 || byte > 0xf4) exit(106);
    int n = byte < 0xe0 ? 1 : byte < 0xf0 ? 2 : 3;
    uint32_t value = byte & (0x7f >> (n+1));
    uint32_t min = n == 1 ? 0x80 : n == 2 ? 0x800 : 0x10000;
    while (n--) {
        unsigned next = *(*cursor)++;
        if ((next & 0xc0) != 0x80) exit(106);
        value = (value << 6) | (next & 63);
    }
    if (value < min || value > 0x10ffff || (value >= 0xd800 && value <= 0xdfff)) exit(106);
    return value;
}
static int64_t inspect(const char *arg) {
    int64_t words = 0, characters = 0;
    int inside = 0;
    const unsigned char *p = (const unsigned char *)arg;
    while (*p) {
        uint32_t s = scalar(&p);
        characters += s;
        if (s == ' ' || s == '\n' || s == '\t') inside = 0;
        else if (!inside) { ++words; inside = 1; }
    }
    return words * 1000000 + characters;
}
static int64_t run(int count, const char *arg) {
    int64_t result = 0;
    for (int i = 0; i < count; ++i) result += inspect(arg);
    return result;
}
#endif
int main(int argc, char **argv) {
    if (argc != 3) return 2;
    int count = atoi(argv[1]);
    printf("%" PRId64 "\n", run(count, argv[2]));
    return 0;
}
