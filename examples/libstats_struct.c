// libstats_struct.c — Struct examples for C FFI
// Compile: gcc -shared -fPIC -o libstats_struct.so libstats_struct.c -lm

// Mixed-type struct (like a real-world summary)
typedef struct {
    double min;
    double max;
    double mean;
} Stats;

// Fill output struct
void compute_stats(double* arr, int len, Stats* out) {
    if (len <= 0) return;
    double mn = arr[0], mx = arr[0], sum = 0;
    for (int i = 0; i < len; i++) {
        if (arr[i] < mn) mn = arr[i];
        if (arr[i] > mx) mx = arr[i];
        sum += arr[i];
    }
    out->min = mn;
    out->max = mx;
    out->mean = sum / len;
}
