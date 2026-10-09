#ifndef QMS_TYPES_H
#define QMS_TYPES_H

#include <stdint.h>

#define QMS_MAX_DEVICES 16
#define QMS_DEVICE_NAME_MAX 256

typedef struct qms_device_info {
    int32_t index;
    int32_t compute_major;
    int32_t compute_minor;
    int32_t multiprocessor_count;
    uint64_t global_memory_bytes;
    char name[QMS_DEVICE_NAME_MAX];
} qms_device_info;

typedef struct qms_launch_params {
    int32_t time_cost;
    int32_t mem_cost;
    int32_t parallelism;
    int32_t version;
    int32_t key_length;
} qms_launch_params;

typedef struct qms_scan_request {
    const uint8_t *preimage;
    int32_t preimage_len;
    uint64_t start_nonce;
    uint64_t count;
    const uint8_t *mask;
    int32_t mask_len;
    uint64_t *out_nonces;
    int32_t out_capacity;
    qms_launch_params params;
} qms_scan_request;

typedef struct qms_scan_result {
    int32_t found;
    int32_t launched;
    int32_t threads_per_hash;
} qms_scan_result;

#endif
