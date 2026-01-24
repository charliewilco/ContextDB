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

ContextDBHandle *contextdb_open(const char *path);
void contextdb_close(ContextDBHandle *handle);

bool contextdb_insert(ContextDBHandle *handle,
                      const char *expression,
                      const float *meaning_ptr,
                      size_t meaning_len);

bool contextdb_count(const ContextDBHandle *handle, size_t *out_count);

ContextDBQueryResult *contextdb_query_meaning(const ContextDBHandle *handle,
                                             const float *meaning_ptr,
                                             size_t meaning_len,
                                             float threshold,
                                             size_t limit,
                                             size_t *out_len);

ContextDBQueryResult *contextdb_query_expression_contains(const ContextDBHandle *handle,
                                                         const char *expression,
                                                         size_t limit,
                                                         size_t *out_len);

void contextdb_query_results_free(ContextDBQueryResult *results, size_t len);

char *contextdb_last_error_message(void);
void contextdb_string_free(char *ptr);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // CONTEXTDB_H
