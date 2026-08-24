/* testlib.c — native functions for ⎕NA end-to-end tests.
 * Build: cc -shared -fPIC -o libtestmath.so testlib.c
 */
#include <stdint.h>
#include <string.h>

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

/* struct demo (F3c): {I4 F8} pair */
typedef struct {
    int32_t tag;
    double weight;
} pair_t;

double struct_weight(pair_t p) {
    return p.weight * p.tag;
}

/* struct out-arg: fills the result struct from two scalars */
void make_pair(int32_t tag, double weight, pair_t *out) {
    out->tag = tag;
    out->weight = weight;
}

int32_t sum_i4(int32_t *v, int32_t n) {
    int32_t s = 0;
    for (int32_t i = 0; i < n; i++) s += v[i];
    return s;
}

/* fill n doubles with i*1.5 (output pointer) */
void fill_f8(double *out, int32_t n) {
    for (int32_t i = 0; i < n; i++) out[i] = (double)i * 1.5;
}

/* increment every element in place (in/out pointer) */
void bump_i4(int32_t *v, int32_t n, int32_t by) {
    for (int32_t i = 0; i < n; i++) v[i] += by;
}

/* reverse a NUL-terminated string into out */
void str_rev(char *out, const char *in) {
    size_t n = strlen(in);
    for (size_t i = 0; i < n; i++) out[i] = in[n - 1 - i];
    out[n] = 0;
}

/* uppercase copy of a NUL-terminated string */
void str_up(char *out, const char *in) {
    for (; *in; in++, out++) {
        *out = (*in >= 'a' && *in <= 'z') ? (char)(*in - 32) : *in;
    }
    *out = 0;
}
