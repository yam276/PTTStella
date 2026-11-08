//! Explicit fixture-only verification of the actual native viewport component.
use super::ArticleView;
use crate::{
    TerminalWindow,
    verification::{self, Verification, require, wait},
};
use slint::{
    ComponentHandle, Model,
    platform::{Key, PointerEventButton as Button, WindowEvent as Event},
};
use std::{error::Error, fs::OpenOptions, path::PathBuf, rc::Rc, time::Duration};

pub fn start(
    ui: &TerminalWindow,
    owner: Rc<ArticleView>,
    directory: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(
        ui,
        directory,
        "article-viewport",
        move |ui, directory| async move {
            wait(
                || owner.painted.borrow().is_some(),
                "article first raster missing",
            )
            .await?;
            require(owner.rows.get() == 837, "article fixture lost rows")?;
            ui.invoke_focus_article();
            tokio::time::sleep(Duration::from_millis(30)).await;
            verification::capture(&ui, &directory, "top.png")?;
            let layout = owner.layout.get().ok_or("article layout missing")?;
            let point = slint::LogicalPosition::new(
                ui.get_terminal_origin_x() + layout.view_width / 2.,
                ui.get_terminal_origin_y() + layout.body_height / 2.,
            );
            ui.window().dispatch_event(Event::PointerScrolled {
                position: point,
                delta_x: 0.,
                delta_y: -3. * layout.line_height(),
            });
            require(
                (owner.anchor.get() - 3.).abs() < 0.001,
                "native wheel missed article body",
            )?;
            verification::key(&ui, Key::Home);
            let track = slint::LogicalPosition::new(
                ui.get_terminal_origin_x() + layout.view_width + 6.,
                ui.get_terminal_origin_y() + layout.body_height / 2.,
            );
            verification::click(&ui, track, Button::Right);
            require(
                owner.anchor.get() == 0.,
                "right button moved article scrollbar",
            )?;
            verification::click(&ui, track, Button::Left);
            require(
                owner.anchor.get() > 100.,
                "native vertical scrollbar did not move",
            )?;
            let before = owner.anchor.get();
            let thumb = (layout.body_height * layout.body_height
                / (owner.rows.get() as f32 * layout.line_height()))
            .max(24.)
            .min(layout.body_height);
            let top = (layout.body_height - thumb) * before as f32 / layout.maximum() as f32;
            let grab = slint::LogicalPosition::new(track.x, ui.get_terminal_origin_y() + top + 5.);
            ui.window().dispatch_event(Event::PointerPressed {
                position: grab,
                button: Button::Left,
            });
            require(
                (owner.anchor.get() - before).abs() < 0.001,
                "thumb press lost original grab offset",
            )?;
            let to = slint::LogicalPosition::new(grab.x, grab.y + 40.);
            ui.window()
                .dispatch_event(Event::PointerMoved { position: to });
            ui.window().dispatch_event(Event::PointerReleased {
                position: to,
                button: Button::Left,
            });
            let expected_drag =
                before + 40. / f64::from(layout.body_height - thumb) * layout.maximum();
            require(
                (owner.anchor.get() - expected_drag).abs() < 0.01,
                "thumb drag did not keep grab offset",
            )?;
            let held = owner.anchor.get();
            let top = (layout.body_height - thumb) * held as f32 / layout.maximum() as f32;
            let held_position =
                slint::LogicalPosition::new(track.x, ui.get_terminal_origin_y() + top + 5.);
            ui.window().dispatch_event(Event::PointerPressed {
                position: held_position,
                button: Button::Left,
            });
            let size = ui.window().size();
            let dpi = ui.window().scale_factor();
            ui.window().set_size(slint::LogicalSize::new(
                size.width as f32 / dpi + 4.,
                size.height as f32 / dpi,
            ));
            wait(
                || {
                    owner
                        .layout
                        .get()
                        .is_some_and(|l| l.view_width > layout.view_width)
                },
                "width-only resize did not reach viewport",
            )
            .await?;
            require(
                owner
                    .layout
                    .get()
                    .is_some_and(|l| l.row == layout.row && l.body_height == layout.body_height),
                "resize fixture changed row raster geometry",
            )?;
            let shifted = slint::LogicalPosition::new(held_position.x, held_position.y + 40.);
            ui.window()
                .dispatch_event(Event::PointerMoved { position: shifted });
            ui.window().dispatch_event(Event::PointerReleased {
                position: shifted,
                button: Button::Left,
            });
            require(
                (owner.anchor.get() - held).abs() < 0.01,
                "width-only resize retained old scrollbar grab",
            )?;
            verification::key(&ui, Key::Home);
            let layout = owner.layout.get().ok_or("resized initial layout missing")?;
            let expected = layout.page(0., true);
            verification::key(&ui, Key::PageDown);
            wait(
                || (owner.anchor.get() - expected).abs() < 0.001,
                "PageDown did not preserve last full line",
            )
            .await?;
            verification::key(&ui, Key::Home);
            wait(|| owner.anchor.get() == 0., "Home did not reach first row").await?;
            ui.invoke_article_scroll(-650.25 * layout.line_height(), false);
            wait(
                || {
                    owner
                        .painted
                        .borrow()
                        .as_ref()
                        .is_some_and(|p| p.rows.start == 648)
                },
                "historical visible window missing",
            )
            .await?;
            tokio::time::sleep(Duration::from_millis(30)).await;
            verification::capture(&ui, &directory, "historical-ink.png")?;
            let rows_before = owner.row_raster_count.get();
            verification::key(&ui, Key::DownArrow);
            wait(
                || {
                    owner
                        .painted
                        .borrow()
                        .as_ref()
                        .is_some_and(|p| p.rows.start == 649)
                },
                "row-down raster did not advance",
            )
            .await?;
            require(
                owner.row_raster_count.get() == rows_before + 1,
                "one-row scroll rerasterized retained rows",
            )?;
            verification::key(&ui, Key::UpArrow);
            wait(
                || {
                    owner
                        .painted
                        .borrow()
                        .as_ref()
                        .is_some_and(|p| p.rows.start == 648)
                },
                "row-up raster did not return",
            )
            .await?;
            let anchor = owner.anchor.get();
            let count = owner.raster_count.get();
            ui.invoke_article_scroll(-0.1 * layout.line_height(), false);
            tokio::time::sleep(Duration::from_millis(30)).await;
            require(
                owner.raster_count.get() == count,
                "fractional scroll rebuilt unchanged row window",
            )?;
            let anchor = anchor + 0.1;
            for zoom in [1.25, 1.5] {
                ui.set_zoom(zoom);
                wait(
                    || {
                        owner
                            .painted
                            .borrow()
                            .as_ref()
                            .is_some_and(|p| p.row != layout.row)
                    },
                    "zoom did not reraster rows",
                )
                .await?;
                tokio::time::sleep(Duration::from_millis(30)).await;
                require(
                    (owner.anchor.get() - anchor).abs() < 0.001,
                    "zoom lost article row anchor",
                )?;
            }
            let zoomed = owner.layout.get().ok_or("zoomed layout missing")?;
            ui.invoke_article_scroll_to(false, 0.4);
            let column = owner.x.get();
            ui.set_zoom(1.75);
            tokio::time::sleep(Duration::from_millis(30)).await;
            require(
                (owner.x.get() - column).abs() < 0.001,
                "horizontal column anchor changed on zoom",
            )?;
            ui.set_zoom(1.5);
            tokio::time::sleep(Duration::from_millis(30)).await;
            require(
                (owner.anchor.get() - anchor).abs() < 0.001,
                "horizontal controls changed row anchor",
            )?;
            ui.set_native_composing(true);
            ui.invoke_article_scroll(-zoomed.line_height(), false);
            verification::key(&ui, Key::Home);
            require(
                (owner.anchor.get() - anchor).abs() < 0.001,
                "composing gate allowed article scroll",
            )?;
            ui.set_native_composing(false);
            verification::capture(&ui, &directory, "zoom-anchor.png")?;
            ui.window().set_size(slint::LogicalSize::new(740., 520.));
            tokio::time::sleep(Duration::from_millis(60)).await;
            require(
                (owner.anchor.get() - anchor).abs() < 0.001,
                "resize lost article row anchor",
            )?;
            let resized = owner.layout.get().ok_or("resized layout missing")?;
            let horizontal = slint::LogicalPosition::new(
                ui.get_terminal_origin_x() + resized.view_width * 0.9,
                ui.get_terminal_origin_y() + resized.body_height + resized.line_height() + 6.,
            );
            verification::click(&ui, horizontal, Button::Left);
            require(
                owner.x.get() > 0.,
                "native horizontal scrollbar did not move",
            )?;
            ui.invoke_article_scroll_to(false, 0.);
            verification::key(&ui, Key::End);
            wait(
                || {
                    owner
                        .painted
                        .borrow()
                        .as_ref()
                        .is_some_and(|p| p.rows.end == 837)
                },
                "last article row unreachable",
            )
            .await?;
            tokio::time::sleep(Duration::from_millis(30)).await;
            verification::capture(&ui, &directory, "last-row.png")?;
            let last = owner.layout.get().ok_or("last layout missing")?;
            require(
                (owner.anchor.get() - last.maximum()).abs() < 0.001,
                "End did not clamp to real document edge",
            )?;
            let model = ui.get_article_tiles();
            let pixels = (0..model.row_count())
                .filter_map(|i| model.row_data(i))
                .map(|t| {
                    let size = t.image.size();
                    u64::from(size.width) * u64::from(size.height)
                })
                .sum::<u64>();
            require(
                pixels <= 16 * 1024 * 1024,
                "visible raster pixel budget exceeded",
            )?;
            let mut expected_frame = (owner.project)(&owner.source.borrow(), 836..837)
                .ok_or("last row source missing")?
                .paint
                .frame;
            expected_frame.rows = 1;
            expected_frame.glyphs.retain(|g| g.row == 0);
            expected_frame.hyperlinks.clear();
            expected_frame.cursor_visible = false;
            let tile_frame = owner.renderer.borrow().fork(expected_frame);
            let raw = tile_frame
                .paint(
                    crate::renderer::Size {
                        width: last.width,
                        height: last.row,
                    },
                    false,
                )
                .map_err(str::to_owned)?;
            crate::save_png(&raw, &directory.join("last-tile.png")).map_err(|e| e.to_string())?;
            let tile_text = pttstella::selection::extract(
                tile_frame.frame(),
                &pttstella::selection::Selection::All,
            )
            .map_err(|e| e.to_string())?
            .text;
            drop(tile_frame);
            require(
                tile_text
                    .lines()
                    .last()
                    .is_some_and(|line| line.trim_end() == "Full article synthetic line 0837"),
                "last raster lacks expected line 837",
            )?;
            let shot = ui.window().take_snapshot().map_err(|e| e.to_string())?;
            let dpi = ui.window().scale_factor();
            let target_x = ((ui.get_terminal_origin_x()
                + (last.view_width - last.logical_width()).max(0.) / 2.
                - ui.get_article_offset_x())
                * dpi)
                .round() as usize;
            let target_y = ((ui.get_terminal_origin_y() + last.body_height - last.line_height())
                * dpi)
                .round() as usize;
            let compare_width = (last.view_width * dpi).floor().min(512.) as usize;
            require(
                target_x + compare_width <= shot.width() as usize
                    && target_y + last.row as usize <= shot.height() as usize,
                "last row lies outside snapshot",
            )?;
            let expected = raw.as_bytes();
            let actual = shot.as_bytes();
            let mut mismatches = 0;
            for y in 0..last.row as usize {
                for x in 0..compare_width {
                    let source = ((raw.height() as usize - last.row as usize + y)
                        * raw.width() as usize
                        + x)
                        * 4;
                    let target = ((target_y + y) * shot.width() as usize + target_x + x) * 4;
                    if expected[source..source + 4]
                        .iter()
                        .zip(&actual[target..target + 4])
                        .any(|(a, b)| a.abs_diff(*b) > 1)
                    {
                        mismatches += 1;
                    }
                }
            }
            require(
                mismatches == 0,
                &format!("last row does not match visible native pixels: {mismatches}"),
            )?;

            let final_anchor = owner.anchor.get();
            let final_rows = owner
                .painted
                .borrow()
                .as_ref()
                .map(|p| [p.rows.start, p.rows.end]);
            let final_offset = ui.get_article_offset_y();
            ui.invoke_article_scroll(-1., false);
            owner.finish();
            require(
                !owner.timer.running()
                    && owner.row_cache.borrow().is_empty()
                    && ui.get_article_tiles().row_count() == 0
                    && !ui.get_article_active(),
                "closed article view retained timer or raster model",
            )?;
            let record = serde_json::json!({"scope":"offline native viewport; toolkit events only; no station/HTTP/input owner acceptance", "rows":owner.rows.get(),"layout":format!("{last:?}"),"view":[ui.get_view_width(),ui.get_view_height()],"tile_text":tile_text,"line_height":last.line_height(),"offset_y":final_offset,"last_anchor":final_anchor,"visible_rows":final_rows,"tiles":model.row_count(),"last_row_pixel_mismatches":mismatches,"native_wheel_scrollbars":true,"width_only_resize_cancels_grab":true,"owned_timer_model_cleanup":true,"horizontal_column_anchor":true,"last_row_screen_origin":[target_x,target_y],"body_pixels":pixels,"raster_count":owner.raster_count.get(),"row_raster_count":owner.row_raster_count.get(),"retained_rows_reused":true,"fractional_scroll_reuses_raster":true,"zoom_resize_preserve_anchor":true});
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join("result.json"))
                .map_err(|e| e.to_string())?;
            serde_json::to_writer_pretty(&mut file, &record).map_err(|e| e.to_string())?;
            Ok(())
        },
    )
}
