//! Replace texture chunks in a DAT with new pixels, at any size.
//!
//! usage: retexture <in.DAT> <out.DAT> <name>=<file.rgba>:<W>x<H> ...
//!
//! `name` is the texture's 16-byte name with spaces as stored, trimmed (e.g.
//! `tim     hm_b61_3` -> `hm_b61_3`, matched on the last word). Each `.rgba`
//! file is raw 8-bit RGBA, alpha as the texture stores it. The chunk keeps its
//! header fields except name, size and data size; every other chunk is copied
//! byte for byte.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::process::ExitCode;

use ffxi_modder::chunk::{self, Chunk};
use ffxi_modder::kind::ChunkKind;
use ffxi_modder::texture;
use ffxi_modder::texture_write::{self, TEXTURE_NAME_BYTES};

const TEXTURE_NAME_AT: usize = 0x01;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: {} <in.DAT> <out.DAT> <name>=<file.rgba>:<W>x<H> ...", args[0]);
        return ExitCode::from(2);
    }
    match run(&args[1], &args[2], &args[3..]) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(input: &str, output: &str, specs: &[String]) -> Result<(), String> {
    let mut wanted: HashMap<String, (Vec<u8>, u32, u32)> = HashMap::new();
    for s in specs {
        let (name, rest) = s.split_once('=').ok_or(format!("bad spec {s}"))?;
        let (path, dims) = rest.rsplit_once(':').ok_or(format!("bad spec {s}"))?;
        let (w, h) = dims.split_once('x').ok_or(format!("bad size in {s}"))?;
        let (w, h): (u32, u32) = (w.parse().map_err(|_| "bad width")?, h.parse().map_err(|_| "bad height")?);
        let px = fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        if px.len() != (w * h * 4) as usize {
            return Err(format!("{path}: {} bytes, expected {}", px.len(), w * h * 4));
        }
        wanted.insert(name.to_string(), (px, w, h));
    }
    let bytes = fs::read(input).map_err(|e| e.to_string())?;
    let all: Vec<Chunk> = chunk::walk(&bytes).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    let end = |c: &Chunk| c.offset + chunk::CHUNK_HEADER_BYTES + c.data.len();
    let mut out = Vec::with_capacity(bytes.len());
    let mut done = 0;
    for c in &all {
        let name = (c.kind == ChunkKind::Img as u8 && c.data.len() > TEXTURE_NAME_AT + TEXTURE_NAME_BYTES)
            .then(|| String::from_utf8_lossy(&c.data[TEXTURE_NAME_AT..TEXTURE_NAME_AT + TEXTURE_NAME_BYTES]).into_owned());
        let key = name.as_ref().and_then(|n| n.split_whitespace().last().map(str::to_string));
        match key.and_then(|k| wanted.get(&k).map(|v| (k, v))) {
            Some((k, (px, w, h))) => {
                let raw: [u8; TEXTURE_NAME_BYTES] = c.data[TEXTURE_NAME_AT..TEXTURE_NAME_AT + TEXTURE_NAME_BYTES]
                    .try_into()
                    .expect("16 bytes");
                let body = texture_write::encode_dxt3(c.data, raw, *w, *h, px).map_err(|e| format!("{k}: {e}"))?;
                let back = texture::decode_texture(&body).map_err(|e| format!("{k} re-read: {e}"))?;
                if (back.width, back.height) != (*w, *h) {
                    return Err(format!("{k} re-read at the wrong size"));
                }
                out.extend(chunk::reencode(&bytes, c, &body));
                println!("{k}: {}x{} -> {w}x{h}", u32::from_le_bytes(c.data[0x15..0x19].try_into().unwrap()), u32::from_le_bytes(c.data[0x19..0x1d].try_into().unwrap()));
                done += 1;
            }
            None => out.extend_from_slice(&bytes[c.offset..end(c)]),
        }
    }
    out.extend_from_slice(&bytes[all.last().map_or(0, end)..]);
    if done != wanted.len() {
        return Err(format!("matched {done} of {} textures", wanted.len()));
    }
    let check: Vec<Chunk> = chunk::walk(&out).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    if check.len() != all.len() || check.last().map(end) != Some(out.len() - (bytes.len() - all.last().map_or(0, end))) {
        return Err("output does not walk like the input".into());
    }
    fs::write(output, &out).map_err(|e| e.to_string())?;
    println!("wrote {output} ({} -> {} bytes)", bytes.len(), out.len());
    Ok(())
}
