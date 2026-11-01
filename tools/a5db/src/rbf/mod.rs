//! `.rbf` writer: a `.sof` image to the bitstream quartus_pfg would make,
//! without Quartus (README (l), (o)). The CMF section is the install's
//! signed `main.cmf` with one word and its block CRC patched; the IO and
//! CORE sections are pfg's byte for byte (CORE through pfg's own 2D
//! compression, S4 flags and page interleave).

pub mod core;
pub mod deflate;
pub mod install;
pub mod io;
pub mod section;
mod tables;

use anyhow::{ensure, Context, Result};

use crate::db::Db;
use install::{pb_parse, Install, Pb};
use section::{Fields, Kind, Page, FLAG_ACTION, PAGE};

pub const RBF_MAGIC: u32 = 0x6229_4895;

/// The design's "IO hash" and "Design hash" from the .sof (README (g2)):
/// the IO hash is hex text in the OPTIONS record (a later 0x3a record,
/// after its u32 index, field 1.20), the design hash raw in record 0x36
/// field 2.10.
pub fn sof_hashes(records: &[(u16, &[u8])]) -> Result<([u8; 32], [u8; 32])> {
    let field = |b: &[u8], path: &[u64]| -> Option<Vec<u8>> {
        let mut cur = b.to_vec();
        if pb_parse(&cur).is_none() {
            cur = b.get(4..)?.to_vec();
        }
        for &f in path {
            let fs = pb_parse(&cur)?;
            let v = fs.into_iter().find_map(|(g, v)| match v {
                Pb::Bytes(x) if g == f => Some(x.to_vec()),
                _ => None,
            })?;
            cur = v;
        }
        Some(cur)
    };
    let mut io = None;
    let mut design = None;
    for (tag, data) in records.iter().skip(1) {
        if *tag == 0x3a && io.is_none() {
            if let Some(t) = field(data, &[1, 20]) {
                if t.len() == 64 && t.iter().all(u8::is_ascii_hexdigit) {
                    let s = std::str::from_utf8(&t)?;
                    let mut h = [0u8; 32];
                    for i in 0..32 {
                        h[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16)?;
                    }
                    io = Some(h);
                }
            }
        }
        if *tag == 0x36 {
            if let Some(d) = field(data, &[2, 10]) {
                design = d.try_into().ok();
            }
        }
    }
    Ok((io.context("no IO hash (record 0x3a field 1.20)")?, design.context("no design hash (record 0x36 field 2.10)")?))
}

/// The CMF section: main.cmf with +0x1F00 = number of sections after it and
/// block 1's CRC recomputed.
fn cmf(inst: &Install, following: u32) -> Result<Vec<u8>> {
    let mut c = inst.fw("main.cmf")?.to_vec();
    c[0x1f00..0x1f04].copy_from_slice(&following.to_le_bytes());
    let crc = section::crc32_bzip2(&c[0x1000..0x1ffc]);
    c[0x1ffc..0x2000].copy_from_slice(&crc.to_le_bytes());
    Ok(c)
}

fn hashed(pages: Vec<Vec<u8>>, flag: u32) -> impl Iterator<Item = Page> {
    pages.into_iter().map(move |p| Page::Hashed(p, flag))
}

fn io_section(db: &mut Db, inst: &Install, img: &[u8], f: &Fields, key: &[u8]) -> Result<Vec<u8>> {
    let ranges = io::pd_ranges(db)?;
    let streams = io::streams(img, &ranges);
    let pages: Vec<Vec<Vec<u8>>> = streams.iter().map(|s| Ok(section::stream_pages(&deflate::deflate(s)?))).collect::<Result<_>>()?;
    let count = |i: usize| (pages[i].len() as u32, vec![false; pages[i].len()]);
    let act1 = section::action_page(tables::IO_ACT1, &[count(0), count(1), count(2)])?;
    let act2 = section::action_page(tables::IO_ACT2, &[count(3), count(4), count(5)])?;
    let mut out: Vec<Page> = Vec::new();
    out.extend(section::module(inst.fw("enable_pm.fw")?)?);
    out.extend(section::module(inst.fw("cssm.fw")?)?);
    out.push(Page::Hashed(act1, FLAG_ACTION));
    let mut pages = pages.into_iter();
    for _ in 0..3 {
        out.extend(hashed(pages.next().unwrap(), 0));
    }
    out.extend(section::module(inst.fw("cssm_reset_release.fw")?)?);
    out.extend(section::module(inst.fw("cssm_ux_phy.fw")?)?);
    out.extend(section::module(inst.fw("io_config.fw")?)?);
    out.push(Page::Hashed(act2, FLAG_ACTION));
    for p in pages {
        out.extend(hashed(p, 0));
    }
    section::assemble(out, f, key)
}

fn core_section(inst: &Install, img: &[u8], f: &Fields, key: &[u8]) -> Result<Vec<u8>> {
    let lay = core::Layout::new(&inst.s4)?;
    let (s0, n) = io::part("sector0", "cram");
    let (s1, _) = io::part("sector1", "cram");
    ensure!(n == core::NF * core::FB);
    let enc = core::encode(&lay, [&img[s0..s0 + n], &img[s1..s1 + n]], &inst.s4_flags);
    let offs = enc.offs;
    let lib_pages: Vec<Vec<u8>> = enc.libs.chunks(PAGE).map(<[u8]>::to_vec).collect();
    // Per sector: codebook, M20K contents, second codebook, BBM, then the
    // frame stream. The contents' pages go in the frame pages' interleave.
    let ram: Vec<Vec<u8>> = ["sector0", "sector1"]
        .iter()
        .map(|o| {
            let (s, n) = io::part(o, "ram");
            deflate::deflate(&core::ram_block(&img[s..s + n]))
        })
        .collect::<Result<_>>()?;
    let rpages: Vec<Vec<Vec<u8>>> = ram.iter().map(|d| section::stream_pages(d)).collect();
    let deflated: Vec<Vec<u8>> = enc.frames.iter().map(|f| deflate::deflate(f)).collect::<Result<_>>()?;
    let fpages: Vec<Vec<Vec<u8>>> = deflated.iter().map(|d| section::stream_pages(d)).collect();
    let frames = [fpages[0].len(), fpages[1].len()];
    // Lib pages sector by sector, the setup pages alternating, then the frame
    // pages in pfg's interleave.
    let mut data: Vec<Page> = Vec::new();
    for s in 0..2 {
        data.extend(hashed(lib_pages.clone(), s));
    }
    for s in 0..2 {
        data.push(Page::Hashed(inst.cb1.clone(), s as u32));
    }
    let mut ram_bitmap = Vec::new();
    let mut next = [0usize; 2];
    for s in interleave([ram[0].len(), ram[1].len()]) {
        ram_bitmap.push(s == 1);
        data.push(Page::Hashed(rpages[s][next[s]].clone(), s as u32));
        next[s] += 1;
    }
    ensure!(next == [rpages[0].len(), rpages[1].len()], "interleave placed {next:?} contents pages");
    for s in 0..2 {
        data.push(Page::Hashed(inst.cb2.clone(), s as u32));
    }
    for s in 0..2 {
        data.push(Page::Hashed(enc.bbm[s].clone(), s as u32));
    }
    let mut bitmap = Vec::new();
    let mut next = [0usize; 2];
    for s in interleave([deflated[0].len(), deflated[1].len()]) {
        bitmap.push(s == 1);
        data.push(Page::Hashed(fpages[s][next[s]].clone(), s as u32));
        next[s] += 1;
    }
    ensure!(next == frames, "interleave placed {next:?} of {frames:?} frame pages");
    ensure!(bitmap.len() == frames[0] + frames[1]);
    // With no libs pfg sends no lib pages and drops their 0x15 records.
    let recs: Vec<(u32, Vec<u32>)> = tables::CORE_ACT1
        .iter()
        .filter(|(t, w)| !(*t == 0x15 && w[2] == 4 && lib_pages.is_empty()))
        .map(|(t, w)| {
            let mut w = w.to_vec();
            if *t == 0x1f {
                w[1..4].copy_from_slice(&offs);
            }
            if *t == 0x15 && w[2] == 4 {
                w[0] = lib_pages.len() as u32;
            }
            (*t, w)
        })
        .collect();
    let recs: Vec<(u32, &[u32])> = recs.iter().map(|(t, w)| (*t, w.as_slice())).collect();
    let act1 = section::action_page(&recs, &[(ram_bitmap.len() as u32, ram_bitmap), (bitmap.len() as u32, bitmap)])?;
    let (h0s, h0n) = io::part("sector0", "hdr");
    let (h1s, h1n) = io::part("sector1", "hdr");
    let shdr = section::stream_pages(&deflate::deflate(&core::sector_header_stream(&img[h0s..h0s + h0n], &img[h1s..h1s + h1n]))?);
    let act2 = section::action_page(tables::CORE_ACT2, &[(shdr.len() as u32, vec![false; shdr.len()])])?;
    let mut out: Vec<Page> = Vec::new();
    out.extend(section::module(inst.fw("lsm_init.fw")?)?);
    out.push(Page::Hashed(act1, FLAG_ACTION));
    out.extend(data);
    out.extend(section::module(inst.fw("unfreeze_routing_fw.fw")?)?);
    out.push(Page::Hashed(act2, FLAG_ACTION));
    out.extend(hashed(shdr, 0));
    out.extend(section::module(inst.fw("enter_user_mode_fw.fw")?)?);
    section::assemble(out, f, key)
}

/// The order in which pfg sends the two sectors' frame pages
/// (`BITASM_FAMILY_VAR1::get_group_data`, README (o)). The sector with more
/// pages (R; sector 0 on a tie) sends on every turn; the other sends only
/// while its bytes sent, scaled by the page-count ratio, stay within 4 KiB
/// of R's. Turns alternate 0, 1, 0, … starting at R, for as many rounds as
/// R has pages.
fn interleave(sizes: [usize; 2]) -> Vec<usize> {
    let n = sizes.map(|s| s.div_ceil(PAGE));
    let r = if n[0] >= n[1] { n[0] as f64 / n[1] as f64 } else { n[1] as f64 / n[0] as f64 };
    let big = usize::from(n[1] > n[0]);
    let rounds = sizes[0].max(sizes[1]).div_ceil(PAGE);
    let mut off = [0usize; 2];
    let mut out = Vec::new();
    let mut emit = |t: usize, off: &mut [usize; 2]| {
        if off[t] < sizes[t] {
            out.push(t);
            off[t] = (off[t] + PAGE).min(sizes[t]);
        }
    };
    let (mut t, mut round) = (big, 0);
    emit(t, &mut off);
    loop {
        loop {
            t += 1;
            if t == 2 {
                t = 0;
                round += 1;
                if round == rounds {
                    return out;
                }
            }
            if t == big || (off[t] as f64) * r - off[big] as f64 <= PAGE as f64 {
                break;
            }
        }
        emit(t, &mut off);
    }
}

/// Byte sum of every object header and both sectors' CRAM and M20K
/// contents (header +0x78; the contents were always zero before README (y)).
fn header_sum(img: &[u8]) -> u32 {
    let mut s = 0u32;
    for (o, p, start, n) in crate::db::PARTS {
        if *p == "hdr" || ((*p == "cram" || *p == "ram") && o.starts_with("sector")) {
            s = img[*start..start + n].iter().fold(s, |a, &b| a.wrapping_add(b as u32));
        }
    }
    s
}

pub fn write(db: &mut Db, inst: &Install, sof: &crate::sof::Sof, key: &[u8]) -> Result<Vec<u8>> {
    let img = sof.image();
    let (io_hash, design_hash) = sof_hashes(&sof.records())?;
    let sum = header_sum(img);
    let io_f = Fields { kind: Kind::Io, io_hash, design_hash, sum, following: 1 };
    let core_f = Fields { kind: Kind::Core, io_hash, design_hash, sum, following: 0 };
    let mut out = cmf(inst, 2)?;
    ensure!(u32::from_le_bytes(out[0..4].try_into().unwrap()) == RBF_MAGIC, "main.cmf does not start with the .rbf magic");
    out.extend(io_section(db, inst, img, &io_f, key)?);
    out.extend(core_section(inst, img, &core_f, key)?);
    Ok(out)
}
