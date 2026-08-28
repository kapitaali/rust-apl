/* 
 * Rust APL 2.0 — libapl C API header
 * Based on GNU APL libapl.h (compatible subset)
 */

#ifndef __LIBAPL_H_DEFINED__
#define __LIBAPL_H_DEFINED__

#include <stdio.h>
#include <stdint.h>

/* APL_Float is double in our implementation */
typedef double APL_Float;

/* Cell types */
enum C_CellType
{
   CCT_CHAR    = 0x02,
   CCT_POINTER = 0x04,
   CCT_INT     = 0x10,
   CCT_FLOAT   = 0x20,
   CCT_COMPLEX = 0x40,
   CCT_NUMERIC = CCT_INT | CCT_FLOAT | CCT_COMPLEX,
};

/* Error codes (subset) */
enum LIBAPL_error
{
    LAE_NO_ERROR           = 0,
    LAE_DOMAIN_ERROR       = 0x010001,
    LAE_INDEX_ERROR        = 0x010002,
    LAE_LENGTH_ERROR       = 0x010003,
    LAE_RANK_ERROR         = 0x010004,
    LAE_SYNTAX_ERROR       = 0x010005,
    LAE_VALUE_ERROR        = 0x010006,
    LAE_NOT_IMPLEMENTED    = 0x010007,
    LAE_VARIABLE_NOT_ASSIGNED = 0x010008,
    LAE_OUT_BUFFER_OVERFLOW   = 0x010009,
    LAE_IN_BUFFER_OVERFLOW    = 0x01000A,
};
typedef enum LIBAPL_error LIBAPL_error;

#ifdef __cplusplus
extern "C" {
#endif

/* APL opaque types */
typedef struct APLValue * APL_value;
typedef struct APLFunction const * APL_function;

/* Helper macros */
#define STR(x) #x
#define LOC Loc(__FILE__, __LINE__)
#define Loc(f, l) f ":" STR(l)

/*═══════════════════════════════════════════════════════════════
 * 1. Initialization
 *═══════════════════════════════════════════════════════════════*/

extern void init_libapl(const char * progname, int log_startup);
extern int expand_LF_to_CRLF(int on);
extern void disable_safe_mode(void);

/*═══════════════════════════════════════════════════════════════
 * 2. Execution
 *═══════════════════════════════════════════════════════════════*/

extern LIBAPL_error apl_exec(const char * line_utf8);
extern const char * apl_command(const char * command_utf8);

extern long repl(char * input_buffer, int * input_bufsize,
                 char * output_buffer, int * output_bufsize,
                 LIBAPL_error * error);

extern LIBAPL_error fix_function(const char ** function_lines_utf8);
extern LIBAPL_error fix_function_NL(const char * function_lines_utf8);

extern void print_ucs(FILE * out, const unsigned int * string_ucs);

/*═══════════════════════════════════════════════════════════════
 * 3. Value constructors
 *═══════════════════════════════════════════════════════════════*/

extern APL_value get_var_value(const char * var_name_utf8, const char * loc);
extern APL_value int_scalar(int64_t val, const char * loc);
extern APL_value double_scalar(APL_Float val, const char * loc);
extern APL_value complex_scalar(APL_Float real, APL_Float imag, const char * loc);
extern APL_value char_scalar(int unicode, const char * loc);
extern APL_value apl_value(int rank, const int64_t * shape, const char * loc);
extern APL_value char_vector(const char * str, const char * loc);

static inline APL_value apl_scalar(const char * loc)
   { return apl_value(0, 0, loc); }
static inline APL_value apl_vector(int64_t len, const char * loc)
   { return apl_value(1, &len, loc); }
static inline APL_value apl_matrix(int64_t rows, int64_t cols, const char * loc)
   { const int64_t sh[] = { rows, cols }; return apl_value(2, sh, loc); }

/*═══════════════════════════════════════════════════════════════
 * 4. Value destructor
 *═══════════════════════════════════════════════════════════════*/

extern void release_value(APL_value val, const char * loc);

/*═══════════════════════════════════════════════════════════════
 * 5. Read access
 *═══════════════════════════════════════════════════════════════*/

extern int get_rank(const APL_value val);
extern int64_t get_axis(const APL_value val, unsigned int axis);
extern uint64_t get_element_count(const APL_value val);
extern int get_type(const APL_value val, uint64_t idx);
extern int get_char(const APL_value val, uint64_t idx);
extern int64_t get_int(const APL_value val, uint64_t idx);
extern APL_Float get_real(const APL_value val, uint64_t idx);
extern APL_Float get_imag(const APL_value val, uint64_t idx);
extern APL_value get_value(const APL_value val, uint64_t idx);
extern int is_string(APL_value val);

static inline int is_char(const APL_value val, uint64_t idx)
   { return get_type(val, idx) == CCT_CHAR; }
static inline int is_int(const APL_value val, uint64_t idx)
   { return get_type(val, idx) == CCT_INT; }
static inline int is_double(const APL_value val, uint64_t idx)
   { return get_type(val, idx) == CCT_FLOAT; }
static inline int is_complex(const APL_value val, uint64_t idx)
   { return get_type(val, idx) == CCT_COMPLEX; }
static inline int is_numeric(const APL_value val, uint64_t idx)
   { return get_type(val, idx) & CCT_NUMERIC; }
static inline int is_value(const APL_value val, uint64_t idx)
   { return get_type(val, idx) == CCT_POINTER; }

/*═══════════════════════════════════════════════════════════════
 * 6. Write access
 *═══════════════════════════════════════════════════════════════*/

extern int set_var_value(const char * var_name_utf8, const APL_value new_value,
                  const char * loc);
extern void set_char(int unicode, APL_value val, uint64_t idx);
extern void set_int(int64_t new_int, APL_value val, uint64_t idx);
extern void set_double(APL_Float new_real, APL_value val, uint64_t idx);
extern void set_complex(APL_Float new_real, APL_Float new_imag, APL_value val, uint64_t idx);
extern void set_value(const APL_value new_value, APL_value val, uint64_t idx);

/*═══════════════════════════════════════════════════════════════
 * 7. Printing
 *═══════════════════════════════════════════════════════════════*/

extern void print_value(const APL_value value, FILE * out);
extern char * print_value_to_string(const APL_value value);

/*═══════════════════════════════════════════════════════════════
 * 8. UTF conversion
 *═══════════════════════════════════════════════════════════════*/

extern int UTF8_to_Unicode(const char * utf, int * length);
extern void Unicode_to_UTF8(int unicode, char * dest, int * length);

/*═══════════════════════════════════════════════════════════════
 * 9. Callbacks
 *═══════════════════════════════════════════════════════════════*/

typedef int (*result_callback)(const APL_value result, int committed);
typedef const char * (*get_line_from_user_cb)(int mode, const char * prompt);

extern result_callback install_result_callback(result_callback new_callback);
extern get_line_from_user_cb
 install_get_line_from_user_cb(get_line_from_user_cb new_callback);

/*═══════════════════════════════════════════════════════════════
 * 10. Evaluation functions (stubs)
 *═══════════════════════════════════════════════════════════════*/

extern APL_value eval__fun(APL_function fun);
extern APL_value eval__fun_B(APL_function fun, APL_value B);
extern APL_value eval__A_fun_B(APL_value A, APL_function fun, APL_value B);

/*═══════════════════════════════════════════════════════════════
 * 11. Utilities
 *═══════════════════════════════════════════════════════════════*/

extern int get_owner_count(APL_value val);
extern APL_function get_function_ucs(const unsigned int * name,
                                     APL_function * L, APL_function * R);

#ifdef __cplusplus
}
#endif

#endif /* __LIBAPL_H_DEFINED__ */
