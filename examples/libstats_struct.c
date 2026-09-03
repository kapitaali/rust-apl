// libstats_struct.c — Struct example for C FFI
// Compile: gcc -shared -fPIC -o libstats_struct.so libstats_struct.c

typedef struct {
    double re;
    double im;
} Complex;

typedef struct {
    double min;
    double max;
    double mean;
} Stats;

// Return Stats struct as F8[3] output buffer (simpler than struct return)
void compute_stats(double* arr, int len, double* out) {
    if (len <= 0) return;
    double mn = arr[0], mx = arr[0], sum = 0;
    for (int i = 0; i < len; i++) {
        if (arr[i] < mn) mn = arr[i];
        if (arr[i] > mx) mx = arr[i];
        sum += arr[i];
    }
    out[0] = mn;
    out[1] = mx;
    out[2] = sum / len;
}
