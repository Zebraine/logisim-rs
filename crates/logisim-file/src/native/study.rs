//! Release comparison on known, validated documents. All candidate byte buffers
//! decoded here are produced in memory by the corresponding serializer.
use super::*;
use binary::PositionalPart;
use serde_json::{Value, json};
use std::hint::black_box;
use std::time::Instant;

fn measure<T>(mut operation: impl FnMut() -> T, runs: usize) -> Value {
    for _ in 0..3 {
        black_box(operation());
    }
    let mut ns = Vec::with_capacity(runs);
    for _ in 0..runs {
        let start = Instant::now();
        let value = black_box(operation());
        ns.push(start.elapsed().as_nanos() as u64);
        drop(value);
    }
    ns.sort_unstable();
    json!({"p50_us": ns[(runs*50).div_ceil(100)-1] as f64/1000.,
           "p95_us": ns[(runs*95).div_ceil(100)-1] as f64/1000.})
}

fn compare<T>(
    p: &Project,
    runs: usize,
    header: usize,
    make: impl Fn(&Project) -> T,
    encode: impl Fn(&T) -> Vec<u8>,
    decode: impl Fn(&[u8]) -> T,
    open: impl Fn(T) -> Result<Project>,
) -> Result<Value> {
    let t = make(p);
    let bytes = encode(&t);
    let restored = open(decode(&bytes))?;
    if Native::encode(p)? != Native::encode(&restored)? || bytes != encode(&make(&restored)) {
        return Err(Error(
            "Candidate round trip changed the document or bytes".into(),
        ));
    }
    Ok(
        json!({"bytes":bytes.len()+header,"dto_parse":measure(||decode(&bytes),runs),
        "dto_write":measure(||encode(&t),runs),
        "open":measure(||open(decode(&bytes)).unwrap(),runs),
        "save":measure(||encode(&make(p)),runs)}),
    )
}

/// Compare codec and model costs independently. `runs` is the sample count per phase.
pub fn report(p: &Project, runs: usize) -> Result<Value> {
    if !(1..=10000).contains(&runs) {
        return Err(Error("Expected 1..10000 samples".into()));
    }
    let make = |p: &Project| Native::encode(p).unwrap();
    Native::encode(p)?;
    let json = compare(
        p,
        runs,
        0,
        make,
        |n| serde_json::to_vec(n).unwrap(),
        |b| serde_json::from_slice(b).unwrap(),
        Native::decode,
    )?;
    let cbor = compare(
        p,
        runs,
        6,
        make,
        |n| serde_cbor::ser::to_vec_packed(n).unwrap(),
        |b| serde_cbor::from_slice(b).unwrap(),
        Native::decode,
    )?;
    let postcard = compare(
        p,
        runs,
        6,
        |p| make(p).map_parts::<PositionalPart>(),
        |n| postcard::to_allocvec(n).unwrap(),
        |b| postcard::from_bytes::<Native<PositionalPart>>(b).unwrap(),
        |n| n.map_parts().decode(),
    )?;
    let xml = crate::serialize(p)?;
    let read_xml = || crate::parse(&xml).unwrap();
    let read = read_xml();
    if read.partial || !read.warnings.is_empty() {
        return Err(Error(format!("Incomplete XML: {:?}", read.warnings)));
    }
    verify_export(p, &read.document)?;
    Ok(json!({"compact_json":json,"cbor":cbor,"postcard":postcard,
        "circ":{"bytes":xml.len(),"open":measure(read_xml,runs),"save":measure(||crate::serialize(p).unwrap(),runs)}}))
}
