use crate::domain::params::PowParams;

const IV: [u64; 8] = [
    0x6a09e667f3bcc908,
    0xbb67ae8584caa73b,
    0x3c6ef372fe94f82b,
    0xa54ff53a5f1d36f1,
    0x510e527fade682d1,
    0x9b05688c2b3e6c1f,
    0x1f83d9abfb41bd6b,
    0x5be0cd19137e2179,
];

const SIGMA: [[usize; 16]; 12] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
];

fn blake2b(data: &[u8], out_len: usize) -> Vec<u8> {
    let mut h = IV;
    h[0] ^= 0x0101_0000 ^ (out_len as u64);

    let mut counter: u128 = 0;
    let mut blocks: Vec<&[u8]> = Vec::new();
    let mut chunks = data.chunks_exact(128);
    for c in &mut chunks {
        blocks.push(c);
    }
    let rem = chunks.remainder();
    let mut last = [0u8; 128];
    last[..rem.len()].copy_from_slice(rem);
    blocks.push(&last);
    if data.is_empty() {
        blocks = vec![&last];
    }

    let total = blocks.len();
    for (index, block) in blocks.iter().enumerate() {
        counter += 128;
        if index + 1 == total {
            counter -= 128 - rem.len() as u128;
        }
        let mut m = [0u64; 16];
        for i in 0..16 {
            let mut word = [0u8; 8];
            word.copy_from_slice(&block[i * 8..i * 8 + 8]);
            m[i] = u64::from_le_bytes(word);
        }
        compress(&mut h, &m, counter, index + 1 == total);
    }

    let mut out = Vec::with_capacity(out_len);
    for word in h.iter() {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out.truncate(out_len);
    out
}

fn compress(h: &mut [u64; 8], m: &[u64; 16], counter: u128, last: bool) {
    let mut v = [0u64; 16];
    v[..8].copy_from_slice(h);
    v[8..].copy_from_slice(&IV);
    v[12] ^= counter as u64;
    v[13] ^= (counter >> 64) as u64;
    if last {
        v[14] = !v[14];
    }

    for round in 0..12 {
        let s = &SIGMA[round];
        mix(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
        mix(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
        mix(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
        mix(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
        mix(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
        mix(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
        mix(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
        mix(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
    }

    for i in 0..8 {
        h[i] ^= v[i] ^ v[i + 8];
    }
}

fn mix(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = (v[d] ^ v[a]).rotate_right(32);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(24);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(63);
}

fn h_prime(input: &[u8], out_len: usize) -> Vec<u8> {
    if out_len <= 64 {
        let mut buffer = Vec::with_capacity(4 + input.len());
        buffer.extend_from_slice(&(out_len as u32).to_le_bytes());
        buffer.extend_from_slice(input);
        return blake2b(&buffer, out_len);
    }

    let rounds = out_len.div_ceil(32) - 2;
    let mut out = Vec::with_capacity(out_len);
    let mut buffer = Vec::with_capacity(4 + input.len());
    buffer.extend_from_slice(&(out_len as u32).to_le_bytes());
    buffer.extend_from_slice(input);
    let mut value = blake2b(&buffer, 64);
    out.extend_from_slice(&value[..32]);
    for _ in 1..rounds {
        value = blake2b(&value, 64);
        out.extend_from_slice(&value[..32]);
    }
    let rest = out_len - 32 * rounds;
    let tail = blake2b(&value, rest);
    out.extend_from_slice(&tail);
    out
}

fn block_from(bytes: &[u8; 1024]) -> [u64; 128] {
    let mut words = [0u64; 128];
    for i in 0..128 {
        let mut word = [0u8; 8];
        word.copy_from_slice(&bytes[i * 8..i * 8 + 8]);
        words[i] = u64::from_le_bytes(word);
    }
    words
}

fn block_to(words: &[u64; 128]) -> [u8; 1024] {
    let mut bytes = [0u8; 1024];
    for i in 0..128 {
        bytes[i * 8..i * 8 + 8].copy_from_slice(&words[i].to_le_bytes());
    }
    bytes
}

fn round_nomsg(v: &mut [u64; 16]) {
    g(v, 0, 4, 8, 12);
    g(v, 1, 5, 9, 13);
    g(v, 2, 6, 10, 14);
    g(v, 3, 7, 11, 15);
    g(v, 0, 5, 10, 15);
    g(v, 1, 6, 11, 12);
    g(v, 2, 7, 8, 13);
    g(v, 3, 4, 9, 14);
}

fn mix_block(prev: &[u64; 128], reference: &[u64; 128], next: &mut [u64; 128], with_xor: bool) {
    let mut r = [0u64; 128];
    for i in 0..128 {
        r[i] = prev[i] ^ reference[i];
    }
    let mut tmp = r;
    if with_xor {
        for i in 0..128 {
            tmp[i] ^= next[i];
        }
    }
    for row in 0..8 {
        let base = row * 16;
        let mut v = [0u64; 16];
        v.copy_from_slice(&r[base..base + 16]);
        round_nomsg(&mut v);
        r[base..base + 16].copy_from_slice(&v);
    }
    for i in 0..8 {
        let idx = [
            2 * i,
            2 * i + 1,
            2 * i + 16,
            2 * i + 17,
            2 * i + 32,
            2 * i + 33,
            2 * i + 48,
            2 * i + 49,
            2 * i + 64,
            2 * i + 65,
            2 * i + 80,
            2 * i + 81,
            2 * i + 96,
            2 * i + 97,
            2 * i + 112,
            2 * i + 113,
        ];
        let mut v = [0u64; 16];
        for (k, j) in idx.iter().enumerate() {
            v[k] = r[*j];
        }
        round_nomsg(&mut v);
        for (k, j) in idx.iter().enumerate() {
            r[*j] = v[k];
        }
    }
    for i in 0..128 {
        next[i] = tmp[i] ^ r[i];
    }
}

fn g(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize) {
    v[a] = v[a]
        .wrapping_add(v[b])
        .wrapping_add(2u64.wrapping_mul((v[a] as u32 as u64).wrapping_mul(v[b] as u32 as u64)));
    v[d] = (v[d] ^ v[a]).rotate_right(32);
    v[c] = v[c]
        .wrapping_add(v[d])
        .wrapping_add(2u64.wrapping_mul((v[c] as u32 as u64).wrapping_mul(v[d] as u32 as u64)));
    v[b] = (v[b] ^ v[c]).rotate_right(24);
    v[a] = v[a]
        .wrapping_add(v[b])
        .wrapping_add(2u64.wrapping_mul((v[a] as u32 as u64).wrapping_mul(v[b] as u32 as u64)));
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c]
        .wrapping_add(v[d])
        .wrapping_add(2u64.wrapping_mul((v[c] as u32 as u64).wrapping_mul(v[d] as u32 as u64)));
    v[b] = (v[b] ^ v[c]).rotate_right(63);
}

pub fn hash(params: &PowParams, password: &[u8], salt: &[u8], version: u32) -> Vec<u8> {
    let lanes = params.parallelism.max(1) as usize;
    let memory = params.mem_cost.max(8) as usize;
    let segment_length = memory / (lanes * 4);
    let lane_length = segment_length * 4;
    let rounds = params.time_cost.max(1) as usize;
    let out_len = params.key_length.max(4) as usize;
    let with_xor = version != 0x10;

    let mut h0 = Vec::with_capacity(64 + password.len() + salt.len());
    h0.extend_from_slice(&(lanes as u32).to_le_bytes());
    h0.extend_from_slice(&(out_len as u32).to_le_bytes());
    h0.extend_from_slice(&(params.mem_cost as u32).to_le_bytes());
    h0.extend_from_slice(&(params.time_cost as u32).to_le_bytes());
    h0.extend_from_slice(&version.to_le_bytes());
    h0.extend_from_slice(&params.type_cost.to_le_bytes());
    h0.extend_from_slice(&(password.len() as u32).to_le_bytes());
    h0.extend_from_slice(password);
    h0.extend_from_slice(&(salt.len() as u32).to_le_bytes());
    h0.extend_from_slice(salt);
    h0.extend_from_slice(&0u32.to_le_bytes());
    h0.extend_from_slice(&0u32.to_le_bytes());

    let initial_hash = blake2b(&h0, 64);
    let mut memory_blocks: Vec<[u64; 128]> = vec![[0u64; 128]; lane_length * lanes];
    for lane in 0..lanes {
        for index in 0..2 {
            let mut seed = initial_hash.clone();
            seed.extend_from_slice(&(index as u32).to_le_bytes());
            seed.extend_from_slice(&(lane as u32).to_le_bytes());
            let bytes = h_prime(&seed, 1024);
            let mut fixed = [0u8; 1024];
            fixed.copy_from_slice(&bytes);
            memory_blocks[lane * lane_length + index] = block_from(&fixed);
        }
    }

    for pass in 0..rounds {
        for slice in 0..4 {
            for lane in 0..lanes {
                let mut index = if pass == 0 && slice == 0 { 2 } else { 0 };
                while index < segment_length {
                    let current = lane * lane_length + slice * segment_length + index;
                    let prev = if current % lane_length == 0 {
                        current + lane_length - 1
                    } else {
                        current - 1
                    };

                    let pseudo = memory_blocks[prev][0];
                    let mut ref_lane = ((pseudo >> 32) % lanes as u64) as usize;
                    if pass == 0 && slice == 0 {
                        ref_lane = lane;
                    }

                    let same_lane = ref_lane == lane;
                    let reference_area = if pass == 0 {
                        if slice == 0 {
                            index - 1
                        } else if same_lane {
                            slice * segment_length + index - 1
                        } else if index == 0 {
                            slice * segment_length - 1
                        } else {
                            slice * segment_length
                        }
                    } else if same_lane {
                        lane_length - segment_length + index - 1
                    } else if index == 0 {
                        lane_length - segment_length - 1
                    } else {
                        lane_length - segment_length
                    };

                    let area = reference_area.max(1) as u64;
                    let mut relative = (pseudo & 0xffff_ffff) as u64;
                    relative = (relative * relative) >> 32;
                    relative = area - 1 - ((area * relative) >> 32);
                    let start = if pass == 0 {
                        0
                    } else if slice == 3 {
                        0
                    } else {
                        (slice + 1) * segment_length
                    };
                    let absolute = (start + relative as usize) % lane_length;
                    let reference = ref_lane * lane_length + absolute;

                    let prev_block = memory_blocks[prev];
                    let ref_block = memory_blocks[reference];
                    let mut next_block = memory_blocks[current];
                    let xor = with_xor && pass > 0;
                    mix_block(&prev_block, &ref_block, &mut next_block, xor);
                    memory_blocks[current] = next_block;

                    index += 1;
                }
            }
        }
    }

    let mut final_block = memory_blocks[lane_length - 1];
    for lane in 1..lanes {
        for i in 0..128 {
            final_block[i] ^= memory_blocks[lane * lane_length + lane_length - 1][i];
        }
    }
    let fb = block_to(&final_block);
    h_prime(&fb, out_len)
}
