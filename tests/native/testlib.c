/* testlib.c — native functions for ⎕NA end-to-end tests.
 * Build: cc -shared -fPIC -o libtestmath.so testlib.c
 */
#include <stdint.h>

double divide(int32_t a, int32_t b) {
    return (double)a / (double)b;
}

int64_t add64(int64_t a, int64_t b) {
    return a + b;
}

uint8_t clamp_u8(int32_t v) {
    if (v < 0) return 0;
    if (v > 255) return 255;
    return (uint8_t)v;
}

float fscale(float x, float k) {
    return x * k;
}

double dscale(double x, double k) {
    return x * k;
}

uintptr_t identity_ptr(uintptr_t p) {
    return p;
}
