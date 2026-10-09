
#include <cuda_runtime.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "common.cuh"
#include "argon2_blake2b.cuh"
#include "../include/qms_cuda.h"
#include "../include/qms_types.h"
#include "../include/qms_errors.h"

#define QMS_MAX_LANES 4
#define QMS_MAX_SEGMENTS 4
#define QMS_MAX_MEM_BLOCKS 8192
#define QMS_BLOCK_U64 128
#define QMS_PREHASH_BYTES 80
#define QMS_MAX_THREADS 1024
#define QMS_THREADS_PER_BLOCK 64
#define QMS_MAX_SLOTS 65536
#define QMS_MAX_SCRATCH_SLOTS 65536
#define QMS_SCRATCH_BUDGET_PCT 90

__device__ __forceinline__ uint64_t qms_blamka(uint64_t left, uint64_t right)
{
    return left + right + qms_mul32(left, right);
}

__device__ __forceinline__ void qms_g(
    uint64_t *v, int a, int b, int c, int d)
{
    v[a] = qms_blamka(v[a], v[b]);
    v[d] = qms_rotr64(v[d] ^ v[a], 32);
    v[c] = qms_blamka(v[c], v[d]);
    v[b] = qms_rotr64(v[b] ^ v[c], 24);
    v[a] = qms_blamka(v[a], v[b]);
    v[d] = qms_rotr64(v[d] ^ v[a], 16);
    v[c] = qms_blamka(v[c], v[d]);
    v[b] = qms_rotr64(v[b] ^ v[c], 63);
}

#define QMS_ROUND_NOMSG(v, i0, i1, i2, i3, i4, i5, i6, i7, i8, i9, i10, i11, i12, i13, i14, i15) \
    qms_g(v, i0, i4, i8, i12);  \
    qms_g(v, i1, i5, i9, i13);  \
    qms_g(v, i2, i6, i10, i14); \
    qms_g(v, i3, i7, i11, i15); \
    qms_g(v, i0, i5, i10, i15); \
    qms_g(v, i1, i6, i11, i12); \
    qms_g(v, i2, i7, i8, i13);  \
    qms_g(v, i3, i4, i9, i14)

__device__ __forceinline__ void qms_fill_block(
    uint64_t *prev, uint64_t *ref, uint64_t *next, int with_xor)
{
    uint64_t block_r[QMS_BLOCK_U64];

#pragma unroll
    for (int i = 0; i < QMS_BLOCK_U64; i++) {
        block_r[i] = prev[i] ^ ref[i];
    }

#pragma unroll 1
    for (int i = 0; i < 8; i++) {
        int o = i * 16;
        QMS_ROUND_NOMSG(block_r,
            o + 0, o + 1, o + 2, o + 3, o + 4, o + 5, o + 6, o + 7,
            o + 8, o + 9, o + 10, o + 11, o + 12, o + 13, o + 14, o + 15);
    }

#pragma unroll 1
    for (int i = 0; i < 8; i++) {
        int o = 2 * i;
        QMS_ROUND_NOMSG(block_r,
            o + 0, o + 1, o + 16, o + 17, o + 32, o + 33, o + 48, o + 49,
            o + 64, o + 65, o + 80, o + 81, o + 96, o + 97, o + 112, o + 113);
    }

#pragma unroll
    for (int i = 0; i < QMS_BLOCK_U64; i++) {
        uint64_t init = prev[i] ^ ref[i];
        next[i] = (with_xor ? (init ^ next[i]) : init) ^ block_r[i];
    }
}

__device__ __forceinline__ uint32_t qms_index_alpha(
    uint32_t pass, uint32_t slice, uint32_t index, uint32_t pseudo_rand,
    int same_lane, uint32_t segment_length, uint32_t lane_length)
{
    uint32_t reference_area_size;
    uint64_t relative_position;
    uint32_t start_position;
    uint32_t absolute_position;

    if (pass == 0) {
        if (slice == 0) {
            reference_area_size = index - 1;
        } else if (same_lane) {
            reference_area_size = slice * segment_length + index - 1;
        } else {
            reference_area_size = slice * segment_length + (index == 0 ? 0xffffffffu : 0u);
        }
    } else if (same_lane) {
        reference_area_size = lane_length - segment_length + index - 1;
    } else {
        reference_area_size = lane_length - segment_length + (index == 0 ? 0xffffffffu : 0u);
    }

    relative_position = pseudo_rand;
    relative_position = (relative_position * relative_position) >> 32;
    relative_position = reference_area_size - 1 -
        ((uint64_t)reference_area_size * relative_position >> 32);

    if (pass == 0) {
        start_position = 0u;
    } else {
        start_position = (slice == QMS_MAX_SEGMENTS - 1) ? 0u : ((slice + 1) * segment_length);
    }

    absolute_position = (uint32_t)((start_position + relative_position) % lane_length);
    return absolute_position;
}

__device__ void qms_fill_segment(
    uint64_t *memory, uint32_t pass, uint32_t lane, uint32_t slice,
    uint32_t lanes, uint32_t segment_length, uint32_t lane_length,
    uint32_t version)
{
    uint32_t starting_index = 0;
    if (pass == 0 && slice == 0) {
        starting_index = 2;
    }

    uint32_t curr_offset = lane * lane_length + slice * segment_length + starting_index;
    uint32_t prev_offset;
    if (curr_offset % lane_length == 0) {
        prev_offset = curr_offset + lane_length - 1;
    } else {
        prev_offset = curr_offset - 1;
    }

    for (uint32_t i = starting_index; i < segment_length; i++, curr_offset++, prev_offset++) {
        if (curr_offset % lane_length == 1) {
            prev_offset = curr_offset - 1;
        }

        uint64_t pseudo_rand = memory[(size_t)prev_offset * QMS_BLOCK_U64];
        uint32_t ref_lane = (uint32_t)((pseudo_rand >> 32) % lanes);
        if (pass == 0 && slice == 0) {
            ref_lane = lane;
        }
        uint32_t ref_index = qms_index_alpha(
            pass, slice, i, (uint32_t)(pseudo_rand & 0xffffffffULL),
            ref_lane == lane ? 1 : 0, segment_length, lane_length);

        uint64_t *ref_block = memory + ((size_t)lane_length * ref_lane + ref_index) * QMS_BLOCK_U64;
        uint64_t *curr_block = memory + (size_t)curr_offset * QMS_BLOCK_U64;
        uint64_t *prev_block = memory + (size_t)prev_offset * QMS_BLOCK_U64;

        int with_xor = (version == 16 || pass == 0) ? 0 : 1;
        qms_fill_block(prev_block, ref_block, curr_block, with_xor);
    }
}

__device__ void qms_long_hash(const uint8_t *input, uint32_t in_len, uint8_t *out, uint32_t out_len)
{
    uint8_t prefix[4];
    qms_store32(prefix, out_len);

    uint8_t message[QMS_BLOCK_BYTES + 4];
    if (in_len > QMS_BLOCK_BYTES) {
        in_len = QMS_BLOCK_BYTES;
    }
    for (uint32_t i = 0; i < 4; i++) {
        message[i] = prefix[i];
    }
    for (uint32_t i = 0; i < in_len; i++) {
        message[4 + i] = input[i];
    }

    if (out_len <= 64) {
        uint64_t h[8];
        uint64_t lo = 0, hi = 0;
        uint8_t digest[64];
        qms_blake2b_init(h, out_len);
        qms_blake2b_update(h, message, in_len + 4, &lo, &hi);
#pragma unroll
        for (int i = 0; i < 8; i++) {
            qms_store64(digest + i * 8, h[i]);
        }
        for (uint32_t i = 0; i < out_len; i++) {
            out[i] = digest[i];
        }
        return;
    }

    uint32_t rounds = (out_len + 31) / 32 - 2;
    uint8_t buffer[64];
    uint32_t written = 0;

    for (uint32_t r = 0; r < rounds; r++) {
        uint64_t h[8];
        uint64_t lo = 0, hi = 0;
        qms_blake2b_init(h, 64);
        if (r == 0) {
            qms_blake2b_update(h, message, in_len + 4, &lo, &hi);
        } else {
            qms_blake2b_update(h, buffer, 64, &lo, &hi);
        }
#pragma unroll
        for (int i = 0; i < 8; i++) {
            qms_store64(buffer + i * 8, h[i]);
        }
        for (uint32_t i = 0; i < 32; i++) {
            out[written + i] = buffer[i];
        }
        written += 32;
    }

    uint32_t remaining = out_len - written;
    uint64_t h[8];
    uint64_t lo = 0, hi = 0;
    qms_blake2b_init(h, remaining);
    qms_blake2b_update(h, buffer, 64, &lo, &hi);
#pragma unroll
    for (int i = 0; i < 8; i++) {
        qms_store64(buffer + i * 8, h[i]);
    }
    for (uint32_t i = 0; i < remaining; i++) {
        out[written + i] = buffer[i];
    }
}

__device__ void qms_argon2d_hash(
    const uint8_t *preimage, uint32_t preimage_len,
    uint64_t nonce, uint32_t time_cost, uint32_t mem_cost,
    uint32_t parallelism, uint32_t version, uint32_t key_length,
    uint64_t *memory, uint8_t *out)
{
    uint8_t password[8];
    for (uint32_t i = 0; i < 8; i++) {
        password[i] = (uint8_t)((nonce >> (8 * (7 - i))) & 0xff);
    }

    uint32_t lanes = parallelism == 0 ? 1 : parallelism;
    if (lanes > QMS_MAX_LANES) {
        lanes = QMS_MAX_LANES;
    }
    uint32_t memory_blocks = mem_cost;
    if (memory_blocks < 8 * lanes) {
        memory_blocks = 8 * lanes;
    }
    uint32_t segment_length = memory_blocks / (lanes * QMS_MAX_SEGMENTS);
    memory_blocks = segment_length * lanes * QMS_MAX_SEGMENTS;
    uint32_t lane_length = segment_length * QMS_MAX_SEGMENTS;

    uint8_t prehash[QMS_PREHASH_BYTES];
    uint32_t at = 0;
    qms_store32(prehash + at, lanes); at += 4;
    qms_store32(prehash + at, key_length); at += 4;
    qms_store32(prehash + at, mem_cost); at += 4;
    qms_store32(prehash + at, time_cost); at += 4;
    qms_store32(prehash + at, version); at += 4;
    qms_store32(prehash + at, 0); at += 4;
    qms_store32(prehash + at, 8); at += 4;
    for (uint32_t i = 0; i < 8; i++) { prehash[at + i] = password[i]; }
    at += 8;
    qms_store32(prehash + at, preimage_len); at += 4;
    for (uint32_t i = 0; i < preimage_len; i++) { prehash[at + i] = preimage[i]; }
    at += preimage_len;
    qms_store32(prehash + at, 0); at += 4;
    qms_store32(prehash + at, 0); at += 4;

    uint8_t h0[64];
    {
        uint64_t h[8];
        uint64_t lo = 0, hi = 0;
        qms_blake2b_init(h, 64);
        qms_blake2b_update(h, prehash, at, &lo, &hi);
#pragma unroll
        for (int i = 0; i < 8; i++) {
            qms_store64(h0 + i * 8, h[i]);
        }
    }

    uint8_t seed[64 + 8];
    for (uint32_t i = 0; i < 64; i++) { seed[i] = h0[i]; }

    uint8_t block_bytes[QMS_BLOCK_BYTES];
    for (uint32_t lane = 0; lane < lanes; lane++) {
        for (uint32_t index = 0; index < 2; index++) {
            qms_store32(seed + 64, index);
            qms_store32(seed + 68, lane);
            qms_long_hash(seed, 72, block_bytes, QMS_BLOCK_BYTES);
            uint64_t *target = memory + ((size_t)lane * lane_length + index) * QMS_BLOCK_U64;
#pragma unroll
            for (int i = 0; i < QMS_BLOCK_U64; i++) {
                target[i] = qms_load64(block_bytes + i * 8);
            }
        }
    }

    for (uint32_t pass = 0; pass < time_cost; pass++) {
        for (uint32_t slice = 0; slice < QMS_MAX_SEGMENTS; slice++) {
            for (uint32_t lane = 0; lane < lanes; lane++) {
                qms_fill_segment(
                    memory, pass, lane, slice, lanes,
                    segment_length, lane_length, version);
            }
        }
    }

    uint64_t *final_block = memory + ((size_t)lane_length - 1) * QMS_BLOCK_U64;
    for (int i = 0; i < QMS_BLOCK_U64; i++) {
        qms_store64(block_bytes + i * 8, final_block[i]);
    }
    qms_long_hash(block_bytes, QMS_BLOCK_BYTES, out, key_length);
}

__device__ __forceinline__ char qms_hex_digit(uint8_t value)
{
    value &= 0x0f;
    return value < 10 ? (char)('0' + value) : (char)('a' + (value - 10));
}

__device__ int qms_hash_matches(const uint8_t *hash, uint32_t hash_len, const char *mask, uint32_t mask_len)
{
    if (mask_len == 0 || mask_len > hash_len * 2) {
        return 0;
    }
    for (uint32_t i = 0; i < mask_len; i++) {
        uint8_t byte = hash[i / 2];
        char digit = (i % 2 == 0) ? qms_hex_digit((uint8_t)(byte >> 4)) : qms_hex_digit(byte);
        char want = mask[i];
        if (digit < want) {
            return 1;
        }
        if (digit > want) {
            return 0;
        }
    }
    return 1;
}

__global__ __launch_bounds__(QMS_THREADS_PER_BLOCK, 2) void qms_scan_kernel(
    const uint8_t *preimage, int32_t preimage_len,
    uint64_t start_nonce, uint64_t count,
    const char *mask, int32_t mask_len,
    uint64_t *out_nonces, int32_t out_capacity, int32_t *out_found,
    int32_t time_cost, int32_t mem_cost, int32_t parallelism,
    int32_t version, int32_t key_length,
    uint64_t *scratch, int32_t blocks_per_thread)
{
    uint64_t slot = (uint64_t)blockIdx.x * blockDim.x + threadIdx.x;
    if (slot >= count) {
        return;
    }

    uint64_t *memory = scratch + (size_t)slot * (size_t)blocks_per_thread * QMS_BLOCK_U64;

    uint8_t digest[64];
    qms_argon2d_hash(
        preimage, (uint32_t)preimage_len, start_nonce + slot,
        (uint32_t)time_cost, (uint32_t)mem_cost, (uint32_t)parallelism,
        (uint32_t)version, (uint32_t)key_length, memory, digest);

    if (!qms_hash_matches(digest, (uint32_t)key_length, mask, (uint32_t)mask_len)) {
        return;
    }

    int32_t index = atomicAdd(out_found, 1);
    if (index < out_capacity) {
        out_nonces[index] = start_nonce + slot;
    }
}

__global__ void qms_single_kernel(
    const uint8_t *preimage, int32_t preimage_len, uint64_t nonce,
    uint8_t *hash_out, int32_t hash_len,
    int32_t time_cost, int32_t mem_cost, int32_t parallelism,
    int32_t version, int32_t key_length,
    uint64_t *scratch, int32_t blocks_per_thread)
{
    uint64_t *memory = scratch;
    (void)blocks_per_thread;

    uint8_t digest[64];
    qms_argon2d_hash(
        preimage, (uint32_t)preimage_len, nonce,
        (uint32_t)time_cost, (uint32_t)mem_cost, (uint32_t)parallelism,
        (uint32_t)version, (uint32_t)key_length, memory, digest);

    for (int i = 0; i < hash_len; i++) {
        hash_out[i] = digest[i];
    }
}

static int32_t g_selected = -1;
static int g_active_lanes[QMS_MAX_DEVICES] = {0};

struct QmsLaneGuard {
    int slot;
    QmsLaneGuard(int s) : slot(s) { __atomic_add_fetch(&g_active_lanes[slot], 1, __ATOMIC_SEQ_CST); }
    ~QmsLaneGuard() { __atomic_add_fetch(&g_active_lanes[slot], -1, __ATOMIC_SEQ_CST); }
};

static __thread uint64_t *g_scratch = NULL;
static __thread size_t g_scratch_bytes = 0;
static __thread int32_t g_scratch_device = -1;

extern "C" {

const char *qms_error_message(int32_t code)
{
    switch (code) {
    case QMS_OK: return "ok";
    case QMS_ERR_LIBRARY: return "accelerator library could not be loaded";
    case QMS_ERR_DEVICE: return "no usable CUDA device";
    case QMS_ERR_MEMORY: return "device memory could not be allocated";
    case QMS_ERR_LAUNCH: return "kernel launch failed";
    case QMS_ERR_ARGUMENT: return "invalid argument";
    case QMS_ERR_HASH: return "hashing failed";
    case QMS_ERR_UNSUPPORTED: return "unsupported configuration";
    default: return "unknown error";
    }
}

QMS_API int32_t qms_cuda_version(void)
{
    int runtime = 0;
    cudaRuntimeGetVersion(&runtime);
    return runtime;
}

QMS_API const char *qms_cuda_error_message(int32_t code)
{
    return qms_error_message(code);
}

QMS_API int32_t qms_cuda_device_count(void)
{
    int count = 0;
    if (cudaGetDeviceCount(&count) != cudaSuccess) {
        return 0;
    }
    return count;
}

QMS_API int32_t qms_cuda_device_name(int32_t index, char *buffer, int32_t buffer_len)
{
    if (!buffer || buffer_len <= 0) {
        return QMS_ERR_ARGUMENT;
    }
    cudaDeviceProp prop;
    if (cudaGetDeviceProperties(&prop, index) != cudaSuccess) {
        return QMS_ERR_DEVICE;
    }
    int written = snprintf(buffer, (size_t)buffer_len, "%s", prop.name);
    if (written < 0) {
        return QMS_ERR_DEVICE;
    }
    return written > buffer_len - 1 ? buffer_len - 1 : written;
}

QMS_API int32_t qms_cuda_device_info(int32_t index, qms_device_info *info)
{
    if (!info) {
        return QMS_ERR_ARGUMENT;
    }
    cudaDeviceProp prop;
    if (cudaGetDeviceProperties(&prop, index) != cudaSuccess) {
        return QMS_ERR_DEVICE;
    }
    info->index = index;
    info->compute_major = prop.major;
    info->compute_minor = prop.minor;
    info->multiprocessor_count = prop.multiProcessorCount;
    info->global_memory_bytes = (uint64_t)prop.totalGlobalMem;
    snprintf(info->name, QMS_DEVICE_NAME_MAX, "%s", prop.name);
    return QMS_OK;
}

QMS_API int32_t qms_cuda_select(int32_t index)
{
    if (cudaSetDevice(index) != cudaSuccess) {
        return QMS_ERR_DEVICE;
    }
    g_selected = index;
    return QMS_OK;
}

QMS_API int32_t qms_cuda_selected(void)
{
    return g_selected;
}

QMS_API int32_t qms_cuda_shutdown(void)
{
    cudaError_t rc = cudaDeviceReset();
    g_selected = -1;
    return rc == cudaSuccess ? QMS_OK : QMS_ERR_DEVICE;
}

QMS_API int32_t qms_cuda_argon2d(
    const uint8_t *preimage, int32_t preimage_len, uint64_t nonce,
    uint8_t *hash_out, int32_t hash_len,
    int32_t time_cost, int32_t mem_cost, int32_t parallelism,
    int32_t version, int32_t key_length)
{
    if (!preimage || preimage_len <= 0 || !hash_out || hash_len <= 0) {
        return QMS_ERR_ARGUMENT;
    }

    int blocks_per_thread = mem_cost;
    if (blocks_per_thread > QMS_MAX_MEM_BLOCKS) {
        blocks_per_thread = QMS_MAX_MEM_BLOCKS;
    }

    uint8_t *d_preimage = NULL;
    uint8_t *d_hash = NULL;
    uint64_t *d_scratch = NULL;
    if (cudaMalloc(&d_preimage, (size_t)preimage_len) != cudaSuccess) {
        return QMS_ERR_MEMORY;
    }
    if (cudaMalloc(&d_hash, (size_t)hash_len) != cudaSuccess) {
        cudaFree(d_preimage);
        return QMS_ERR_MEMORY;
    }

    int32_t rc = QMS_OK;
    if (cudaMemcpy(d_preimage, preimage, (size_t)preimage_len, cudaMemcpyHostToDevice) != cudaSuccess) {
        rc = QMS_ERR_MEMORY;
        goto done;
    }

    {
        size_t scratch_bytes = (size_t)blocks_per_thread * QMS_BLOCK_U64 * sizeof(uint64_t);
        if (cudaMalloc(&d_scratch, scratch_bytes) != cudaSuccess) {
            rc = QMS_ERR_MEMORY;
            goto done;
        }
        qms_single_kernel<<<1, 1>>>(
            d_preimage, preimage_len, nonce, d_hash, hash_len,
            time_cost, mem_cost, parallelism, version, key_length,
            d_scratch, blocks_per_thread);
        cudaError_t err = cudaDeviceSynchronize();
        if (err != cudaSuccess) {
            cudaGetLastError();
            rc = QMS_ERR_LAUNCH;
            goto done;
        }
        if (cudaMemcpy(hash_out, d_hash, (size_t)hash_len, cudaMemcpyDeviceToHost) != cudaSuccess) {
            rc = QMS_ERR_MEMORY;
        }
    }

done:
    if (d_scratch) cudaFree(d_scratch);
    cudaFree(d_preimage);
    cudaFree(d_hash);
    return rc;
}

QMS_API int32_t qms_cuda_scan(
    const uint8_t *preimage, int32_t preimage_len,
    uint64_t start_nonce, uint64_t count,
    const uint8_t *mask, int32_t mask_len,
    uint64_t *out_nonces, int32_t out_capacity,
    int32_t time_cost, int32_t mem_cost, int32_t parallelism,
    int32_t version, int32_t key_length)
{
    if (!preimage || preimage_len <= 0 || !mask || mask_len <= 0 || !out_nonces || out_capacity <= 0) {
        return QMS_ERR_ARGUMENT;
    }
    if (count == 0) {
        return 0;
    }

    int blocks_per_thread = mem_cost;
    if (blocks_per_thread > QMS_MAX_MEM_BLOCKS) {
        blocks_per_thread = QMS_MAX_MEM_BLOCKS;
    }

    size_t per_thread = (size_t)blocks_per_thread * QMS_BLOCK_U64 * sizeof(uint64_t);
    int threads_per_block = QMS_THREADS_PER_BLOCK;

    size_t free_bytes = 0;
    size_t total_bytes = 0;
    if (cudaMemGetInfo(&free_bytes, &total_bytes) != cudaSuccess) {
        cudaGetLastError();
        free_bytes = 0;
    }

    int32_t cur_device = -1;
    if (cudaGetDevice(&cur_device) != cudaSuccess) {
        cudaGetLastError();
        cur_device = -1;
    }
    if (g_scratch_device != cur_device && g_scratch != NULL) {
        cudaFree(g_scratch);
        g_scratch = NULL;
        g_scratch_bytes = 0;
    }
    g_scratch_device = cur_device;

    int lane_slot = (cur_device >= 0 && cur_device < QMS_MAX_DEVICES) ? (int)cur_device : 0;
    QmsLaneGuard lane_guard(lane_slot);
    uint64_t active_lanes = (uint64_t)__atomic_load_n(&g_active_lanes[lane_slot], __ATOMIC_SEQ_CST);
    if (active_lanes < 1) {
        active_lanes = 1;
    }

    uint64_t slots = 0;
    if (g_scratch_bytes >= per_thread) {
        slots = (uint64_t)(g_scratch_bytes / per_thread);
    }
    if (total_bytes > 0) {
        uint64_t budget = (uint64_t)((total_bytes / 100) * QMS_SCRATCH_BUDGET_PCT) / active_lanes;
        if (budget > free_bytes) {
            budget = free_bytes;
        }
        uint64_t fits = budget / per_thread;
        if (fits > slots) {
            slots = fits;
        }
    }
    if (slots > QMS_MAX_SLOTS) {
        slots = QMS_MAX_SLOTS;
    }
    if (slots > QMS_MAX_SCRATCH_SLOTS) {
        slots = QMS_MAX_SCRATCH_SLOTS;
    }
    if (slots > count) {
        slots = count;
    }
    if (slots < 1) {
        slots = 1;
    }

    uint64_t *d_scratch = NULL;
    for (int attempt_index = 0; attempt_index < 6 && slots >= 1; ++attempt_index) {
        size_t need = per_thread * (size_t)slots;
        if (g_scratch != NULL && g_scratch_bytes >= need) {
            d_scratch = g_scratch;
            break;
        }
        uint64_t *attempt = NULL;
        if (cudaMalloc(&attempt, need) == cudaSuccess) {
            if (g_scratch != NULL) {
                cudaFree(g_scratch);
            }
            g_scratch = attempt;
            g_scratch_bytes = need;
            d_scratch = attempt;
            break;
        }
        cudaGetLastError();
        slots /= 2;
    }
    if (d_scratch == NULL) {
        return QMS_ERR_MEMORY;
    }
    if ((uint64_t)threads_per_block > slots) {
        threads_per_block = (int)slots;
    }

    uint8_t *d_preimage = NULL;
    char *d_mask = NULL;
    uint64_t *d_out = NULL;
    int32_t *d_found = NULL;
    int32_t rc = QMS_OK;

    if (cudaMalloc(&d_preimage, (size_t)preimage_len) != cudaSuccess) { rc = QMS_ERR_MEMORY; goto done; }
    if (cudaMalloc(&d_mask, (size_t)mask_len) != cudaSuccess) { rc = QMS_ERR_MEMORY; goto done; }
    if (cudaMalloc(&d_out, sizeof(uint64_t) * (size_t)out_capacity) != cudaSuccess) { rc = QMS_ERR_MEMORY; goto done; }
    if (cudaMalloc(&d_found, sizeof(int32_t)) != cudaSuccess) { rc = QMS_ERR_MEMORY; goto done; }

    if (cudaMemcpy(d_preimage, preimage, (size_t)preimage_len, cudaMemcpyHostToDevice) != cudaSuccess) { rc = QMS_ERR_MEMORY; goto done; }
    if (cudaMemcpy(d_mask, mask, (size_t)mask_len, cudaMemcpyHostToDevice) != cudaSuccess) { rc = QMS_ERR_MEMORY; goto done; }
    if (cudaMemset(d_found, 0, sizeof(int32_t)) != cudaSuccess) { rc = QMS_ERR_MEMORY; goto done; }

    {
        int64_t remaining = (int64_t)count;
        uint64_t cursor = start_nonce;
        while (remaining > 0) {
            uint64_t batch = slots;
            if ((int64_t)batch > remaining) {
                batch = (uint64_t)remaining;
            }
            int grid = (int)((batch + (uint64_t)threads_per_block - 1) / (uint64_t)threads_per_block);

            qms_scan_kernel<<<grid, threads_per_block>>>(
                d_preimage, preimage_len, cursor, batch,
                d_mask, mask_len, d_out, out_capacity, d_found,
                time_cost, mem_cost, parallelism, version, key_length,
                d_scratch, blocks_per_thread);

            cudaError_t err = cudaDeviceSynchronize();
            if (err != cudaSuccess) {
                cudaGetLastError();
                rc = QMS_ERR_LAUNCH;
                goto done;
            }

            int32_t found = 0;
            if (cudaMemcpy(&found, d_found, sizeof(int32_t), cudaMemcpyDeviceToHost) != cudaSuccess) {
                rc = QMS_ERR_MEMORY;
                goto done;
            }
            if (found >= out_capacity) {
                break;
            }

            cursor += batch;
            remaining -= (int64_t)batch;
        }

        int32_t found = 0;
        cudaMemcpy(&found, d_found, sizeof(int32_t), cudaMemcpyDeviceToHost);
        if (found > out_capacity) {
            found = out_capacity;
        }
        if (found > 0) {
            if (cudaMemcpy(out_nonces, d_out, sizeof(uint64_t) * (size_t)found, cudaMemcpyDeviceToHost) != cudaSuccess) {
                rc = QMS_ERR_MEMORY;
                goto done;
            }
        }
        rc = found;
    }

done:
    if (d_preimage) cudaFree(d_preimage);
    if (d_mask) cudaFree(d_mask);
    if (d_out) cudaFree(d_out);
    if (d_found) cudaFree(d_found);
    if (d_scratch && d_scratch != g_scratch) cudaFree(d_scratch);
    return rc;
}

QMS_API int32_t qms_cuda_selftest(void)
{
    int count = 0;
    if (cudaGetDeviceCount(&count) != cudaSuccess || count <= 0) {
        return QMS_ERR_DEVICE;
    }
    return QMS_OK;
}

}
