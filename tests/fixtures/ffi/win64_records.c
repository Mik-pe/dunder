#include <stdarg.h>

typedef struct { int x, y; } Point;
typedef struct { double x, y; } Pair;

__declspec(dllexport) Point rotate(Point p) {
    return (Point){p.y, p.x};
}

__declspec(dllexport) Pair combine(Pair a, int k, Point p, Pair b, Point q) {
    return (Pair){a.x + b.y + k, p.y + q.x};
}

__declspec(dllexport) double sum(double first, ...) {
    va_list ap;
    va_start(ap, first);
    double second = va_arg(ap, double);
    double third = va_arg(ap, double);
    va_end(ap);
    return first + second + third;
}
