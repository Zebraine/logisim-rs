use super::*;
#[test]
fn unchanged_snapshots_do_not_upload_and_rotation_preserves_uvs() {
    let ctx = egui::Context::default();
    let mut image = Arc::new(Image {
        revision: 0,
        width: 2,
        height: 1,
        bounds: [[0, 0], [20, 10]],
        pixels: vec![255, 0, 0, 255, 0, 255, 0, 255],
    });
    let mut c = Component::new(logisim_core::Kind::Plugin, Point::new(100, 100));
    let canvas = Canvas::default();
    let mut texture_id = None;
    for (i, facing) in logisim_core::Facing::ALL.into_iter().enumerate() {
        c.facing = facing;
        let mut out = ctx.run_ui(Default::default(), |ui| {
            draw(ui.painter(), &canvas, &c, &image, false);
            texture_id = Some(texture(ui.ctx(), c.id, &image).id());
        });
        let uploads = out
            .textures_delta
            .set
            .iter()
            .filter(|(id, _)| Some(**id) == texture_id)
            .count();
        assert_eq!(uploads, usize::from(i == 0));
        let mesh = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Mesh(m) if Some(m.texture_id) == texture_id => Some(m),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            mesh.vertices[1].pos,
            canvas.screen(c.position + c.facing.transform(Point::new(20, 0)))
        );
        assert_eq!(mesh.vertices[1].uv, egui::pos2(1., 0.));
        out.textures_delta.clear();
    }
    Arc::make_mut(&mut image).pixels[0] = 0;
    let mut out = ctx.run_ui(Default::default(), |ui| {
        draw(ui.painter(), &canvas, &c, &image, false);
    });
    assert_eq!(
        out.textures_delta
            .set
            .iter()
            .filter(|(id, _)| Some(**id) == texture_id)
            .count(),
        1
    );
    out.textures_delta.clear();
}
