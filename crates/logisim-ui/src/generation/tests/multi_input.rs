use super::support::click;
use crate::generation::*;
use crate::{Language, i18n};
use logisim_core::{Gate, Kind};

#[test]
fn fanin_options_are_localized_captured_by_build_and_undoable() {
    for language in Language::ALL {
        for two_input_only in [false, true] {
            for nand_only in [false, true] {
                let ctx = egui::Context::default();
                crate::configure(&ctx);
                let mut w = Workspace::default();
                w.set_language(language);
                for name in ["C", "D", "E", "F", "G", "H"] {
                    w.generator.analysis.add_input(name.into()).unwrap();
                }
                w.generator
                    .analysis
                    .apply_expression(0, "A & B & C & D & E & F & G & H")
                    .unwrap();
                w.generator.refresh();
                w.open_generator();
                let label = |key: &str| i18n::with_language(language, || tr(key).to_owned());
                if two_input_only {
                    click(&ctx, &mut w, &label("Use two-input gates only"));
                }
                if nand_only {
                    click(&ctx, &mut w, &label("Use NAND gates only"));
                }
                let before = w.project().clone();
                w.generator.request_build();
                let request = w.take_generation_request().unwrap();
                crate::tests::support::widgets::click(
                    &ctx,
                    &mut w,
                    &label("Use two-input gates only"),
                );
                crate::tests::support::widgets::click(&ctx, &mut w, &label("Use NAND gates only"));
                assert_eq!(w.generator.two_input_only, two_input_only);
                assert_eq!(w.generator.nand_only, nand_only);
                // In-flight requests own their configuration, independently of the draft.
                w.generator.two_input_only = !two_input_only;
                w.generator.nand_only = !nand_only;
                w.receive_generation(request.serial, request.execute(|| false));
                assert!(!w.generator.open);
                let built = w.circuit().clone();
                let gates: Vec<_> = built
                    .components
                    .iter()
                    .filter(|c| matches!(c.kind, Kind::Gate(_)))
                    .collect();
                assert_eq!(
                    gates.len(),
                    (if two_input_only { 7 } else { 1 }) * (if nand_only { 2 } else { 1 })
                );
                assert!(
                    gates
                        .iter()
                        .all(|c| c.kind
                            == Kind::Gate(if nand_only { Gate::Nand } else { Gate::And }))
                );
                assert_eq!(
                    gates.iter().map(|c| c.input_count()).max(),
                    Some(if two_input_only { 2 } else { 8 })
                );
                w.editor.undo();
                assert_eq!(w.project(), &before);
                w.editor.redo();
                assert_eq!(w.circuit(), &built);
            }
        }
    }
}
