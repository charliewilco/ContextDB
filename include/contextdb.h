#ifndef CONTEXTDB_H
#define CONTEXTDB_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ContextDBHandle ContextDBHandle;

typedef struct ContextDBQueryResult {
    uint8_t id[16];
    float score;
    char *expression;
} ContextDBQueryResult;

enum {
    CONTEXTDB_STATUS_OK = 0,
    CONTEXTDB_STATUS_INVALID_ARGUMENT = 1,
    CONTEXTDB_STATUS_NOT_FOUND = 2,
    CONTEXTDB_STATUS_DATABASE = 3,
    CONTEXTDB_STATUS_PANIC = 4,
};

uint32_t contextdb_abi_version(void);
int32_t contextdb_last_error_code(void);

ContextDBHandle *contextdb_open(const char *path);
void contextdb_close(ContextDBHandle *handle);

bool contextdb_insert(ContextDBHandle *handle,
                      const char *expression,
                      const float *meaning_ptr,
                      size_t meaning_len);

bool contextdb_count(const ContextDBHandle *handle, size_t *out_count);

// JSON APIs return a ContextDB status code. Returned strings are owned by the
// caller and must be released with contextdb_string_free.
int32_t contextdb_insert_json(ContextDBHandle *handle,
                              const char *json,
                              char **out_id);
int32_t contextdb_get_json(const ContextDBHandle *handle,
                           const char *id,
                           char **out_json);
int32_t contextdb_update_json(ContextDBHandle *handle, const char *json);
int32_t contextdb_delete_id(ContextDBHandle *handle, const char *id);
int32_t contextdb_query_json(const ContextDBHandle *handle,
                             const char *json,
                             char **out_json);

// Returns a newly allocated results array owned by the caller.
// The length is written to out_len (must be non-NULL). Free with
// contextdb_query_results_free(results, *out_len). Each result's
// expression string is also owned by the array and freed there.
// Callers must pass valid handle/pointers and matching lengths.
ContextDBQueryResult *contextdb_query_meaning(const ContextDBHandle *handle,
                                             const float *meaning_ptr,
                                             size_t meaning_len,
                                             float threshold,
                                             size_t limit,
                                             size_t *out_len);

// Returns a newly allocated results array owned by the caller.
// The length is written to out_len (must be non-NULL). Free with
// contextdb_query_results_free(results, *out_len). Each result's
// expression string is also owned by the array and freed there.
// Callers must pass valid handle/pointers and matching lengths.
ContextDBQueryResult *contextdb_query_expression_contains(const ContextDBHandle *handle,
                                                         const char *expression,
                                                         size_t limit,
                                                         size_t *out_len);

// Frees a results array (and any associated result strings) returned by
// contextdb_query_*. len must match the out_len returned by the query.
void contextdb_query_results_free(ContextDBQueryResult *results, size_t len);

// Returns a newly allocated, null-terminated error message string owned
// by the caller. Free with contextdb_string_free.
char *contextdb_last_error_message(void);
// Frees a string returned by contextdb_last_error_message (or other FFI
// string-returning APIs). Pointer must be valid or NULL.
void contextdb_string_free(char *ptr);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // CONTEXTDB_H
