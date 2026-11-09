use crate::{Bit, Simulator, Value};
use logisim_core::{Circuit, Component, Kind, MemoryInterface, Point};

#[test]
fn rom_image_and_select_edits_refresh_compiled_reads_and_preserve_document_snapshots() {
    for constant_address in [false, true] {
        let mut circuit = Circuit::default();
        let mut rom = Component::new(Kind::Rom, Point::new(300, 200));
        rom.memory.address_bits = 4;
        rom.memory.contents.set(0, 0xa5);
        let mut address = Component::new(
            if constant_address {
                Kind::Constant
            } else {
                Kind::Pin
            },
            rom.ports()[1],
        );
        address.bits = 4;
        address.constant = 0;
        circuit.add_component(address);
        let select = circuit.add_component(Component::new(Kind::Pin, rom.ports()[2]));
        let id = circuit.add_component(rom);
        let original = circuit.clone();
        let mut event = Simulator::default();
        let mut compiled = Simulator::default();
        let settle = |event: &mut Simulator, compiled: &mut Simulator, expected| {
            event.advance(32);
            compiled.settle_compiled().unwrap();
            assert_eq!(event.pending_count(), 0);
            assert_eq!(event.port_value(id, 0), Some(expected));
            assert_eq!(compiled.port_value(id, 0), Some(expected));
        };
        for sim in [&mut event, &mut compiled] {
            sim.synchronize(&circuit);
        }
        settle(&mut event, &mut compiled, Value::known(8, 0xa5));

        // Drawing and RAM-only attributes do not invalidate this ROM's pure plan.
        let mut edited = circuit.component(id).unwrap().clone();
        edited.label = "program".into();
        edited.memory.simple = false;
        edited.memory.interface = MemoryInterface::Separate;
        circuit.replace_component(edited.clone());
        assert!(!event.synchronize(&circuit));
        assert!(!compiled.synchronize(&circuit));
        settle(&mut event, &mut compiled, Value::known(8, 0xa5));

        edited.memory.contents.set(0, 0x5a);
        circuit.replace_component(edited.clone());
        assert!(event.synchronize(&circuit));
        assert!(compiled.synchronize(&circuit));
        settle(&mut event, &mut compiled, Value::known(8, 0x5a));
        assert_eq!(original.component(id).unwrap().memory.contents.get(0), 0xa5);

        edited.memory.select_high = true;
        circuit.replace_component(edited);
        assert!(event.synchronize(&circuit));
        assert!(compiled.synchronize(&circuit));
        settle(&mut event, &mut compiled, Value::filled(8, Bit::Unknown));
        for bit in [Bit::One, Bit::Unknown, Bit::Error] {
            event.set_input_value(select, Value::filled(1, bit));
            compiled.set_input_value(select, Value::filled(1, bit));
            settle(&mut event, &mut compiled, Value::known(8, 0x5a));
        }

        // Restoring a document snapshot refreshes both the image and polarity.
        assert!(event.synchronize(&original));
        assert!(compiled.synchronize(&original));
        event.reset();
        compiled.reset();
        settle(&mut event, &mut compiled, Value::known(8, 0xa5));
        assert_eq!(event.memory_contents(id), None);
        assert_eq!(compiled.memory_contents(id), None);
    }
}
