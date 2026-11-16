#include <stdarg.h>
#include <stdint.h>

int variadic_mixed(int tag, ...) {
    va_list args;
    va_start(args, tag);
    int small = va_arg(args, int);
    unsigned int unsigned_small = va_arg(args, unsigned int);
    int64_t wide = va_arg(args, int64_t);
    uint64_t unsigned_wide = va_arg(args, uint64_t);
    double real = va_arg(args, double);
    void *pointer = va_arg(args, void *);
    int (*transform)(int) = va_arg(args, int (*)(int));
    double tail = va_arg(args, double);
    va_end(args);
    return tag == 123 && small == -7 && unsigned_small == UINT32_C(4026531840)
        && wide == INT64_C(-5000000000) && unsigned_wide == UINT64_C(18446744073709551600)
        && real == 2.5 && pointer != 0 && *(int *)pointer == 41
        && transform(41) == 42 && tail == -9.25;
}

int variadic_named_spill(int a, int b, int c, int d, int e, int f, int g, int h, int i, ...) {
    va_list args;
    va_start(args, i);
    int last = va_arg(args, int);
    double real = va_arg(args, double);
    va_end(args);
    return a + b + c + d + e + f + g + h + i == 45 && last == -42 && real == 6.5;
}

int64_t variadic_integer_banks(int count, ...) {
    va_list args;
    va_start(args, count);
    int64_t result = 0;
    for (int i = 0; i < count; ++i) result += va_arg(args, int64_t);
    va_end(args);
    return result;
}

double variadic_float_banks(double named, int count, ...) {
    va_list args;
    va_start(args, count);
    double result = named;
    for (int i = 0; i < count; ++i) result += va_arg(args, double);
    va_end(args);
    return result;
}

int variadic_no_extra(int named, ...) {
    return named + 1;
}

double ordinary_float_before_integer(double named, int value) {
    return named + value;
}
