use crate::*;
use logisim_core::{Gate, Kind, Point};

#[test]
fn generated_circuits_save_as_ordinary_components_and_preserve_geometry() {
    use logisim_core::generation::{Analysis, Cell, GateBasis};
    let mut a = Analysis::default();
    for name in ["A", "B", "Cin"] {
        a.add_input(name.into()).unwrap();
    }
    for name in ["Sum", "Carry", "Enable"] {
        a.add_output(name.into()).unwrap();
    }
    a.apply_expression(0, "A ^ B ^ Cin").unwrap();
    a.apply_expression(1, "A & B | Cin & (A ^ B)").unwrap();
    for row in 0..8 {
        a.set_cell(
            2,
            row,
            if row == 0 {
                Cell::Zero
            } else if row >= 4 {
                Cell::One
            } else {
                Cell::DontCare
            },
        )
        .unwrap();
    }
    for basis in [GateBasis::Standard, GateBasis::Nand] {
        let circuit = a.generate_with_basis(basis).unwrap();
        let mut document = Document::default();
        let id = document.add("adder".into()).unwrap();
        document.circuit_mut(id).unwrap().circuit = circuit.clone();
        document.refresh_interfaces(false);
        let instance = document.instance(id, Point::new(400, 300)).unwrap();
        let expected_ports = instance.ports();
        document.main_circuit_mut().circuit.add_component(instance);
        let xml = serialize(&document).unwrap();
        let parsed = parse(&xml).unwrap();
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(serialize(&parsed.document).unwrap(), xml);
        let loaded = &parsed.document.named("adder").unwrap().circuit;
        assert_eq!(loaded.components.len(), circuit.components.len());
        for (original, restored) in circuit.components.iter().zip(&loaded.components) {
            assert_eq!(original.kind, restored.kind);
            assert_eq!(original.ports(), restored.ports());
            assert_eq!(original.label, restored.label);
        }
        assert_eq!(
            circuit.wires.iter().map(|w| (w.a, w.b)).collect::<Vec<_>>(),
            loaded.wires.iter().map(|w| (w.a, w.b)).collect::<Vec<_>>()
        );
        assert_eq!(
            parsed.document.main_circuit().circuit.components[0].ports(),
            expected_ports
        );
    }
}

#[test]
fn expression_only_generation_saves_thirty_two_inputs_without_analysis_metadata() {
    use logisim_core::generation::Analysis;
    let mut a = Analysis::expressions();
    for i in 0..32 {
        a.add_input(format!("I{i}")).unwrap();
    }
    a.add_output("Y".into()).unwrap();
    a.apply_expression(0, &a.inputs().join(" ^ ")).unwrap();
    let document = Document::single("xor32", a.generate().unwrap());
    let xml = serialize(&document).unwrap();
    let loaded = parse(&xml).unwrap();
    assert!(loaded.warnings.is_empty());
    assert_eq!(serialize(&loaded.document).unwrap(), xml);
    let circuit = &loaded.document.main_circuit().circuit;
    assert_eq!(
        circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Pin && !c.output)
            .count(),
        32
    );
    assert_eq!(
        circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Gate(Gate::Xor))
            .count(),
        31
    );
}

#[test]
fn generated_multi_input_gates_roundtrip_with_parity_and_port_geometry() {
    use logisim_core::generation::{Analysis, GateBasis, GenerationOptions};
    let mut a = Analysis::expressions();
    for i in 0..32 {
        a.add_input(format!("I{i}")).unwrap();
    }
    for (name, operator) in [("Product", " & "), ("Sum", " | "), ("Parity", " ^ ")] {
        a.add_output(name.into()).unwrap();
        a.apply_expression(a.outputs().len() - 1, &a.inputs().join(operator))
            .unwrap();
    }
    for basis in [GateBasis::Standard, GateBasis::Nand] {
        let c = a
            .generate_with_options(GenerationOptions {
                basis,
                two_input_only: false,
            })
            .unwrap();
        let document = Document::single("multi", c);
        let xml = serialize(&document).unwrap();
        assert!(xml.contains("name=\"inputs\" val=\"8\""));
        if basis == GateBasis::Standard {
            assert!(xml.contains("name=\"xor\" val=\"odd\""));
        }
        let loaded = parse(&xml).unwrap();
        assert!(loaded.warnings.is_empty());
        assert_eq!(serialize(&loaded.document).unwrap(), xml);
        let geometry = |p: &Document| {
            p.main_circuit()
                .circuit
                .components
                .iter()
                .map(|c| (c.kind, c.ports(), c.gate_options))
                .collect::<Vec<_>>()
        };
        assert_eq!(geometry(&loaded.document), geometry(&document));
    }
}
