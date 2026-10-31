//! The CORE section's data streams, all literal: every 98-frame group of
//! both sectors stored column by column with BBM code 0, no lib copies or
//! row prediction (README (g) "CORE section: 2D compression", (l)). S4
//! pairs carry pfg's flag nibbles (README (o)). It decodes to the same CRAM
//! as pfg's own encoding, but isn't pfg's byte stream.

use anyhow::{ensure, Result};

pub const NF: usize = 4704; // frames per sector
pub const FB: usize = 540; // dr bytes per frame
pub const PF: usize = 656; // physical frame bytes
const GROUP: usize = 98;
const HDR: [u8; 8] = [4, 0, 0xa4, 0, 0, 0, 0, 0];

/// Frame groups the stream carries: group 1 is never stored.
pub fn stored_groups() -> impl Iterator<Item = usize> {
    std::iter::once(0).chain(2..48)
}

/// dr bits with no physical bit.
const DROPPED: [(usize, usize); 7] = [(256, 275), (1664, 1675), (2080, 2099), (2496, 2515), (3392, 3403), (3808, 3827), (4192, 4215)];
/// 4-bit dr groups stored as two pad bits, (b+2, b+3), two pad bits, (b, b+1).
const PADDED_PAIRS: [usize; 3] = [4264, 3488, 1760];
/// 32 always-zero physical bits.
const PAD32_AT: usize = 2528;

#[derive(Clone, Copy)]
enum Phys {
    Copy(usize),
    /// An S4 pair (a, b) as a nibble starting at this bit: `0 0 a b`, or
    /// the flag nibble when one side is set and its key is listed. The keys
    /// are the pair's bits in pfg's swizzled frame order (ka for a, kb for b).
    Nib { a: usize, b: usize, ka: u16, kb: u16 },
}

/// Physical bit → dr bit (README (g), "Physical frame ↔ dr").
pub struct Layout(Vec<Option<Phys>>);

impl Layout {
    pub fn new(s4: &[usize]) -> Result<Layout> {
        let dropped = |d: usize| DROPPED.iter().any(|&(a, b)| (a..=b).contains(&d));
        let groups: std::collections::HashSet<usize> = s4.iter().copied().collect();
        let mut out: Vec<Option<Phys>> = Vec::new();
        let mut d = 4319i64;
        while d >= 0 {
            let u = d as usize;
            if dropped(u) {
                d -= 1;
            } else if u >= 3 && PADDED_PAIRS.contains(&(u - 3)) {
                let b = u - 3;
                out.extend([None, None, Some(Phys::Copy(b + 2)), Some(Phys::Copy(b + 3)), None, None, Some(Phys::Copy(b)), Some(Phys::Copy(b + 1))]);
                d = b as i64 - 1;
            } else if u >= 7 && groups.contains(&(u - 7)) {
                let g = u - 7;
                out.extend([Some(Phys::Copy(g + 5)), Some(Phys::Copy(g + 1)), Some(Phys::Copy(g + 4)), Some(Phys::Copy(g))]);
                let nib = |a, b, ka: usize| Some(Phys::Nib { a, b, ka: ka as u16, kb: ka as u16 + 1 });
                out.extend([nib(g + 3, g + 7, g + 2), None, None, None, nib(g + 2, g + 6, g), None, None, None]);
                d = g as i64 - 1;
            } else {
                out.push(Some(Phys::Copy(u)));
                d -= 1;
            }
        }
        out.splice(PAD32_AT..PAD32_AT, std::iter::repeat_n(None, 32));
        ensure!(out.len() == PF * 8, "physical layout has {} bits, expected {}", out.len(), PF * 8);
        Ok(Layout(out))
    }

    /// dr frame → physical frame; `flags` is the frame's S4 key list.
    pub fn to_phys(&self, dr: &[u8], flags: &[(u16, u8)]) -> Vec<u8> {
        let bit = |d: usize| (dr[d >> 3] >> (d & 7)) & 1;
        let mut out = vec![0u8; PF];
        let mut set = |pb: usize, v: u8| out[pb >> 3] |= v << (pb & 7);
        for (pb, m) in self.0.iter().enumerate() {
            match *m {
                Some(Phys::Copy(d)) => set(pb, bit(d)),
                Some(Phys::Nib { a, b, ka, kb }) => {
                    let (va, vb) = (bit(a), bit(b));
                    let key = if va == vb { None } else { Some(if va == 1 { ka } else { kb }) };
                    match key.and_then(|k| flags.binary_search_by_key(&k, |&(k, _)| k).ok()) {
                        Some(i) => (0..4).for_each(|j| set(pb + j, flags[i].1 >> j & 1)),
                        None => {
                            set(pb + 2, va);
                            set(pb + 3, vb);
                        }
                    }
                }
                None => {}
            }
        }
        out
    }
}

/// One sector's inflated frame stream: every stored frame as a packet of
/// the 8-byte header and the whole physical frame (all columns literal).
pub fn frame_stream(lay: &Layout, cram: &[u8], s4_flags: &[Vec<(u16, u8)>]) -> Vec<u8> {
    let mut out = Vec::with_capacity(47 * GROUP * (8 + PF));
    for g in stored_groups() {
        for f in GROUP * g..GROUP * (g + 1) {
            out.extend_from_slice(&HDR);
            out.extend(lay.to_phys(&cram[f * FB..(f + 1) * FB], &s4_flags[f]));
        }
    }
    out
}

/// The BBM page: the 15 IP types (id, width / 2, 98, 0), a u16, then 48
/// rows of (u16, 60 nibbles). Row 0 names each column's IP; rows 1..47
/// are all code 0 (literal).
pub fn bbm_page() -> Vec<u8> {
    const HEAD: &str = "0105620002056200030c62000408620005086200060462000700620008006200090062000a0062000b0062000c0062000d0062000e0062000f0462000000\
                        0000252141121223212121212141121246121223212121212121212121411212";
    let mut p = super::section::hex(HEAD);
    p.resize(4096, 0);
    p
}

/// Lib geometry: one zero lib per IP id 1..4 (no code refers to them).
/// Returns (lib bytes, the 0x1f record's offset words in 8-byte units).
pub fn libs() -> (Vec<u8>, [u32; 3]) {
    let mut offs = vec![0usize];
    for wid in [10usize, 10, 24, 16] {
        let stride = if wid < 12 { wid.div_ceil(4) * 4 } else { wid.div_ceil(8) * 8 };
        offs.push(offs.last().unwrap() + GROUP * stride / 8);
    }
    let bytes = offs[4] * 8;
    (vec![0u8; bytes.div_ceil(4096) * 4096], [(offs[1] << 16 | offs[0]) as u32, (offs[3] << 16 | offs[2]) as u32, offs[4] as u32])
}

/// The block pfg stores after the first codebook: 47 groups x 4 records of
/// (8, w, 128, 2) and 2,564 zero bytes, w = 12, 16, 20, 25 (482,784 bytes;
/// the same in every specimen).
pub fn zero_block() -> Vec<u8> {
    let mut out = Vec::with_capacity(482_784);
    for _ in 0..47 {
        for w in [12u8, 16, 20, 25] {
            out.extend_from_slice(&[8, w, 128, 2]);
            out.extend(std::iter::repeat_n(0, 2564));
        }
    }
    out
}

/// The stream after the second action page: both sector headers, twice,
/// each as a send of the header bits reversed and shifted by 24 bits with a
/// command word in front; the first pass forces a few bits (README (l)).
pub fn sector_header_stream(h0: &[u8], h1: &[u8]) -> Vec<u8> {
    fn body(h: &[u8], set: &[usize], clear: &[usize]) -> Vec<u8> {
        let n = h.len() * 8;
        let bit = |i: usize| (h[i >> 3] >> (i & 7)) & 1;
        let mut out = vec![0u8; h.len()];
        for j in 0..n {
            if n >= 25 + j && bit(n - 25 - j) == 1 {
                out[j >> 3] |= 1 << (j & 7);
            }
        }
        for &j in set {
            out[j >> 3] |= 1 << (j & 7);
        }
        for &j in clear {
            out[j >> 3] &= !(1 << (j & 7));
        }
        out
    }
    let passes: [[(&[u8], &[usize], &[usize], u32); 2]; 2] = [
        [(h0, &[2, 3, 6, 19, 21, 23, 26, 27, 832, 1885, 2134, 3183], &[], 0), (h1, &[2, 3, 6, 19, 21, 23, 27, 768, 921, 1494, 2095], &[173], 1)],
        [(h0, &[6, 19, 21, 23, 26, 27], &[], 0), (h1, &[6, 19, 21, 23, 27], &[], 1)],
    ];
    let mut out = Vec::new();
    for pass in passes {
        for (h, set, clear, flags) in pass {
            let b = body(h, set, clear);
            let n = (b.len() / 4) as u32;
            out.extend_from_slice(&(flags << 8 | n << 16).to_le_bytes());
            out.extend_from_slice(&[0; 4]);
            out.extend_from_slice(&b);
            if n & 1 == 1 {
                out.extend_from_slice(&[0; 4]);
            }
        }
        for w in [0x0064_000du32, 0, 0x0004_000c, 0] {
            out.extend_from_slice(&w.to_le_bytes());
        }
    }
    out
}
