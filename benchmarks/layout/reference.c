#include <stdint.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

static void advance(float *restrict x, float *restrict y, const float *restrict dx,
                    const float *restrict dy, int count, float dt) {
    for (int i = 0; i < count; ++i) {
        x[i] += dx[i] * dt;
        y[i] += dy[i] * dt;
    }
}

static int64_t weight(int i, float x, float y) {
    return (i % 13 + 1) * ((int64_t)x + 17 * (int64_t)y);
}

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    int frames = atoi(argv[1]), seed = atoi(argv[2]);
    if (frames < 0 || frames > 100000 || seed < 0 || seed > 1000000) return 2;
    enum { count = 4099 };
    float x[count], y[count], dx[count], dy[count];
    for (int i = 0; i < count; ++i) {
        x[i] = (float)((i * 3 + seed) % 127);
        y[i] = (float)((i * 5 + seed) % 61);
        dx[i] = (float)(i % 11 - 5);
        dy[i] = (float)(i % 7 - 3);
    }
    int64_t checksum = 0;
    for (int frame = 0; frame < frames; ++frame) {
        advance(x, y, dx, dy, count, frame % 3 == 1 ? -2.0f : 1.0f);
        int i = frame % count;
        checksum += weight(i, x[i], y[i]);
    }
    for (int i = 0; i < count; ++i) checksum += weight(i, x[i], y[i]);
    printf("%" PRId64 "\n", checksum);
    return 0;
}
