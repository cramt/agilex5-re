use ddb::{container, pdb, pins, reader, schema, schemas};

use anyhow::{Context, Result};
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!(
        "usage:
  ddb info <file.ddb>                 header + footer class table as JSON
  ddb check <file.ddb>...             one line per file: OK/FAIL, sizes, root class
  ddb unpack <file.ddb> <dir>         write decompressed body / footer
  ddb trace <file.ddb> <schema>       decode with a schema, print the trace token
                                      stream (compare with verify/trace_tokens.py)
  ddb dump <file.ddb> <schema>        every decoded object as one JSON line
                                      (table, id, class, fields; pointers are ids)
  ddb pins <pkg.ddb> <pad.ddb>        package ball -> pad -> location, every slot, CSV"
    );
    std::process::exit(2)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else { usage() };
    match (cmd.as_str(), &args[1..]) {
        ("info", [f]) => {
            let d = container::Ddb::open(&PathBuf::from(f))?;
            let footer = pdb::parse_footer(&d.footer)?;
            let j = serde_json::json!({ "header": d.header, "footer": footer });
            println!("{}", serde_json::to_string_pretty(&j)?);
        }
        ("check", files) if !files.is_empty() => {
            let mut bad = 0;
            for f in files {
                if let Some(kind) = container::sniff_foreign(&PathBuf::from(f))? {
                    println!("SKIP {f} not a PDB file: {kind}");
                    continue;
                }
                let r = container::Ddb::open(&PathBuf::from(f)).and_then(|d| {
                    let ft = pdb::parse_footer(&d.footer).context("footer")?;
                    Ok((d.header, ft))
                });
                match r {
                    Ok((h, ft)) => println!(
                        "OK {f} {:?} body={} classes={} root={}",
                        h.compression,
                        h.size_of_body,
                        ft.classes.len(),
                        ft.root_type_name
                    ),
                    Err(e) => {
                        bad += 1;
                        println!("FAIL {f} {e:#}");
                    }
                }
            }
            if bad > 0 {
                anyhow::bail!("{bad} of {} files failed", files.len());
            }
        }
        ("unpack", [f, dir]) => {
            let d = container::Ddb::open(&PathBuf::from(f))?;
            let dir = PathBuf::from(dir);
            std::fs::create_dir_all(&dir)?;
            let stem = PathBuf::from(f).file_stem().unwrap().to_string_lossy().into_owned();
            std::fs::write(dir.join(format!("{stem}.body.bin")), &d.body)?;
            std::fs::write(dir.join(format!("{stem}.footer.bin")), &d.footer)?;
        }
        ("trace", [f, sch]) => {
            let d = container::Ddb::open(&PathBuf::from(f))?;
            let footer = pdb::parse_footer(&d.footer)?;
            let schema = schema::Schema::parse(&std::fs::read_to_string(sch)?)?;
            let mut out = std::io::BufWriter::new(std::io::stdout().lock());
            let r = reader::Reader::new(&d.body, &footer, &schema, Some(&mut out));
            let res = r.decode(&footer.root_type_name);
            drop(out);
            let dec = res?;
            eprintln!("decoded {} deferred objects, body fully consumed", dec.objects.len());
        }
        ("dump", [f, sch]) => {
            use std::io::Write;
            let d = container::Ddb::open(&PathBuf::from(f))?;
            let footer = pdb::parse_footer(&d.footer)?;
            let schema = schema::Schema::parse(&std::fs::read_to_string(sch)?)?;
            let dec = reader::Reader::new(&d.body, &footer, &schema, None).decode(&footer.root_type_name)?;
            let mut out = std::io::BufWriter::new(std::io::stdout().lock());
            writeln!(out, "{}", serde_json::json!({"table": footer.root_type_name, "id": 1, "class": dec.root.class, "fields": dec.root.to_json()}))?;
            let mut keys: Vec<_> = dec.objects.keys().cloned().collect();
            keys.sort();
            for k in keys {
                let o = &dec.objects[&k];
                writeln!(out, "{}", serde_json::json!({"table": k.0, "id": k.1, "class": o.class, "fields": o.to_json()}))?;
            }
        }
        ("pins", [pkg, pad]) => {
            let (spkg, spad) = (schema::Schema::parse(schemas::PKG)?, schema::Schema::parse(schemas::PAD)?);
            let pk = pins::decode(&PathBuf::from(pkg), &spkg)?;
            let pd = pins::decode(&PathBuf::from(pad), &spad)?;
            let mut out = std::io::BufWriter::new(std::io::stdout().lock());
            pins::write_csv(&mut out, &pins::pins(&pk)?, &pins::pads(&pd)?)?;
        }
        _ => usage(),
    }
    Ok(())
}
