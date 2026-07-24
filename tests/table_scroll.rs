use eframe::egui;
use egui_extras::{Column, TableBuilder};

#[test]
fn wide_table_keeps_header_and_body_aligned_and_body_visible() {
    let mut header_x = None;
    let mut body_x = None;
    let mut body_positions_y = Vec::new();
    let mut rendered_rows = 0;

    egui::__run_test_ui(|ui| {
        ui.set_min_size(egui::vec2(700.0, 500.0));

        let mut table = TableBuilder::new(ui)
            .id_salt("wide_table_test")
            .horizontal_scroll_offset(240.0)
            .vertical_scroll_offset(120.0)
            .max_scroll_height(400.0)
            .column(Column::exact(180.0));
        for _ in 0..7 {
            table = table.column(Column::exact(180.0));
        }

        table
            .header(25.0, |mut row| {
                row.col(|ui| {
                    header_x = Some(ui.label("header").rect.min.x);
                });
                for _ in 0..7 {
                    row.col(|ui| {
                        ui.label("header");
                    });
                }
            })
            .body(|body| {
                body.rows(25.0, 500, |mut row| {
                    rendered_rows += 1;
                    row.col(|ui| {
                        let response = ui.label("body");
                        body_x.get_or_insert(response.rect.min.x);
                        body_positions_y.push(response.rect.min.y);
                    });
                    for _ in 0..7 {
                        row.col(|ui| {
                            ui.label("body");
                        });
                    }
                });
            });
    });

    assert!(rendered_rows > 0, "wide table rendered no body rows");
    let header_x = header_x.expect("header was not rendered");
    let body_x = body_x.expect("body was not rendered");
    assert!(
        header_x < -100.0,
        "horizontal offset moved the thumb but not the columns: x={header_x}"
    );
    assert!(
        (header_x - body_x).abs() < 1.0,
        "header/body misaligned: header={header_x}, body={body_x}"
    );
    assert!(
        body_positions_y.iter().any(|y| (25.0..500.0).contains(y)),
        "vertically scrolled body was painted outside the viewport: y={body_positions_y:?}"
    );
}

#[test]
fn stale_vertical_offset_does_not_blank_a_shorter_table() {
    let mut rendered_rows = 0;

    egui::__run_test_ui(|ui| {
        ui.set_min_size(egui::vec2(700.0, 500.0));

        let table_state_id = ui.id().with("stale_offset_test");
        let scroll_id =
            ui.make_persistent_id(egui::IdSalt::new(table_state_id.with("__scroll_area")));
        let mut scroll_state = egui::scroll_area::State::default();
        scroll_state.offset.y = 500_000.0;
        scroll_state.store(ui.ctx(), scroll_id);

        TableBuilder::new(ui)
            .id_salt("stale_offset_test")
            .max_scroll_height(400.0)
            .column(Column::exact(180.0))
            .body(|body| {
                body.rows(25.0, 20, |mut row| {
                    rendered_rows += 1;
                    row.col(|ui| {
                        ui.label("body");
                    });
                });
            });
    });

    assert!(
        rendered_rows > 0,
        "a stale vertical offset blanked the entire table"
    );
}

#[test]
fn wide_table_keeps_rendering_across_vertical_scroll_frames() {
    let ctx = egui::Context::default();
    let mut first_visible_rows = Vec::new();

    for offset_y in [0.0, 250.0, 2_500.0, 10_000.0] {
        let mut first_visible_row = None;
        let _ = ctx.run_ui(Default::default(), |ui| {
            ui.set_min_size(egui::vec2(700.0, 500.0));

            let mut table = TableBuilder::new(ui)
                .id_salt("vertical_frames_test")
                .vertical_scroll_offset(offset_y)
                .max_scroll_height(400.0);
            for _ in 0..8 {
                table = table.column(Column::exact(180.0));
            }
            table
                .header(25.0, |mut row| {
                    for _ in 0..8 {
                        row.col(|ui| {
                            ui.label("header");
                        });
                    }
                })
                .body(|body| {
                    body.rows(25.0, 15_180, |mut row| {
                        first_visible_row.get_or_insert(row.index());
                        for _ in 0..8 {
                            row.col(|ui| {
                                ui.label("body");
                            });
                        }
                    });
                });
        });
        first_visible_rows.push(first_visible_row);
    }

    assert!(
        first_visible_rows.iter().all(Option::is_some),
        "table vanished at vertical offsets: {first_visible_rows:?}"
    );
    assert!(
        first_visible_rows.windows(2).all(|pair| pair[1] > pair[0]),
        "vertical scrolling did not advance rows: {first_visible_rows:?}"
    );
}

#[test]
fn external_offset_directly_selects_the_rendered_row_window() {
    let mut first_rows = Vec::new();
    for offset in [0.0, 3_300.0, 330_000.0] {
        let mut first = None;
        egui::__run_test_ui(|ui| {
            ui.set_min_size(egui::vec2(700.0, 400.0));
            TableBuilder::new(ui)
                .max_scroll_height(400.0)
                .column(Column::exact(180.0))
                .body(|body| {
                    body.rows_at_offset(25.0, 15_180, offset, |mut row| {
                        let index = row.index();
                        first.get_or_insert(index);
                        row.col(|ui| {
                            ui.label(index.to_string());
                        });
                    });
                });
        });
        first_rows.push(first.unwrap());
    }
    assert_eq!(first_rows[0], 0);
    assert!(
        first_rows[1] >= 100,
        "unexpected row window: {first_rows:?}"
    );
    assert!(
        first_rows[2] >= 10_000,
        "unexpected row window: {first_rows:?}"
    );
}

#[test]
fn mouse_wheel_scroll_keeps_wide_table_visible() {
    fn render(ctx: &egui::Context, mut input: egui::RawInput) -> (Option<usize>, f32, Vec<f32>) {
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(700.0, 500.0),
        ));
        let mut first_visible_row = None;
        let mut header_y = f32::NAN;
        let mut body_positions_y = Vec::new();
        let _ = ctx.run_ui(input, |ui| {
            let mut table = TableBuilder::new(ui)
                .id_salt("mouse_wheel_test")
                .max_scroll_height(400.0);
            for _ in 0..8 {
                table = table.column(Column::exact(180.0));
            }
            table
                .header(25.0, |mut row| {
                    row.col(|ui| {
                        header_y = ui.label("header").rect.min.y;
                    });
                    for _ in 1..8 {
                        row.col(|ui| {
                            ui.label("header");
                        });
                    }
                })
                .body(|body| {
                    body.rows(25.0, 515, |mut row| {
                        first_visible_row.get_or_insert(row.index());
                        for _ in 0..8 {
                            row.col(|ui| {
                                body_positions_y.push(ui.label("body").rect.min.y);
                            });
                        }
                    });
                });
        });
        (first_visible_row, header_y, body_positions_y)
    }

    let ctx = egui::Context::default();
    let (initial, _, _) = render(&ctx, egui::RawInput::default());
    let initial = initial.expect("initial rows missing");
    let mut input = egui::RawInput::default();
    input
        .events
        .push(egui::Event::PointerMoved(egui::pos2(692.0, 200.0)));
    input.events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -300.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    let (scrolled, scrolled_header_y, scrolled_body_y) = render(&ctx, input.clone());
    let scrolled = scrolled.expect("rows vanished during wheel scroll");
    let mut repeated = scrolled;
    for _ in 0..4 {
        let (next, _, _) = render(&ctx, input.clone());
        let next = next.expect("rows vanished during repeated wheel scroll");
        assert!(
            next > repeated,
            "vertical scrolling stopped responding: previous={repeated}, next={next}"
        );
        repeated = next;
    }
    let (settled, settled_header_y, settled_body_y) = render(&ctx, egui::RawInput::default());
    let settled = settled.expect("rows vanished after wheel scroll");

    assert!(
        repeated > initial || settled > initial,
        "wheel did not advance rows: initial={initial}, scrolled={scrolled}, repeated={repeated}, settled={settled}"
    );
    assert!(
        (0.0..30.0).contains(&settled_header_y),
        "header left the visible viewport after vertical scrolling: during={scrolled_header_y}, settled={settled_header_y}"
    );
    assert!(
        settled_body_y.iter().any(|y| (25.0..500.0).contains(y)),
        "body left the visible viewport after vertical scrolling: during={scrolled_body_y:?}, settled={settled_body_y:?}"
    );
}

#[test]
fn dragging_vertical_handle_keeps_wide_table_visible() {
    fn render(
        ctx: &egui::Context,
        mut input: egui::RawInput,
    ) -> (Option<usize>, Option<f32>, Vec<f32>) {
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(700.0, 500.0),
        ));
        let mut first_visible_row = None;
        let mut header_y = None;
        let mut body_positions_y = Vec::new();
        let _ = ctx.run_ui(input, |ui| {
            let mut table = TableBuilder::new(ui)
                .id_salt("vertical_drag_test")
                .resizable(true)
                .max_scroll_height(400.0);
            for _ in 0..8 {
                // With the default 8-point spacing, the fourth resize handle
                // lands at x=692: directly over the vertical scrollbar used
                // below. It must not steal the scrollbar drag.
                table = table.column(Column::initial(166.0));
            }
            table
                .header(25.0, |mut row| {
                    row.col(|ui| {
                        header_y = Some(ui.label("header").rect.min.y);
                    });
                    for _ in 1..8 {
                        row.col(|ui| {
                            ui.label("header");
                        });
                    }
                })
                .body(|body| {
                    body.rows(25.0, 15_180, |mut row| {
                        first_visible_row.get_or_insert(row.index());
                        for _ in 0..8 {
                            row.col(|ui| {
                                body_positions_y.push(ui.label("body").rect.min.y);
                            });
                        }
                    });
                });
        });
        (first_visible_row, header_y, body_positions_y)
    }

    let ctx = egui::Context::default();
    let (initial, _, _) = render(&ctx, egui::RawInput::default());
    let initial = initial.expect("initial rows missing");

    let handle = egui::pos2(692.0, 30.0);
    let mut press = egui::RawInput::default();
    press.events.push(egui::Event::PointerMoved(handle));
    press.events.push(egui::Event::PointerButton {
        pos: handle,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    let _ = render(&ctx, press);

    let dragged_to = egui::pos2(692.0, 280.0);
    let mut drag = egui::RawInput::default();
    drag.events.push(egui::Event::PointerMoved(dragged_to));
    let _ = render(&ctx, drag);

    let mut release = egui::RawInput::default();
    release.events.push(egui::Event::PointerButton {
        pos: dragged_to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    let (settled, header_y, body_positions_y) = render(&ctx, release);
    let settled = settled.expect("rows vanished after vertical handle drag");

    assert!(
        settled > initial + 3_000,
        "vertical handle did not make a proportional jump: initial={initial}, settled={settled}"
    );
    assert!(
        header_y.is_some_and(|y| (0.0..30.0).contains(&y)),
        "header left the viewport after handle drag: {header_y:?}"
    );
    assert!(
        body_positions_y.iter().any(|y| (0.0..500.0).contains(y)),
        "body was painted outside the viewport after handle drag: {body_positions_y:?}"
    );
}
