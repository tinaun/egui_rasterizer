use egui::{epaint::ClippedShape, Color32, Context, ImageData, Pos2, Shape, TextureId};
use tiny_skia::{ClipMask, Color, PathBuilder, Pattern, Pixmap, PixmapMut};

use std::collections::HashMap;

pub fn rasterize(size: (u32, u32), ui: impl FnOnce(&Context)) -> Pixmap {
    let mut backend = TinySkiaBackend::new();

    let input = egui::RawInput {
        screen_rect: Some([Pos2::default(), Pos2::new(size.0 as f32, size.1 as f32)].into()),
        //pixels_per_point: Some(2.0),
        ..Default::default()
    };

    let output = backend.output_to_pixmap(input, ui);

    // while output.1 {
    //     output = self.output_to_pixmap(input, ui);
    // }

    output.2
}

pub struct TinySkiaBackend {
    ctx: Context,
    textures: HashMap<TextureId, Pixmap>,
    clip_mask: ClipMask,
}

impl TinySkiaBackend {
    pub fn new() -> Self {
        Self {
            ctx: Context::default(),
            textures: HashMap::new(),
            clip_mask: ClipMask::new(),
        }
    }

    pub fn context(&self) -> &Context {
        &self.ctx
    }

    pub fn output_to_pixmap(
        &mut self,
        input: egui::RawInput,
        ui: impl FnOnce(&Context),
    ) -> (egui::output::PlatformOutput, bool, Pixmap) {
        use egui::output::FullOutput;

        let rect = input
            .screen_rect
            .unwrap_or_else(|| self.ctx.input().screen_rect);
        let scale = input
            .pixels_per_point
            .unwrap_or_else(|| self.ctx.input().pixels_per_point);

        let mut pixmap =
            Pixmap::new((rect.max.x * scale) as u32, (rect.max.y * scale) as u32).unwrap();

        let FullOutput {
            platform_output,
            needs_repaint,
            textures_delta,
            shapes,
        } = self.ctx.run(input, ui);

        for (id, tex) in textures_delta.set {
            let pixmap = data_to_pixmap(&tex.image);

            if let Some(pos) = tex.pos {
                let texture = self.textures.get_mut(&id).unwrap();
                texture.draw_pixmap(
                    pos[0] as i32,
                    pos[1] as i32,
                    pixmap.as_ref(),
                    &Default::default(),
                    tiny_skia::Transform::identity(),
                    None,
                );
            } else {
                self.textures.insert(id, pixmap);
            }
        }

        for shape in shapes {
            // TODO: set clip rect
            self.draw_shape(&mut pixmap.as_mut(), shape, scale)
        }

        for id in textures_delta.free {
            self.textures.remove(&id);
        }

        (platform_output, needs_repaint, pixmap)
    }

    fn draw_shape(&mut self, pixmap: &mut PixmapMut, shape: ClippedShape, scale: f32) {
        let mut clip = PathBuilder::new();
        clip.push_rect(
            shape.0.left(),
            shape.0.top(),
            shape.0.width(),
            shape.0.height(),
        );
        if let Some(clip) = clip.finish() {
            //println!("{:?}", shape.0);
            self.clip_mask.set_path(
                pixmap.width(),
                pixmap.height(),
                &clip,
                Default::default(),
                false,
            );
        }

        match shape.1 {
            Shape::Noop => {}
            Shape::Vec(v) => {
                for inner_shape in v {
                    self.draw_shape(pixmap, ClippedShape(shape.0, inner_shape), scale);
                }
            }
            Shape::Mesh(mesh) => {
                // TODO
                // println!("skipping mesh... ({} vertices)", mesh.vertices.len());
                let mut tris = mesh.vertices.windows(3);
                while let Some(&[a, b, c]) = tris.next() {
                    let mut path = PathBuilder::new();
                    path.move_to(a.pos.x, a.pos.y);
                    path.line_to(b.pos.x, b.pos.y);
                    path.line_to(c.pos.x, c.pos.y);
                    path.close();
                    draw_path(
                        &mut self.clip_mask,
                        pixmap,
                        path,
                        Some(a.color),
                        Some(egui::Stroke {
                            width: 0.5,
                            color: a.color,
                        }),
                    );
                }
            }
            Shape::Rect(rect) => {
                let mut path = PathBuilder::new();
                if rect.rounding == egui::epaint::Rounding::default() {
                    path.push_rect(
                        rect.rect.left(),
                        rect.rect.top(),
                        rect.rect.width(),
                        rect.rect.height(),
                    );
                } else {
                    let r = rect.rounding;
                    let rect = rect.rect;

                    path.move_to(rect.left(), rect.top() + r.nw);
                    path.quad_to(rect.left(), rect.top(), rect.left() + r.nw, rect.top());
                    path.line_to(rect.right() - r.ne, rect.top());
                    path.quad_to(rect.right(), rect.top(), rect.right(), rect.top() + r.ne);
                    path.line_to(rect.right(), rect.bottom() - r.se);
                    path.quad_to(
                        rect.right(),
                        rect.bottom(),
                        rect.right() - r.se,
                        rect.bottom(),
                    );
                    path.line_to(rect.left() + r.sw, rect.bottom());
                    path.quad_to(
                        rect.left(),
                        rect.bottom(),
                        rect.left(),
                        rect.bottom() - r.sw,
                    );
                    path.close();
                }

                draw_path(
                    &mut self.clip_mask,
                    pixmap,
                    path,
                    Some(rect.fill),
                    Some(rect.stroke),
                );
            }
            Shape::LineSegment { points, stroke } => {
                let mut path = PathBuilder::new();
                path.move_to(points[0].x, points[0].y);
                path.line_to(points[1].x, points[1].y);

                draw_path(&mut self.clip_mask, pixmap, path, None, Some(stroke));
            }
            Shape::Circle(circle) => {
                let mut path = PathBuilder::new();
                path.push_oval(
                    tiny_skia::Rect::from_ltrb(
                        circle.center.x - circle.radius,
                        circle.center.y - circle.radius,
                        circle.center.x + circle.radius,
                        circle.center.y + circle.radius,
                    )
                    .unwrap(),
                );

                draw_path(
                    &mut self.clip_mask,
                    pixmap,
                    path,
                    Some(circle.fill),
                    Some(circle.stroke),
                );
            }
            Shape::Path(path_shape) => {
                if path_shape.points.is_empty() {
                    return;
                }
                let mut path = PathBuilder::new();
                path.move_to(path_shape.points[0].x, path_shape.points[0].y);
                for p in &path_shape.points[1..] {
                    path.line_to(p.x, p.y);
                }
                if path_shape.closed {
                    path.close();
                }

                draw_path(
                    &mut self.clip_mask,
                    pixmap,
                    path,
                    Some(path_shape.fill),
                    Some(path_shape.stroke),
                );
            }
            Shape::Text(ts) => {
                let font_pixmap = self.textures.get(&TextureId::Managed(0)).unwrap();

                //println!("{:?}", ts.pos);
                let origin = ts.pos;

                for row in &ts.galley.rows {
                    //println!("row: {:?}", row.rect);

                    for g in &row.glyphs {
                        let mut path = PathBuilder::new();
                        //println!("- glyph {} {:?} {:?} {:?}", g.chr, g.pos, g.size, g.uv_rect);
                        path.push_rect(
                            origin.x + g.pos.x + g.uv_rect.offset.x + 0.1,
                            origin.y + g.pos.y + g.uv_rect.offset.y - 0.1,
                            g.uv_rect.size.x,
                            g.uv_rect.size.y,
                        );

                        let path = path.finish().unwrap();

                        let uv = tiny_skia::IntRect::from_ltrb(
                            g.uv_rect.min[0] as i32,
                            g.uv_rect.min[1] as i32,
                            g.uv_rect.max[0] as i32,
                            g.uv_rect.max[1] as i32,
                        );
                        if uv.is_none() {
                            continue;
                        }
                        let uv = uv.unwrap();
                        let mut glyph_pixmap = font_pixmap.clone_rect(uv).unwrap();
                        let format = &ts.galley.job.sections[g.section_index as usize].format;
                        let color = if let Some(color) = ts.override_text_color {
                            color
                        } else {
                            format.color
                        };

                        let rect = tiny_skia::Rect::from_xywh(
                            0.0,
                            0.0,
                            glyph_pixmap.width() as f32,
                            glyph_pixmap.height() as f32,
                        )
                        .unwrap();

                        glyph_pixmap
                            .fill_rect(
                                rect,
                                &tiny_skia::Paint {
                                    shader: tiny_skia::Shader::SolidColor(Color::from_rgba8(
                                        color[0], color[1], color[2], color[3],
                                    )),
                                    blend_mode: tiny_skia::BlendMode::SourceAtop,
                                    ..Default::default()
                                },
                                tiny_skia::Transform::identity(),
                                None,
                            )
                            .unwrap();

                        let fill_shader = Pattern::new(
                            glyph_pixmap.as_ref(),
                            tiny_skia::SpreadMode::Pad,
                            tiny_skia::FilterQuality::Bilinear,
                            1.0,
                            tiny_skia::Transform::from_translate(
                                origin.x + g.pos.x + g.uv_rect.offset.x,
                                origin.y + g.pos.y + g.uv_rect.offset.y,
                            )
                            .pre_scale(1.0 / scale, 1.0 / scale),
                        );

                        pixmap
                            .fill_path(
                                &path,
                                &tiny_skia::Paint {
                                    shader: fill_shader,
                                    anti_alias: true,
                                    force_hq_pipeline: true,
                                    ..Default::default()
                                },
                                tiny_skia::FillRule::EvenOdd,
                                tiny_skia::Transform::identity(),
                                Some(&self.clip_mask),
                            )
                            .unwrap_or_default();
                    }
                }
            }
            Shape::QuadraticBezier(_) => todo!(),
            Shape::CubicBezier(_) => todo!(),
            Shape::Callback(_) => todo!(),
        }
    }
}

fn data_to_pixmap(data: &ImageData) -> Pixmap {
    let mut image_data: Vec<u8> = match data {
        ImageData::Color(c) => c
            .pixels
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], c[3]])
            .collect(),
        ImageData::Font(f) => f
            .srgba_pixels(1.0 / 2.2)
            .flat_map(|c| [c[0], c[1], c[2], c[3]])
            .collect(),
    };

    PixmapMut::from_bytes(&mut image_data, data.width() as u32, data.height() as u32)
        .unwrap()
        .to_owned()
}

fn draw_path(
    mask: &mut ClipMask,
    pixmap: &mut PixmapMut,
    path: PathBuilder,
    fill: Option<Color32>,
    stroke: Option<egui::epaint::Stroke>,
) {
    let path = path.finish().unwrap();

    if let Some(fill) = fill {
        let fill_shader =
            tiny_skia::Shader::SolidColor(Color::from_rgba8(fill[0], fill[1], fill[2], fill[3]));

        pixmap.fill_path(
            &path,
            &tiny_skia::Paint {
                shader: fill_shader,
                anti_alias: true,
                ..Default::default()
            },
            tiny_skia::FillRule::EvenOdd,
            tiny_skia::Transform::identity(),
            Some(&mask),
        );
        //.unwrap();
    }

    if let Some(stroke) = stroke {
        let stroke_color = stroke.color;
        let stroke_shader = tiny_skia::Shader::SolidColor(Color::from_rgba8(
            stroke_color[0],
            stroke_color[1],
            stroke_color[2],
            255,
        ));

        let sw = if stroke.width == 0.0 {
            0.0001
        } else {
            stroke.width
        };

        //println!("{:?}", path);

        pixmap.stroke_path(
            &path,
            &tiny_skia::Paint {
                shader: stroke_shader,
                anti_alias: true,
                ..Default::default()
            },
            &tiny_skia::Stroke {
                width: sw,
                ..Default::default()
            },
            tiny_skia::Transform::identity(),
            Some(&mask),
        );
        //.unwrap();
    }
}




