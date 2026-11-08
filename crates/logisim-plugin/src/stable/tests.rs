use super::*;
use logisim_plugin_api::{ABI_VERSION, MAX_STATE, OutputDependency, Port, Tool};

fn manifest(abi: u32) -> Manifest {
    Manifest {
        abi,
        id: "test.stable".into(),
        version: "1.0".into(),
        name: "Stable".into(),
        tools: vec![Tool {
            id: "test".into(),
            name: "Test".into(),
            bounds: [[0, 0], [20, 20]],
            ports: vec![Port {
                id: "out".into(),
                label: "Output".into(),
                at: [0, 0],
                width: 1,
                output: true,
            }],
            graphics: vec![],
            properties: vec![],
            actions: vec![],
        }],
    }
}
fn contract() -> StableContract {
    StableContract {
        outputs: vec![OutputDependency {
            port: 0,
            inputs: vec![],
        }],
        graphics_inputs: vec![],
        state_inputs: Some(vec![]),
    }
}
fn signal() -> Signal {
    Signal {
        width: 1,
        value: 1,
        unknown: 0,
        error: 0,
    }
}
fn outputs() -> StableOutputs {
    StableOutputs {
        outputs: vec![signal()],
        graphics: vec![],
    }
}

/// Real Wasm dispatch by JSON tag. Query mutates a private global; phase execution
/// traps if that query VM is reused. The execution VM also faults on a second output
/// call so the test can verify sticky faults and actual VM recreation on reset.
fn module(
    abi: u32,
    declared: Response,
    output: Response,
    sampled: Response,
    code: &str,
) -> Vec<u8> {
    let responses = [
        Response::Manifest(manifest(abi)),
        Response::Update(Update {
            outputs: vec![signal()],
            state: vec![],
            graphics: vec![],
        }),
        declared,
        output,
        sampled,
    ];
    let mut data = String::new();
    let mut packed = vec![];
    for (i, response) in responses.iter().enumerate() {
        let bytes = serde_json::to_vec(response).unwrap();
        let offset = 262144 + i * 300000;
        let text = bytes
            .iter()
            .map(|b| format!("\\{b:02x}"))
            .collect::<String>();
        data.push_str(&format!("(data (i32.const {offset}) \"{text}\")"));
        packed.push(((bytes.len() as u64) << 32) | offset as u64);
    }
    let [m, reset, c, o, s] = packed.as_slice() else {
        unreachable!()
    };
    wat::parse_str(format!(
        r#"(module (memory (export "memory") 32)
        (global $query (mut i32) (i32.const 0)) (global $calls (mut i32) (i32.const 0))
        {data}
        (func (export "logisim_abi") (result i32) i32.const {abi})
        (func (export "logisim_buffer") (param i32) (result i32) i32.const 0)
        (func (export "logisim_call") (param i32) (result i64)
            i32.const 9 i32.load8_u i32.const 100 i32.eq
            if (result i64) i64.const {m}
            else i32.const 9 i32.load8_u i32.const 101 i32.eq
                if (result i64) i64.const {reset}
                else i32.const 16 i32.load8_u i32.const 99 i32.eq
                    if (result i64) i32.const 1 global.set $query i64.const {c}
                    else global.get $query if unreachable end
                        i32.const 16 i32.load8_u i32.const 111 i32.eq
                        if (result i64) {code} i64.const {o}
                        else i64.const {s} end
                    end
                end
            end))"#
    ))
    .unwrap()
}
fn load(declared: Response, output: Response, sampled: Response, code: &str) -> Plugin {
    Plugin::load(&module(ABI_VERSION, declared, output, sampled, code)).unwrap()
}
#[test]
fn legacy_opt_out_and_bad_contracts_are_distinguished() {
    let p = Plugin::load(&module(
        1,
        Response::Error("must not query".into()),
        Response::StableOutputs(outputs()),
        Response::StableState(vec![]),
        "",
    ))
    .unwrap();
    assert!(
        p.instantiate_stable(&p.manifest().components()[0])
            .unwrap()
            .is_none()
    );
    let good = Response::StableOutputs(outputs());
    let sample = Response::StableState(vec![]);
    let p = load(
        Response::StableContract(None),
        good.clone(),
        sample.clone(),
        "",
    );
    assert!(
        p.instantiate_stable(&p.manifest().components()[0])
            .unwrap()
            .is_none()
    );
    let mut invalid = contract();
    invalid.outputs[0].inputs.push(0);
    for response in [
        Response::StableContract(Some(invalid)),
        Response::StableState(vec![]),
        Response::Error("unsupported mode".into()),
    ] {
        let p = load(response, good.clone(), sample.clone(), "");
        assert!(p.instantiate_stable(&p.manifest().components()[0]).is_err());
    }
    // No capability can bypass exact component matching.
    let p = load(Response::StableContract(Some(contract())), good, sample, "");
    let mut wrong = p.manifest().components().remove(0);
    wrong.version = "2".into();
    assert!(p.instantiate_stable(&wrong).is_err());
}
#[test]
fn phase_vms_are_isolated_faults_stick_and_reset_creates_fresh_vm() {
    let p = load(
        Response::StableContract(Some(contract())),
        Response::StableOutputs(outputs()),
        Response::StableState(vec![42]),
        "global.get $calls if unreachable end i32.const 1 global.set $calls",
    );
    let mut vm = p
        .instantiate_stable(&p.manifest().components()[0])
        .unwrap()
        .unwrap();
    assert_eq!(vm.outputs(&[signal()], &[]).unwrap(), outputs());
    assert_eq!(vm.sample(&[signal()], &[]).unwrap(), vec![42]);
    assert!(vm.outputs(&[signal()], &[]).is_err());
    assert!(vm.sample(&[signal()], &[]).is_err());
    vm.reset(&[signal()]).unwrap();
    assert_eq!(vm.outputs(&[signal()], &[]).unwrap(), outputs());
}
#[test]
fn phase_outputs_samples_and_requests_are_bounded_and_validated() {
    let mut bad_width = outputs();
    bad_width.outputs[0].width = 2;
    let mut bad_mask = outputs();
    bad_mask.outputs[0].unknown = 1;
    let mut bad_graphics = outputs();
    bad_graphics.graphics.push(logisim_plugin_api::Shape::Line {
        a: [0, 0],
        b: [5000, 0],
        color: [0; 4],
    });
    for response in [
        Response::StableOutputs(bad_width),
        Response::StableOutputs(bad_mask),
        Response::StableOutputs(bad_graphics),
        Response::StableOutputs(StableOutputs {
            outputs: vec![],
            graphics: vec![],
        }),
        Response::StableState(vec![]),
    ] {
        let p = load(
            Response::StableContract(Some(contract())),
            response,
            Response::StableState(vec![]),
            "",
        );
        assert!(
            p.instantiate_stable(&p.manifest().components()[0])
                .unwrap()
                .unwrap()
                .outputs(&[signal()], &[])
                .is_err()
        );
    }
    for response in [
        Response::StableState(vec![0; MAX_STATE + 1]),
        Response::StableOutputs(outputs()),
    ] {
        let p = load(
            Response::StableContract(Some(contract())),
            Response::StableOutputs(outputs()),
            response,
            "",
        );
        assert!(
            p.instantiate_stable(&p.manifest().components()[0])
                .unwrap()
                .unwrap()
                .sample(&[signal()], &[])
                .is_err()
        );
    }
    for code in ["unreachable", "(loop $forever br $forever)"] {
        let p = load(
            Response::StableContract(Some(contract())),
            Response::StableOutputs(outputs()),
            Response::StableState(vec![]),
            code,
        );
        assert!(
            p.instantiate_stable(&p.manifest().components()[0])
                .unwrap()
                .unwrap()
                .outputs(&[signal()], &[])
                .is_err()
        );
    }
    let p = load(
        Response::StableContract(Some(contract())),
        Response::StableOutputs(outputs()),
        Response::StableState(vec![]),
        "",
    );
    let c = &p.manifest().components()[0];
    assert!(
        p.instantiate_stable(c)
            .unwrap()
            .unwrap()
            .outputs(&[], &[])
            .is_err()
    );
    assert!(
        p.instantiate_stable(c)
            .unwrap()
            .unwrap()
            .sample(&[signal()], &vec![0; MAX_STATE + 1])
            .is_err()
    );
}
