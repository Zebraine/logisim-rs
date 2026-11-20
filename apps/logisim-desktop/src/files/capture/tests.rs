use super::*;
use eframe::egui::{Color32, pos2, vec2};

#[test]
fn crop_uses_physical_scissor_edges_and_viewport_origin_without_resampling() {
    let frame = ColorImage::new([12, 9], (0..108).map(Color32::from_gray).collect());
    let g = Geometry {
        viewport: Rect::from_min_size(pos2(3., 4.), vec2(8., 6.)),
        canvas: Rect::from_min_max(pos2(5.4, 5.6), pos2(14., 12.)),
        pixels_per_point: 1.5,
    };
    let crop = g.crop(&frame).unwrap();
    assert_eq!(crop.size, [8, 7]);
    assert_eq!(crop[(0, 0)], frame[(4, 2)]);
    assert_eq!(crop[(7, 6)], frame[(11, 8)]);
    assert!(
        Geometry {
            pixels_per_point: 2.,
            ..g
        }
        .crop(&frame)
        .is_err()
    );
    assert!(
        Geometry {
            canvas: Rect::NOTHING,
            ..g
        }
        .crop(&frame)
        .is_err()
    );
    let native = Geometry {
        viewport: Rect::from_min_size(pos2(0., 0.), vec2(12., 9.)),
        canvas: Rect::from_min_max(pos2(3., 2.), pos2(10., 8.)),
        pixels_per_point: 1.,
    };
    assert_eq!(
        native.crop(&frame).unwrap().pixels,
        frame.region_by_pixels([3, 2], [7, 6]).pixels
    );
}
