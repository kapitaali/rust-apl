/* Test program for the libapl C API */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "libapl.h"

int main(void)
{
    int failures = 0;
    printf("=== libapl C API test ===\n");

    /* Initialize the interpreter */
    init_libapl("test", 0);
    printf("1. init_libapl: OK\n");

    /* Execute some APL */
    LIBAPL_error err = apl_exec("x ← 42");
    printf("2. apl_exec 'x ← 42': err=0x%X (%s)\n", err,
           err == LAE_NO_ERROR ? "OK" : "FAIL");
    if (err != LAE_NO_ERROR) failures++;

    /* Retrieve the variable */
    APL_value v = get_var_value("x", LOC);
    if (v == NULL) {
        printf("3. get_var_value FAILED\n");
        return 1;
    }
    printf("3. get_var_value: OK\n");

    /* Inspect it */
    int rank = get_rank(v);
    uint64_t count = get_element_count(v);
    int64_t ival = get_int(v, 0);
    int type = get_type(v, 0);
    printf("4. x: rank=%d, count=%lu, [0]=%ld, type=0x%X (expect 0x10)\n",
           rank, (unsigned long)count, (long)ival, type);
    if (type != 0x10) { printf("   FAIL: expected CCT_INT (0x10)\n"); failures++; }

    /* Print it */
    printf("5. print_value: ");
    print_value(v, stdout);
    printf("\n");

    /* Release it */
    release_value(v, LOC);
    printf("6. release_value: OK\n");

    /* Test a math expression */
    err = apl_exec("y ← 3 + 4 × 5");
    printf("7. apl_exec 'y ← 3 + 4 × 5': err=0x%X\n", err);
    v = get_var_value("y", LOC);
    if (v) {
        printf("8. y = %ld\n", (long)get_int(v, 0));
        if (get_int(v, 0) != 23) { printf("   FAIL: expected 23\n"); failures++; }
        release_value(v, LOC);
    }

    /* Test ⎕PP */
    const char * cmd_out = apl_command(")VARS");
    if (cmd_out) {
        printf("9. )VARS:\n%s\n", cmd_out);
        free((void*)cmd_out);
    }

    /* Test char vector */
    err = apl_exec("s ← 'hello'");
    printf("10. apl_exec 's ← \"hello\"': err=0x%X\n", err);
    v = get_var_value("s", LOC);
    if (v) {
        int is_str = is_string(v);
        int r = get_rank(v);
        uint64_t c = get_element_count(v);
        printf("11. s: is_string=%d, rank=%d, count=%lu\n", is_str, r, (unsigned long)c);
        if (!is_str) { printf("    FAIL: expected is_string=1\n"); failures++; }
        if (r != 1) { printf("    FAIL: expected rank=1\n"); failures++; }
        if (c != 5) { printf("    FAIL: expected count=5\n"); failures++; }
        /* Check first char */
        int ch = get_char(v, 0);
        printf("12. s[0] = '%c' (expect 'h')\n", ch);
        if (ch != 'h') { printf("    FAIL: expected 'h'\n"); failures++; }
        release_value(v, LOC);
    }

    /* Test float */
    err = apl_exec("f ← 3.14");
    printf("13. apl_exec 'f ← 3.14': err=0x%X\n", err);
    v = get_var_value("f", LOC);
    if (v) {
        APL_Float fval = get_real(v, 0);
        int type2 = get_type(v, 0);
        printf("14. f: real=%g, type=0x%X (expect 0x20)\n", fval, type2);
        if (type2 != 0x20) { printf("    FAIL: expected CCT_FLOAT (0x20)\n"); failures++; }
        release_value(v, LOC);
    }

    /* Test int_scalar and set_int */
    APL_value is = int_scalar(99, LOC);
    printf("15. int_scalar(99): [0]=%ld\n", (long)get_int(is, 0));
    if (get_int(is, 0) != 99) { printf("    FAIL: expected 99\n"); failures++; }
    set_int(77, is, 0);
    printf("16. set_int(77): [0]=%ld\n", (long)get_int(is, 0));
    if (get_int(is, 0) != 77) { printf("    FAIL: expected 77\n"); failures++; }
    release_value(is, LOC);

    /* Test char_scalar */
    APL_value cs = char_scalar('A', LOC);
    printf("17. char_scalar('A'): [0]=%c, type=0x%X\n", get_char(cs, 0), get_type(cs, 0));
    if (get_char(cs, 0) != 'A') { printf("    FAIL: expected 'A'\n"); failures++; }
    if (get_type(cs, 0) != 0x02) { printf("    FAIL: expected CCT_CHAR (0x02)\n"); failures++; }
    release_value(cs, LOC);

    /* Test double_scalar */
    APL_value ds = double_scalar(2.718, LOC);
    printf("18. double_scalar(2.718): [0]=%g\n", get_real(ds, 0));
    release_value(ds, LOC);

    /* Test apl_value (rank 2, 3×2) */
    int64_t shape[] = {3, 2};
    APL_value av = apl_value(2, shape, LOC);
    printf("19. apl_value(2, {3,2}): rank=%d, count=%lu\n",
           get_rank(av), (unsigned long)get_element_count(av));
    if (get_rank(av) != 2) { printf("    FAIL: expected rank=2\n"); failures++; }
    if (get_element_count(av) != 6) { printf("    FAIL: expected count=6\n"); failures++; }
    release_value(av, LOC);

    /* Test char_vector */
    APL_value cv = char_vector("world", LOC);
    printf("20. char_vector('world'): is_string=%d, count=%lu\n",
           is_string(cv), (unsigned long)get_element_count(cv));
    if (!is_string(cv)) { printf("    FAIL: expected is_string=1\n"); failures++; }
    if (get_element_count(cv) != 5) { printf("    FAIL: expected count=5\n"); failures++; }
    release_value(cv, LOC);

    /* Test set_var_value */
    APL_value sv = int_scalar(123, LOC);
    set_var_value("z", sv, LOC);
    release_value(sv, LOC);
    v = get_var_value("z", LOC);
    if (v) {
        printf("21. set_var_value('z', 123): z=%ld\n", (long)get_int(v, 0));
        if (get_int(v, 0) != 123) { printf("    FAIL: expected 123\n"); failures++; }
        release_value(v, LOC);
    }

    /* Test print_value_to_string */
    APL_value ps = int_scalar(42, LOC);
    char * ps_str = print_value_to_string(ps);
    printf("22. print_value_to_string(42): '%s'\n", ps_str);
    free(ps_str);
    release_value(ps, LOC);

    /* Test get_owner_count */
    APL_value oc = int_scalar(1, LOC);
    int owners = get_owner_count(oc);
    printf("23. get_owner_count: %d (expect 1)\n", owners);
    if (owners != 1) { printf("    FAIL: expected 1\n"); failures++; }
    release_value(oc, LOC);

    if (failures == 0)
        printf("\n=== All tests passed ===\n");
    else
        printf("\n=== %d FAILURES ===\n", failures);

    return failures;
}
