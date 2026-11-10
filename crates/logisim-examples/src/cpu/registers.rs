use crate::builder::{Builder, E, hierarchy};
use logisim_core::Circuit;
pub(super) fn build() -> Circuit {
    let mut b = Builder::new("G16 registers / r0 is zero / two read ports, one write port");
    b.input("CLK", 1);
    let reset = b.input("RESET", 1);
    let enable = b.input("WE", 1);
    let dest = b.input("DEST", 3);
    let ra = b.input("RA", 3);
    let rb = b.input("RB", 3);
    let value = b.input("WDATA", 16);
    b.section("Writable registers / simultaneous old-value reads");
    let mut values = vec![E::num(16, 0)];
    for i in 1..8 {
        let name = format!("r{i}");
        b.register(&name, &value, &enable.and(&dest.is(i)), &reset);
        values.push(E::net(&name, 16));
    }
    b.section("Balanced read selection");
    b.output("A", &hierarchy::select(&ra, &values));
    b.output("B", &hierarchy::select(&rb, &values));
    b.finish()
}
