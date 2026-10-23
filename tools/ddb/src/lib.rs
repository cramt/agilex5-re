//! Quartus Prime Pro (PDB v25) `.ddb` device database reader, for the
//! Agilex 5 die sm4revb. Forked from cyclone2-re's `tools/ddb`.
pub mod container;
pub mod pdb;
pub mod reader;
pub mod schema;
pub mod pins;

/// The checked schemas, built in.
pub mod schemas {
    pub const PKG: &str = include_str!("../schema/pkg.schema");
    pub const PAD: &str = include_str!("../schema/pad.schema");
}
