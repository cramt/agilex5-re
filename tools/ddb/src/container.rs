//! The PDB v25 `.ddb` outer container (Quartus Prime Pro 26.1): a fixed
//! 0xAA-byte header, a compressed body, and a compressed footer holding the
//! class table. Field names are the ones Quartus prints in its
//! `PDB_ASCII_DUMP` trace (`HEADER: ...`).

use anyhow::{bail, ensure, Context, Result};
use std::io::Read;

use crate::pdb::Cursor;

/// `m_compression_info.type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Compression {
    /// `is_enabled == 0`: body and footer are stored as is.
    None,
    /// Type 0: zlib header + deflate, ended by a sync flush and no final
    /// block (the same quirk as Quartus 13).
    Zlib,
    /// Type 2: raw LZMA1, a 5-byte properties header (lc/lp/pb byte and a
    /// u32 dictionary size) and the range-coded stream, with no size field
    /// and no end marker. The size comes from the PDB header.
    Lzma1,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Header {
    pub version_number: i32,
    pub size_of_header: u16,
    pub end_of_header_pos: u16,
    /// Quartus build string, e.g. "Version 26.1.1 Build 130 ...".
    pub version_string: String,
    pub size_of_body: u64,
    pub end_of_body_pos: u64,
    pub size_of_footer: u64,
    pub compression: Compression,
    pub compression_chunk_size: i32,
    pub compression_level: i32,
    /// Hex SHA-1; every file seen has SHA-1("") here.
    pub pdb_core_signature: String,
    pub end_of_footer_pos: u64,
    pub num_user_sections: u64,
}

const MAGIC: u16 = 0x0bad;
/// 0x1bad marks a file whose writer died before committing (Quartus 13).
const MAGIC_TRUNCATED: u16 = 0x1bad;
const VERSION: i32 = 25;

pub struct Ddb {
    pub header: Header,
    pub body: Vec<u8>,
    pub footer: Vec<u8>,
}

impl Ddb {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let raw = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        Self::parse(&raw)
    }

    pub fn parse(raw: &[u8]) -> Result<Self> {
        let h = parse_header(raw)?;
        ensure!(h.end_of_footer_pos as usize == raw.len(), "m_end_of_footer_pos {} != file size {}", h.end_of_footer_pos, raw.len());
        let body = &raw[h.end_of_header_pos as usize..h.end_of_body_pos as usize];
        let footer = &raw[h.end_of_body_pos as usize..];
        let body = decompress(h.compression, body, h.size_of_body).context("body")?;
        let footer = decompress(h.compression, footer, h.size_of_footer).context("footer")?;
        Ok(Ddb { header: h, body, footer })
    }
}

pub fn parse_header(raw: &[u8]) -> Result<Header> {
    let mut c = Cursor::new(raw);
    let version_number = c.i32()?;
    let magic = c.u16()?;
    ensure!(magic != MAGIC_TRUNCATED, "truncated database (header id 0x1bad)");
    ensure!(magic == MAGIC, "bad header id {magic:#x}");
    ensure!(version_number == VERSION, "unsupported header version {version_number}");
    let (u0, u1) = (c.i32()?, c.i16()?);
    ensure!(u0 == 0 && u1 == 0, "unused header fields are {u0:#x} {u1:#x}");
    let size_of_header = c.u16()?;
    let end_of_header_pos = c.u16()?;
    let version_string = c.c_str()?.unwrap_or_default();
    let size_of_body = c.u64()?;
    let end_of_body_pos = c.u64()?;
    let size_of_footer = c.u64()?;
    let is_enabled = c.i32()?;
    let ty = c.i32()?;
    let compression_chunk_size = c.i32()?;
    let compression_level = c.i32()?;
    let compression = match (is_enabled, ty) {
        (0, _) => Compression::None,
        (1, 0) => Compression::Zlib,
        (1, 2) => Compression::Lzma1,
        (e, t) => bail!("unknown compression is_enabled={e} type={t}"),
    };
    // Every file seen is one stream; a nonzero chunk size would mean a
    // chunked layout nobody has looked at yet.
    ensure!(compression_chunk_size == 0, "chunked compression ({compression_chunk_size}) not supported");
    let pdb_core_signature = c.c_str()?.unwrap_or_default();
    let end_of_footer_pos = c.u64()?;
    let num_user_sections = c.u64()?;
    ensure!(c.pos() == end_of_header_pos as usize, "header ends at {:#x}, m_end_of_header_pos is {end_of_header_pos:#x}", c.pos());
    ensure!(size_of_header == end_of_header_pos, "m_size_of_header {size_of_header} != m_end_of_header_pos {end_of_header_pos}");
    ensure!(end_of_header_pos as u64 <= end_of_body_pos && end_of_body_pos <= end_of_footer_pos, "section bounds out of order");
    Ok(Header {
        version_number,
        size_of_header,
        end_of_header_pos,
        version_string,
        size_of_body,
        end_of_body_pos,
        size_of_footer,
        compression,
        compression_chunk_size,
        compression_level,
        pdb_core_signature,
        end_of_footer_pos,
        num_user_sections,
    })
}

fn decompress(kind: Compression, src: &[u8], expect: u64) -> Result<Vec<u8>> {
    let out = match kind {
        Compression::None => src.to_vec(),
        Compression::Zlib => {
            let mut out = Vec::with_capacity(expect as usize);
            // No final block, so read_to_end reports UnexpectedEof once the
            // input is exhausted; everything decoded up to then is valid.
            let mut z = flate2::read::ZlibDecoder::new(src);
            match z.read_to_end(&mut out) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {}
                Err(e) => return Err(e).context("zlib"),
            }
            out
        }
        Compression::Lzma1 => {
            let mut out = Vec::with_capacity(expect as usize);
            let opts = lzma_rs::decompress::Options {
                unpacked_size: lzma_rs::decompress::UnpackedSize::UseProvided(Some(expect)),
                ..Default::default()
            };
            lzma_rs::lzma_decompress_with_options(&mut std::io::BufReader::new(src), &mut out, &opts)
                .map_err(|e| anyhow::anyhow!("lzma1: {e:?}"))?;
            out
        }
    };
    ensure!(out.len() as u64 == expect, "decompressed {} bytes, header says {expect}", out.len());
    Ok(out)
}

/// Files under `devices/` that carry a `.ddb` name but are not PDB
/// segments. Returns what they are, or None for a PDB candidate.
pub fn sniff_foreign(path: &std::path::Path) -> Result<Option<&'static str>> {
    let mut head = [0u8; 8];
    let n = std::fs::File::open(path)?.read(&mut head)?;
    Ok(match &head[..n.min(8)] {
        // `ac 0f 01 00 00 00 00 00` then a zlib stream: die_comb and
        // info_counts hold a "Family/device/part info - ASCII dump by DEV"
        // text; programming_sequence.pb.enc uses the same wrapper.
        [0xac, 0x0f, 0x01, 0, 0, 0, 0, 0] => Some("ac0f-wrapped zlib"),
        [0xac, 0xef, 0x01, 0, 0, 0, 0, 0] => Some("acef-wrapped, encrypted"),
        [0x02, 0, 0, 0, 0, 0, 0, 0] => Some("mdb (mapped database with SCHEMA table)"),
        _ => None,
    })
}
