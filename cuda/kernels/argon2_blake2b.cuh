#ifndef QMS_ARGON2_CUH
#define QMS_ARGON2_CUH

#include <cuda_runtime.h>
#include <stdint.h>
#include "common.cuh"

__device__ __constant__ static const uint64_t qms_iv[8] = {
    0x6a09e667f3bcc908ULL, 0xbb67ae8584caa73bULL, 0x3c6ef372fe94f82bULL,
    0xa54ff53a5f1d36f1ULL, 0x510e527fade682d1ULL, 0x9b05688c2b3e6c1fULL,
    0x1f83d9abfb41bd6bULL, 0x5be0cd19137e2179ULL
};

__device__ __constant__ static const uint8_t qms_sigma[12][16] = {
    { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15 },
    { 14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3 },
    { 11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4 },
    { 7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8 },
    { 9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13 },
    { 2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9 },
    { 12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11 },
    { 13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10 },
    { 6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5 },
    { 10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0 },
    { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15 },
    { 14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3 }
};

__device__ __forceinline__ uint64_t qms_rotr32(uint64_t value) {
    return (value >> 32) | (value << 32);
}

__device__ __forceinline__ void qms_blake2b_g(
    uint64_t *v, int a, int b, int c, int d, uint64_t x, uint64_t y)
{
    v[a] = v[a] + v[b] + x;
    v[d] = qms_rotr64(v[d] ^ v[a], 32);
    v[c] = v[c] + v[d];
    v[b] = qms_rotr64(v[b] ^ v[c], 24);
    v[a] = v[a] + v[b] + y;
    v[d] = qms_rotr64(v[d] ^ v[a], 16);
    v[c] = v[c] + v[d];
    v[b] = qms_rotr64(v[b] ^ v[c], 63);
}

__device__ __forceinline__ void qms_blake2b_compress(
    uint64_t *h, const uint64_t *m, uint64_t counter_lo, uint64_t counter_hi, int last)
{
    uint64_t v[16];
    uint64_t m_sched[16];

#pragma unroll
    for (int i = 0; i < 8; i++) {
        v[i] = h[i];
        v[i + 8] = qms_iv[i];
    }

    v[12] ^= counter_lo;
    v[13] ^= counter_hi;
    if (last) {
        v[14] = ~v[14];
    }

#pragma unroll
    for (int round = 0; round < 12; round++) {
#pragma unroll
        for (int i = 0; i < 16; i++) {
            m_sched[i] = m[qms_sigma[round][i]];
        }
        qms_blake2b_g(v, 0, 4, 8, 12, m_sched[0], m_sched[1]);
        qms_blake2b_g(v, 1, 5, 9, 13, m_sched[2], m_sched[3]);
        qms_blake2b_g(v, 2, 6, 10, 14, m_sched[4], m_sched[5]);
        qms_blake2b_g(v, 3, 7, 11, 15, m_sched[6], m_sched[7]);
        qms_blake2b_g(v, 0, 5, 10, 15, m_sched[8], m_sched[9]);
        qms_blake2b_g(v, 1, 6, 11, 12, m_sched[10], m_sched[11]);
        qms_blake2b_g(v, 2, 7, 8, 13, m_sched[12], m_sched[13]);
        qms_blake2b_g(v, 3, 4, 9, 14, m_sched[14], m_sched[15]);
    }

#pragma unroll
    for (int i = 0; i < 8; i++) {
        h[i] ^= v[i] ^ v[i + 8];
    }
}

__device__ static inline void qms_blake2b_init(uint64_t *h, uint32_t out_len)
{
    uint64_t param = 0x01010000ULL ^ (uint64_t)out_len;
#pragma unroll
    for (int i = 0; i < 8; i++) {
        h[i] = qms_iv[i];
    }
    h[0] ^= param;
}

__device__ static inline void qms_blake2b_update(
    uint64_t *h, const uint8_t *input, uint32_t in_len,
    uint64_t *counter_lo, uint64_t *counter_hi)
{
    uint64_t m[16];
    uint32_t offset = 0;

    while (in_len > 0) {
        uint32_t take = in_len > QMS_B2B_BLOCK_BYTES ? QMS_B2B_BLOCK_BYTES : in_len;
        uint8_t padded[QMS_B2B_BLOCK_BYTES];

#pragma unroll
        for (int i = 0; i < QMS_B2B_BLOCK_WORDS; i++) {
            padded[i * 8 + 0] = 0;
            padded[i * 8 + 1] = 0;
            padded[i * 8 + 2] = 0;
            padded[i * 8 + 3] = 0;
            padded[i * 8 + 4] = 0;
            padded[i * 8 + 5] = 0;
            padded[i * 8 + 6] = 0;
            padded[i * 8 + 7] = 0;
        }
        for (uint32_t i = 0; i < take; i++) {
            padded[i] = input[offset + i];
        }
#pragma unroll
        for (int i = 0; i < 16; i++) {
            m[i] = qms_load64(padded + i * 8);
        }

        *counter_lo += take;
        int is_last = (take == in_len) ? 1 : 0;
        if (take == QMS_BLOCK_BYTES && in_len == QMS_BLOCK_BYTES) {
            is_last = 1;
        }
        qms_blake2b_compress(h, m, *counter_lo, *counter_hi, is_last);

        offset += take;
        in_len -= take;
    }
}

#endif
