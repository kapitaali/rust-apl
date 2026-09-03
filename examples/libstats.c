// libstats.c — Array statistics library for ⎕NA demonstration
// Compile: gcc -shared -fPIC -o libstats.so libstats.c -lm

#include <math.h>
#include <stdlib.h>
#include <string.h>

// Comparison function for qsort (double)
static int cmp_double(const void* a, const void* b) {
    double da = *(const double*)a;
    double db = *(const double*)b;
    return (da > db) - (da < db);
}

// Mean of a double array
double mean(double* arr, int len) {
    if (len <= 0) return 0.0;
    double sum = 0.0;
    for (int i = 0; i < len; i++) sum += arr[i];
    return sum / len;
}

// Standard deviation (population)
double stddev(double* arr, int len) {
    if (len <= 1) return 0.0;
    double m = mean(arr, len);
    double sum = 0.0;
    for (int i = 0; i < len; i++) {
        double d = arr[i] - m;
        sum += d * d;
    }
    return sqrt(sum / len);
}

// Sample standard deviation (N-1)
double stddev_sample(double* arr, int len) {
    if (len <= 1) return 0.0;
    double m = mean(arr, len);
    double sum = 0.0;
    for (int i = 0; i < len; i++) {
        double d = arr[i] - m;
        sum += d * d;
    }
    return sqrt(sum / (len - 1));
}

// Min value
double array_min(double* arr, int len) {
    if (len <= 0) return 0.0;
    double m = arr[0];
    for (int i = 1; i < len; i++) if (arr[i] < m) m = arr[i];
    return m;
}

// Max value
double array_max(double* arr, int len) {
    if (len <= 0) return 0.0;
    double m = arr[0];
    for (int i = 1; i < len; i++) if (arr[i] > m) m = arr[i];
    return m;
}

// Sum of all elements
double sum_array(double* arr, int len) {
    double s = 0.0;
    for (int i = 0; i < len; i++) s += arr[i];
    return s;
}

// Median (sorts a copy)
double median(double* arr, int len) {
    if (len <= 0) return 0.0;
    double* copy = (double*)malloc(len * sizeof(double));
    memcpy(copy, arr, len * sizeof(double));
    qsort(copy, len, sizeof(double), cmp_double);
    double result;
    if (len % 2 == 0)
        result = (copy[len/2 - 1] + copy[len/2]) / 2.0;
    else
        result = copy[len/2];
    free(copy);
    return result;
}

// Sort array in-place
int sort(double* arr, int len) {
    qsort(arr, len, sizeof(double), cmp_double);
    return 0;
}

// Matrix multiplication: out = a * b
// a is rows_a x cols_a, b is cols_a x cols_b, out is rows_a x cols_b
void matmul(double* a, double* b, double* out, int rows_a, int cols_a, int cols_b) {
    for (int i = 0; i < rows_a; i++) {
        for (int j = 0; j < cols_b; j++) {
            double s = 0.0;
            for (int k = 0; k < cols_a; k++) {
                s += a[i * cols_a + k] * b[k * cols_b + j];
            }
            out[i * cols_b + j] = s;
        }
    }
}

// Determinant of n x n matrix (recursive)
double determinant(double* mat, int n) {
    if (n == 1) return mat[0];
    if (n == 2) return mat[0] * mat[3] - mat[1] * mat[2];
    
    double det = 0.0;
    double* sub = (double*)malloc((n-1) * (n-1) * sizeof(double));
    
    for (int col = 0; col < n; col++) {
        int si = 0;
        for (int i = 1; i < n; i++) {
            int sj = 0;
            for (int j = 0; j < n; j++) {
                if (j == col) continue;
                sub[si * (n-1) + sj] = mat[i * n + j];
                sj++;
            }
            si++;
        }
        double sign = (col % 2 == 0) ? 1.0 : -1.0;
        det += sign * mat[col] * determinant(sub, n-1);
    }
    
    free(sub);
    return det;
}

// Transpose: out = mat^T
// mat is rows x cols, out is cols x rows
void transpose(double* mat, double* out, int rows, int cols) {
    for (int i = 0; i < rows; i++) {
        for (int j = 0; j < cols; j++) {
            out[j * rows + i] = mat[i * cols + j];
        }
    }
}

// Normalize array to [0, 1] range (in-place)
void normalize(double* arr, int len) {
    if (len <= 0) return;
    double mn = array_min(arr, len);
    double mx = array_max(arr, len);
    double range = mx - mn;
    if (range == 0.0) return;
    for (int i = 0; i < len; i++) {
        arr[i] = (arr[i] - mn) / range;
    }
}

// Correlation coefficient between two arrays
double correlation(double* a, double* b, int len) {
    if (len <= 1) return 0.0;
    double ma = mean(a, len);
    double mb = mean(b, len);
    double num = 0.0, den_a = 0.0, den_b = 0.0;
    for (int i = 0; i < len; i++) {
        double da = a[i] - ma;
        double db = b[i] - mb;
        num += da * db;
        den_a += da * da;
        den_b += db * db;
    }
    double denom = sqrt(den_a * den_b);
    if (denom == 0.0) return 0.0;
    return num / denom;
}
