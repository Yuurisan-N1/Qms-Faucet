#ifndef QMS_COMMON_CUH
#define QMS_COMMON_CUH

#include <cuda_runtime.h>
#include <stdint.h>

#define QMS_BLOCK_BYTES 1024
#define QMS_BLOCK_WORDS 128

#define QMS_B2B_BLOCK_BYTES 128
#define QMS_B2B_BLOCK_WORDS 16
#define QMS_ROT32 32
#define QMS_ROT24 24
#define QMS_ROT16 16
#define QMS_ROT63 63

__device__ __forceinline__ uint64_t qms_rotr64(uint64_t value, uint32_t shift) {
    return (value >> shift) | (value << (64 - shift));
}

__device__ __forceinline__ uint64_t qms_load64(const uint8_t *source) {
    uint64_t value;
    value = (uint64_t)source[0];
    value |= (uint64_t)source[1] << 8;
    value |= (uint64_t)source[2] << 16;
    value |= (uint64_t)source[3] << 24;
    value |= (uint64_t)source[4] << 32;
    value |= (uint64_t)source[5] << 40;
    value |= (uint64_t)source[6] << 48;
    value |= (uint64_t)source[7] << 56;
    return value;
}

__device__ __forceinline__ void qms_store64(uint8_t *target, uint64_t value) {
    target[0] = (uint8_t)(value & 0xff);
    target[1] = (uint8_t)((value >> 8) & 0xff);
    target[2] = (uint8_t)((value >> 16) & 0xff);
    target[3] = (uint8_t)((value >> 24) & 0xff);
    target[4] = (uint8_t)((value >> 32) & 0xff);
    target[5] = (uint8_t)((value >> 40) & 0xff);
    target[6] = (uint8_t)((value >> 48) & 0xff);
    target[7] = (uint8_t)((value >> 56) & 0xff);
}

__device__ __forceinline__ void qms_store32(uint8_t *target, uint32_t value) {
    target[0] = (uint8_t)(value & 0xff);
    target[1] = (uint8_t)((value >> 8) & 0xff);
    target[2] = (uint8_t)((value >> 16) & 0xff);
    target[3] = (uint8_t)((value >> 24) & 0xff);
}

__device__ __forceinline__ uint64_t qms_low32(uint64_t value) {
    return value & 0xffffffffULL;
}

__device__ __forceinline__ uint64_t qms_mul32(uint64_t left, uint64_t right) {
    return 2ULL * qms_low32(left) * qms_low32(right);
}

#endif
