#ifndef QMS_CUDA_H
#define QMS_CUDA_H

#include <stdint.h>
#include "qms_types.h"
#include "qms_errors.h"

#if defined(_WIN32)
#define QMS_API __declspec(dllexport)
#else
#define QMS_API __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

QMS_API int32_t qms_cuda_version(void);
QMS_API const char *qms_cuda_error_message(int32_t code);

QMS_API int32_t qms_cuda_device_count(void);
QMS_API int32_t qms_cuda_device_name(int32_t index, char *buffer, int32_t buffer_len);
QMS_API int32_t qms_cuda_device_info(int32_t index, qms_device_info *info);
QMS_API int32_t qms_cuda_select(int32_t index);
QMS_API int32_t qms_cuda_selected(void);
QMS_API int32_t qms_cuda_shutdown(void);

QMS_API int32_t qms_cuda_argon2d(
    const uint8_t *preimage,
    int32_t preimage_len,
    uint64_t nonce,
    uint8_t *hash_out,
    int32_t hash_len,
    int32_t time_cost,
    int32_t mem_cost,
    int32_t parallelism,
    int32_t version,
    int32_t key_length);

QMS_API int32_t qms_cuda_scan(
    const uint8_t *preimage,
    int32_t preimage_len,
    uint64_t start_nonce,
    uint64_t count,
    const uint8_t *mask,
    int32_t mask_len,
    uint64_t *out_nonces,
    int32_t out_capacity,
    int32_t time_cost,
    int32_t mem_cost,
    int32_t parallelism,
    int32_t version,
    int32_t key_length);

QMS_API int32_t qms_cuda_selftest(void);

#ifdef __cplusplus
}
#endif

#endif
