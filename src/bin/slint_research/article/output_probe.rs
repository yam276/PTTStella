//! Native component gestures and the production output worker, with an owned
//! PNG sink. This fixture does not open OS clipboard, picker, or browser APIs.
use super::session::ArticleSession;
use crate::{
    TerminalWindow,
    output::{self, Output},
    pointer::SurfaceGeometry,
    verification::{self, Verification, require, wait},
};
use pttstella::{article_cache::LoadState, connection::Connection, selection::Span};
use slint::{
    ComponentHandle, Model,
    platform::{Key, PointerEventButton as Button, WindowEvent as Event},
};
use std::{
    error::Error,
    io::Write,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

fn point(
    ui: &TerminalWindow,
    output: &Output,
    row: usize,
    col: usize,
) -> Result<slint::LogicalPosition, String> {
    let g = output.geometry(ui).ok_or("article geometry missing")?;
    let rect = output
        .with_frame(ui, |frame| {
            g.span(
                &Span {
                    row,
                    start: col,
                    end: col + 1,
                },
                frame,
            )
        })
        .flatten()
        .ok_or("probe cell is outside the visible article")?;
    Ok(slint::LogicalPosition::new(
        ui.get_terminal_origin_x() + (rect.left + rect.right) / 2. / g.dpi(),
        ui.get_terminal_origin_y() + (rect.top + rect.bottom) / 2. / g.dpi(),
    ))
}
pub(crate) fn drag(ui: &TerminalWindow, a: slint::LogicalPosition, b: slint::LogicalPosition) {
    ui.window().dispatch_event(Event::KeyPressed {
        text: Key::Shift.into(),
    });
    ui.set_native_pointer_down(true);
    ui.set_native_pointer_buttons(1);
    ui.window().dispatch_event(Event::PointerPressed {
        position: a,
        button: Button::Left,
    });
    ui.window()
        .dispatch_event(Event::PointerMoved { position: b });
    ui.set_native_pointer_down(false);
    ui.set_native_pointer_buttons(0);
    ui.window().dispatch_event(Event::PointerReleased {
        position: b,
        button: Button::Left,
    });
    ui.window().dispatch_event(Event::KeyReleased {
        text: Key::Shift.into(),
    });
}
pub(crate) async fn png(
    ui: &TerminalWindow,
    output: &Rc<Output>,
    connection: &Connection,
    directory: &Path,
    name: &str,
    selected: bool,
) -> Result<(), String> {
    let (frame, rows, lease, phase) = output.prepare_export(ui, selected)?;
    let display = pttstella::replay::parity_display_profile().resolve()?;
    let source = connection.updates.clone();
    let path = directory.join(name);
    let (send, receive) = tokio::sync::oneshot::channel();
    output.work(
        ui,
        move || {
            output::check_output(&lease, &source.borrow(), phase)?;
            let image = output::render_png(frame, display, rows)?;
            output::check_output(&lease, &source.borrow(), phase)?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|e| e.to_string())?;
            file.write_all(&image.png).map_err(|e| e.to_string())?;
            Ok(())
        },
        move |_, _, result| {
            let _ = send.send(result);
        },
    );
    receive
        .await
        .map_err(|_| "article PNG worker did not complete".to_owned())??;
    wait(
        || !ui.get_output_busy(),
        "article output worker remained busy",
    )
    .await
}
pub(crate) fn pixels(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|e| e.to_string())?;
    let mut pixels = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("PNG output budget missing")?
    ];
    let info = reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
    Ok((info.width, info.height, pixels))
}
pub fn start(
    ui: &TerminalWindow,
    owner: Rc<ArticleSession>,
    connection: Rc<Connection>,
    output: Rc<Output>,
    directory: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(
        ui,
        directory,
        "article-output",
        move |ui, directory| async move {
            let ready = wait(
                || {
                    verification::foreground(&ui, true);
                    owner.progress().is_some_and(|(_, _, last, state)| {
                        last == 45 && state == Some(LoadState::Complete)
                    }) && output.current(&ui).is_some()
                },
                "article output fixture did not load",
            )
            .await;
            if let Err(error) = ready {
                let core = owner.core.borrow();
                let view = connection.updates.borrow();
                let geometry = output.geometry(&ui);
                let raster=owner.view.borrow().as_ref().map(|native|serde_json::json!({"timer":native.timer.running(),"rows":native.rows.get(),"layout":format!("{:?}",native.layout.get()),"painted":format!("{:?}",native.painted.borrow()),"anchor":native.anchor.get(),"raster_count":native.raster_count.get()}));
                let state = serde_json::json!({"progress":format!("{:?}",owner.progress()),"requests":owner.requests.get(),"geometry":format!("{geometry:?}"),"raster":raster,"dirty":core.dirty(),"in_flight":core.has_in_flight(),"ack":core.last_displayed().map(|p|serde_json::json!({"generation":p.generation,"revision":p.frame.revision,"input_epoch":p.input_epoch,"policy":p.presentation})),"view":format!("{:?}",view.stamp()),"phase":format!("{:?}",view.phase),"input_pending":view.input_pending,"output_pending":view.output_pending,"active":ui.get_article_active()});
                std::fs::write(
                    directory.join("not-ready.json"),
                    serde_json::to_vec_pretty(&state).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                drop(view);
                drop(core);
                verification::capture(&ui, &directory, "not-ready.png")?;
                return Err(error);
            }
            // Resolve the generated component's first native layout before
            // deriving pointer coordinates, just as a visible draw would.
            verification::capture(&ui, &directory, "before-selection.png")?;
            wait(
                || output.current(&ui).is_some() && !ui.get_terminal_pointer_blocked(),
                "first native layout did not settle",
            )
            .await?;
            require(
                owner.requests.get() == 1,
                "article output loaded an extra page",
            )?;
            require(
                matches!(output.geometry(&ui), Some(SurfaceGeometry::Article(_))),
                "output consumed terminal geometry",
            )?;
            // An actual generated component drag selects the reversed halves of a
            // historical Big5 glyph, not the current 23..45 station page.
            let a = point(&ui, &output, 5, 7)?;
            let b = point(&ui, &output, 5, 6)?;
            drag(&ui, a, b);
            if !ui.get_selection_active() {
                verification::capture(&ui, &directory, "selection-failure.png")?;
                let mut state = serde_json::json!({"origin":[ui.get_terminal_origin_x(),ui.get_terminal_origin_y()],"a":[a.x,a.y],"b":[b.x,b.y],"blocked":ui.get_terminal_pointer_blocked(),"foreground":ui.get_native_foreground(),"composing":ui.get_native_composing(),"pan":ui.get_pan_mode(),"article_focused":ui.get_article_focused(),"geometry":format!("{:?}",output.geometry(&ui)),"ready":output.current(&ui).is_some()});
                for (kind, p) in [(0, a), (1, b), (2, b)] {
                    ui.invoke_grid_pointer(
                        kind,
                        p.x - ui.get_terminal_origin_x(),
                        p.y - ui.get_terminal_origin_y(),
                        true,
                    );
                }
                state["direct_callback_selects"] = ui.get_selection_active().into();
                std::fs::write(
                    directory.join("selection-failure.json"),
                    serde_json::to_vec_pretty(&state).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                return Err("article component drag did not select".into());
            }
            let (text, lease, phase) = output.prepare_copy(&ui, false)?;
            require(
                text == "中",
                "history copy split or substituted the dual-colour glyph",
            )?;
            output::check_output(&lease, &connection.updates.borrow(), phase)?;
            let (ansi, _, _) = output.prepare_copy(&ui, true)?;
            let mut replay = pttstella::screen::Screen::new(80, 24).map_err(|e| e.to_string())?;
            replay.feed(&pttstella::encoding::encode(&ansi).map_err(|e| e.to_string())?);
            let glyph = replay
                .frame()
                .glyphs
                .into_iter()
                .find(|g| g.row == 0 && g.col == 0)
                .ok_or("ANSI glyph missing")?;
            require(
                glyph.text == "中"
                    && (glyph.left.fg, glyph.left.bg, glyph.right.fg, glyph.right.bg)
                        == (1, 4, 2, 5),
                "ANSI history copy lost either half's style",
            )?;
            png(
                &ui,
                &output,
                &connection,
                &directory,
                "history-row.png",
                true,
            )
            .await?;
            let (w, h, rgba) = pixels(&directory.join("history-row.png"))?;
            require((w, h) == (960, 24), "history PNG dimensions differ")?;
            verification::check_dual_backgrounds(&rgba, 6)?;
            verification::capture(&ui, &directory, "selection.png")?;
            // Whole-page exports are source 24 rows and survive a selection clear.
            let (frame, rows, full, phase) = output.prepare_export(&ui, false)?;
            let text = pttstella::selection::extract(
                &frame,
                &pttstella::selection::Selection::Line { row: 0 },
            )?
            .text;
            require(
                rows == (0..24) && text.trim_end() == "Full article synthetic line 0023",
                "whole-page export used history rather than the current source",
            )?;
            ui.invoke_clear_selection();
            output::check_output(&full, &connection.updates.borrow(), phase)?;
            png(
                &ui,
                &output,
                &connection,
                &directory,
                "source-page.png",
                false,
            )
            .await?;
            require(
                pixels(&directory.join("source-page.png"))?.0 == 960,
                "source PNG width differs",
            )?;
            let (_, _, geometry_lease, phase) = output.prepare_export(&ui, false)?;
            let original_geometry = output.geometry(&ui);
            ui.set_zoom(1.25);
            wait(
                || output.current(&ui).is_some() && output.geometry(&ui) != original_geometry,
                "zoomed article output was not acknowledged",
            )
            .await?;
            require(
                output::check_output(&geometry_lease, &connection.updates.borrow(), phase).is_err(),
                "resize left a deferred source export alive",
            )?;
            ui.set_zoom(1.);
            wait(
                || output.current(&ui).is_some() && output.geometry(&ui) == original_geometry,
                "restored article output was not acknowledged",
            )
            .await?;
            // The masked row is completely black even in a history-only export.
            drag(&ui, point(&ui, &output, 4, 79)?, point(&ui, &output, 4, 0)?);
            let (masked, _, _) = output.prepare_copy(&ui, false)?;
            require(
                !masked.contains("hidden")
                    && !masked.contains("private")
                    && !masked.contains("Blocked"),
                "masked history leaked through text copy",
            )?;
            png(
                &ui,
                &output,
                &connection,
                &directory,
                "masked-row.png",
                true,
            )
            .await?;
            let (_, _, rgba) = pixels(&directory.join("masked-row.png"))?;
            require(
                rgba.chunks_exact(4).all(|p| p == [0, 0, 0, 255]),
                "masked history leaked through PNG",
            )?;
            ui.invoke_clear_selection();
            let footer = 45;
            let at = point(&ui, &output, footer, 2)?;
            verification::click(&ui, at, Button::Left);
            require(
                ui.get_input_notice() == "https://example.com/footer",
                "pinned footer clicked through or lost OSC 8 metadata",
            )?;
            verification::click(&ui, at, Button::Right);
            require(
                ui.get_context_open(),
                "pinned footer did not open its content menu",
            )?;
            let model = ui.get_context_entries();
            let select = (0..model.row_count())
                .find(|&i| {
                    model.row_data(i).is_some_and(|e| {
                        e.enabled
                            && e.label
                                == crate::context_menu::label(
                                    pttstella::context_menu::Action::SelectLink,
                                )
                    })
                })
                .ok_or("footer select-link action missing")?;
            ui.invoke_context_choose(select as i32);
            require(
                output.prepare_copy(&ui, false)?.0 == "https://example.com/footer",
                "complete footer URL was replaced by its label",
            )?;
            png(
                &ui,
                &output,
                &connection,
                &directory,
                "footer-row.png",
                true,
            )
            .await?;
            ui.invoke_output_action(6);
            require(
                ui.get_selection_active() && !ui.get_selection_rows_ready(),
                "long selection PNG did not become unavailable",
            )?;
            let (all, stale, phase) = output.prepare_copy(&ui, false)?;
            require(
                !all.contains("hidden")
                    && !all.contains("private")
                    && !all.contains("Blocked")
                    && all.contains("Split 中"),
                "full article copy did not use the filtered projection",
            )?;
            require(
                output.prepare_export(&ui, true).is_err(),
                "more than 24 selected rows became a PNG",
            )?;
            ui.invoke_clear_selection();
            require(
                output::check_output(&stale, &connection.updates.borrow(), phase).is_err(),
                "selection clear did not revoke prepared copy",
            )?;
            // A source-identical policy edit must cancel both a deferred PNG and
            // an already open menu before any new network or raster is available.
            verification::click(&ui, at, Button::Right);
            require(ui.get_context_open(), "policy fixture menu did not open")?;
            let (_, _, policy_lease, phase) = output.prepare_export(&ui, false)?;
            let before = connection.updates.borrow().stamp();
            owner.replace_rules(pttstella::filters::Rules::default().edit_policy(
                "Blocked",
                "changed",
                pttstella::filters::Policy::MaskRow,
                false,
            )?);
            require(
                output::check_output(&policy_lease, &connection.updates.borrow(), phase).is_err()
                    && !ui.get_context_open()
                    && connection.updates.borrow().stamp() == before,
                "policy edit left an old output or menu alive",
            )?;
            // Rebuilding a policy starts a new partial article at source 23.
            // Keep the synthetic host backgrounded while inspecting it, so
            // this output-only peer must still see no automatic Home/fetch.
            verification::foreground(&ui, false);
            wait(
                || output.current(&ui).is_some(),
                "edited policy was not painted",
            )
            .await?;
            let (_, _, reading_lease, phase) = output.prepare_export(&ui, false)?;
            owner.station_handoff();
            wait(
                || {
                    output.current(&ui).is_some()
                        && matches!(output.geometry(&ui), Some(SurfaceGeometry::Terminal(_)))
                },
                "raw fallback did not acknowledge its filtered terminal raster",
            )
            .await?;
            require(
                output::check_output(&reading_lease, &connection.updates.borrow(), phase).is_err(),
                "raw fallback retained the reading output lease",
            )?;
            verification::capture(&ui, &directory, "filtered-raw-terminal.png")?;
            wait(
                || output.current(&ui).is_some(),
                "terminal layout did not settle",
            )
            .await?;
            ui.invoke_output_action(6);
            let (raw_text, _, _) = output.prepare_copy(&ui, false)?;
            require(
                !raw_text.contains("Blocked")
                    && !raw_text.contains("private")
                    && !raw_text.contains("hidden"),
                "raw fallback bypassed copy filtering",
            )?;
            ui.invoke_clear_selection();
            png(
                &ui,
                &output,
                &connection,
                &directory,
                "filtered-raw-page.png",
                false,
            )
            .await?;
            let (w, h, rgba) = pixels(&directory.join("filtered-raw-page.png"))?;
            require(
                (w, h) == (960, 576)
                    && rgba[4 * 24 * 960 * 4..5 * 24 * 960 * 4]
                        .chunks_exact(4)
                        .all(|p| p == [0, 0, 0, 255]),
                "raw fallback bypassed PNG masking",
            )?;
            require(
                output
                    .link_at(&ui, {
                        let p = point(&ui, &output, 4, 20)?;
                        let dpi = ui.window().scale_factor();
                        (
                            (p.x - ui.get_terminal_origin_x()) * dpi,
                            (p.y - ui.get_terminal_origin_y()) * dpi,
                        )
                    })
                    .is_none(),
                "raw fallback exposed a masked URL",
            )?;
            verification::foreground(&ui, false);
            tokio::time::sleep(Duration::from_millis(200)).await;
            require(
                owner.requests.get() == 1,
                "output interaction sent a station navigation",
            )?;
            owner.shutdown().await;
            require(owner.stopped(), "article output actor did not join")?;
            std::fs::write(directory.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"scope":"actual generated Slint component gestures and production output worker, synthetic foreground and owned PNG sink; no OS clipboard/picker/browser or real PTT","rows":45,"requests":1,"reverse_dual_glyph":"中","ansi_styles":[1,4,2,5],"png":[960,24],"masked_png_all_black":true,"source_page_first":23,"pinned_footer_url":"https://example.com/footer","whole_page_survives_selection_clear":true,"stale_selection_revoked":true,"geometry_revoked_deferred_png":true,"same_source_policy_revoked_menu_and_png":true,"filtered_raw_terminal_copy_png_link":true,"article_loop_joined":true})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            Ok(())
        },
    )
}
