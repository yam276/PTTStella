//! Complete widgets and strict local peer. Synthetic toolkit gestures are
//! explicitly separate from trusted OS input and real PTT acceptance.
use super::*;
use crate::{
    media::Media,
    verification::{self, Verification, require, wait},
};
use pttstella::{
    connection::Connection,
    selection::{Selection, Span},
    surface_state::CopyTarget,
};
use slint::{
    Model,
    platform::{Key, PointerEventButton as Button, WindowEvent as Event},
};
use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::Arc,
};

fn point(
    ui: &TerminalWindow,
    o: &Manager,
    row: usize,
    col: usize,
) -> Result<slint::LogicalPosition, String> {
    let g = o.output.geometry(ui).ok_or("account geometry missing")?;
    let r = o
        .output
        .with_frame(ui, |f| {
            g.span(
                &Span {
                    row,
                    start: col,
                    end: col + 1,
                },
                f,
            )
        })
        .flatten()
        .ok_or("account cell outside viewport")?;
    let (left, top) = match &g {
        crate::pointer::SurfaceGeometry::Terminal(t) => (t.left, t.top),
        _ => (0., 0.),
    };
    Ok(slint::LogicalPosition::new(
        ui.get_terminal_origin_x() + (left + (r.left + r.right) / 2.) / g.dpi(),
        ui.get_terminal_origin_y() + (top + (r.top + r.bottom) / 2.) / g.dpi(),
    ))
}
fn author_click(ui: &TerminalWindow, o: &Manager) -> Result<(), String> {
    let pos = point(ui, o, 8, 4)?;
    let dpi = ui.window().scale_factor();
    let a = o.output.account_context(
        ui,
        (
            (pos.x - ui.get_terminal_origin_x()) * dpi,
            (pos.y - ui.get_terminal_origin_y()) * dpi,
        ),
    );
    eprintln!(
        "author click diagnostic {}",
        serde_json::json!({"foreground":ui.get_native_foreground(),"blocked":ui.get_terminal_pointer_blocked(),"composing":ui.get_native_composing(),"paint_pending":ui.get_frame_paint_pending(),"selection":ui.get_selection_active(),"target":a.map(|a|serde_json::json!({"cell":a.cell,"word":a.word.map(|w|w.id),"author":a.author,"reply":a.reply,"focused":a.focused}))})
    );
    verification::click(ui, pos, Button::Left);
    Ok(())
}
fn item(ui: &TerminalWindow, key: &str) -> Result<usize, String> {
    let label = pttstella::i18n::application().loader().get(key);
    let entries = ui.get_context_entries();
    (0..entries.row_count())
        .find(|i| {
            entries.row_data(*i).is_some_and(|e| {
                e.enabled && (e.label == label || e.label.ends_with(&format!(" · {label}")))
            })
        })
        .ok_or_else(|| {
            format!(
                "account menu action missing: {key}; entries={:?}",
                (0..entries.row_count())
                    .filter_map(|i| entries.row_data(i))
                    .map(|e| (e.label.to_string(), e.enabled))
                    .collect::<Vec<_>>()
            )
        })
}
async fn open(ui: &TerminalWindow, o: &Manager, row: usize, col: usize) -> Result<(), String> {
    verification::foreground(ui, true);
    let p = point(ui, o, row, col)?;
    eprintln!("account open row={row} col={col}");
    let dpi = ui.window().scale_factor();
    require(
        o.output
            .account_context(
                ui,
                (
                    (p.x - ui.get_terminal_origin_x()) * dpi,
                    (p.y - ui.get_terminal_origin_y()) * dpi,
                ),
            )
            .is_some_and(|a| a.cell == Some((row, col))),
        "probe pointer did not map to the independently specified cell",
    )?;
    verification::click(ui, p, Button::Right);
    wait(
        || ui.get_context_open(),
        "account pointer did not open content menu",
    )
    .await
}
async fn choose(ui: &TerminalWindow, o: &Manager, key: &str) -> Result<(), String> {
    eprintln!("account choose {key}");
    let i = item(ui, key)?;
    ui.invoke_context_choose(i as i32);
    wait(
        || !ui.get_context_open() && !o.output.busy() && o.result.borrow().is_none(),
        "account action did not finish its file worker",
    )
    .await?;
    wait(
        || o.output.current(ui).is_some(),
        "account projection was not acknowledged",
    )
    .await
}
fn row(o: &Manager, ui: &TerminalWindow, index: usize) -> Option<String> {
    o.output
        .with_frame(ui, |f| {
            pttstella::selection::extract(f, &Selection::Line { row: index })
                .ok()
                .map(|s| s.text)
        })
        .flatten()
}
fn images(
    ui: &TerminalWindow,
    media: &Media,
) -> Vec<(i32, Arc<pttstella::media::loader::Image>, f32)> {
    let model = ui.get_inline_pictures();
    (0..model.row_count())
        .filter_map(|i| model.row_data(i))
        .filter_map(|p| {
            Some((
                p.index,
                media.inline_picture(p.index, p.stamp.as_str())?.image?,
                p.width,
            ))
        })
        .collect()
}
async fn png(
    ui: &TerminalWindow,
    o: &Manager,
    c: &Connection,
    d: &Path,
    name: &str,
) -> Result<(), String> {
    crate::article::output_probe::png(ui, &o.output, c, d, name, false).await
}
pub fn start(
    ui: &TerminalWindow,
    o: Rc<Manager>,
    c: Rc<Connection>,
    media: Rc<Media>,
    directory: PathBuf,
    path: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(
        ui,
        directory,
        "account-actions",
        move |ui, directory| async move {
            wait(
                || {
                    verification::foreground(&ui, true);
                    if ui.get_article_active() {
                        ui.invoke_focus_article();
                    } else {
                        ui.invoke_focus_terminal();
                    }
                    o.output.current(&ui).is_some()
                },
                "account source not painted",
            )
            .await?;
            let web = std::env::var("PTTSTELLA_SLINT_FILTERS_PROBE").is_ok_and(|v| v == "web");
            require(
                ui.get_article_active() == web,
                "wrong account probe reading mode",
            )?;
            require(
                row(&o, &ui, 4).is_some_and(|s| s.contains("Blocked") && s.contains("private.png")),
                "account source fixture differs",
            )?;
            require(
                o.article.with_output(&c.updates.borrow(), |p| {
                    p.replies
                        .iter()
                        .map(|r| (r.row, r.id.as_str()))
                        .collect::<Vec<_>>()
                        == [(4, "Blocked"), (8, "Alice"), (9, "ALICE")]
                }) == Some(true),
                "account fixture standard reply metadata differs",
            )?;
            png(&ui, &o, &c, &directory, "before.png").await?;
            open(&ui, &o, 4, 4).await?;
            require(
                ui.get_context_entries()
                    .row_data(item(&ui, "account-mask")?)
                    .is_some_and(|e| e.label.starts_with("Blocked · ")),
                "menu did not identify the visible account",
            )?;
            choose(&ui, &o, "account-mask").await?;
            require(
                o.rules()
                    .entries
                    .iter()
                    .any(|e| e.id == "Blocked" && e.policy == Policy::Mask),
                "pointer account mask did not save",
            )?;
            require(
                row(&o, &ui, 4)
                    .is_some_and(|s| s.contains("Blocked") && !s.contains("private.png")),
                "content mask lost ID or exposed content",
            )?;
            // The explicit form and the quick action share one Store and note.
            ui.invoke_filters_show();
            let w = o.window().ok_or("account form missing")?;
            w.window().dispatch_event(Event::WindowActiveChanged(true));
            w.set_account(w.invoke_field_edit(0, "Blocked".into()));
            w.set_note(w.invoke_field_edit(1, "preserve this note".into()));
            w.invoke_action(0);
            wait(
                || !o.output.busy() && o.result.borrow().is_none(),
                "account form write pending",
            )
            .await?;
            w.invoke_close_filters();
            wait(
                || {
                    verification::foreground(&ui, true);
                    o.output.current(&ui).is_some()
                },
                "form handoff repaint pending",
            )
            .await?;
            let before_failure = std::fs::read(&path).map_err(|e| e.to_string())?;
            std::fs::remove_file(path.with_extension("json.bak")).map_err(|e| e.to_string())?;
            std::fs::create_dir(path.with_extension("json.bak")).map_err(|e| e.to_string())?;
            open(&ui, &o, 4, 4).await?;
            choose(&ui, &o, "account-mask-row").await?;
            require(
                o.rules()
                    .entries
                    .iter()
                    .any(|e| e.policy == Policy::Mask && e.note == "preserve this note")
                    && std::fs::read(&path).map_err(|e| e.to_string())? == before_failure,
                "failed context save changed effective/durable rules",
            )?;
            require(
                !ui.get_input_notice().is_empty() && ui.get_input_notice() != msg!("filters-saved"),
                "context save failure was not visible",
            )?;
            std::fs::write(
                directory.join("context-save-failure.txt"),
                ui.get_input_notice().as_str(),
            )
            .map_err(|e| e.to_string())?;
            std::fs::remove_dir(path.with_extension("json.bak")).map_err(|e| e.to_string())?;
            open(&ui, &o, 4, 4).await?;
            choose(&ui, &o, "account-mask-row").await?;
            require(
                o.rules()
                    .entries
                    .iter()
                    .any(|e| e.policy == Policy::MaskRow && e.note == "preserve this note"),
                "quick policy change discarded the saved note",
            )?;
            require(
                row(&o, &ui, 4).is_some_and(|s| s.chars().all(|c| c == '█')),
                "whole row did not become opaque",
            )?;
            let g = o.output.geometry(&ui).ok_or("masked geometry missing")?;
            let logical = point(&ui, &o, 4, 4)?;
            require(
                o.output
                    .account_context(
                        &ui,
                        (
                            (logical.x - ui.get_terminal_origin_x()) * g.dpi(),
                            (logical.y - ui.get_terminal_origin_y()) * g.dpi(),
                        ),
                    )
                    .is_some_and(|a| a.word.is_none()),
                "hidden author leaked through hit metadata",
            )?;
            require(
                o.article.with_output(&c.updates.borrow(), |p| {
                    p.links.iter().all(|l| !l.url.contains("private.png"))
                }) == Some(true),
                "masked link escaped the common projection",
            )?;
            let view = c.updates.borrow().clone();
            let (selected_id, selected) = o
                .article
                .select_output(&view, 42, &Selection::Line { row: 4 })
                .ok_or("masked selection missing")?;
            require(
                selected.text.chars().all(|c| c == '█'),
                "selection recovered the masked row",
            )?;
            let lease = o
                .article
                .lease_output(&view, None)
                .ok_or("account output lease missing")?;
            png(&ui, &o, &c, &directory, "masked.png").await?;
            let (width, _, pixels) =
                crate::article::output_probe::pixels(&directory.join("masked.png"))?;
            require(
                width == 960
                    && pixels[4 * 24 * 960 * 4..5 * 24 * 960 * 4]
                        .chunks_exact(4)
                        .all(|p| p == [0, 0, 0, 255]),
                "whole-row source PNG contains exposed pixels",
            )?;
            require(ui.get_filter_count() == 1, "native reveal status missing")?;
            ui.invoke_filter_reveal();
            wait(
                || o.article.with_output(&c.updates.borrow(), |p| p.revealed) == Some(true),
                "temporary reveal did not acknowledge",
            )
            .await?;
            require(
                row(&o, &ui, 4).is_some_and(|s| s.contains("private.png")),
                "explicit reveal did not restore content",
            )?;
            require(
                o.article
                    .copy_output(&c.updates.borrow(), &CopyTarget::Selection { request: 42 })
                    .is_none()
                    && lease.check(&c.updates.borrow()).is_err(),
                "reveal revived an old selection/export lease",
            )?;
            o.article.clear_output_selection(selected_id, 42);
            png(&ui, &o, &c, &directory, "revealed.png").await?;
            open(&ui, &o, 4, 4).await?;
            choose(&ui, &o, "account-remove-mask-row").await?;
            require(
                o.rules().entries.is_empty(),
                "toggled whole-row removal failed",
            )?;
            open(&ui, &o, 4, 4).await?;
            choose(&ui, &o, "account-allow").await?;
            require(
                o.rules().entries.iter().any(|e| e.policy == Policy::Allow),
                "quick whitelist failed",
            )?;
            open(&ui, &o, 4, 4).await?;
            choose(&ui, &o, "account-remove-allow").await?;
            // An unrelated background change is independent of persistent rules.
            open(&ui, &o, 4, 4).await?;
            choose(&ui, &o, "account-mask").await?;
            let inline = media.inline_owner().ok_or("account inline owner missing")?;
            let before = std::fs::read(&path).map_err(|e| e.to_string())?;
            if web {
                wait(
                    || ui.get_inline_enabled() && ui.get_inline_arrows().row_count() == 3,
                    "focus image fixture missing",
                )
                .await?;
                for _ in 0..3 {
                    let model = ui.get_inline_arrows();
                    let a = (0..model.row_count())
                        .filter_map(|i| model.row_data(i))
                        .find(|a| !a.opened)
                        .ok_or("unopened focus image missing")?;
                    let p = slint::LogicalPosition::new(
                        ui.get_terminal_origin_x() + a.x + a.size / 2.,
                        ui.get_terminal_origin_y() + a.y + a.size / 2.,
                    );
                    verification::click(&ui, p, Button::Left);
                    wait(
                        || {
                            ui.get_inline_enabled()
                                && ui.get_inline_pictures().row_count()
                                    > (images(&ui, &media).len().saturating_sub(1))
                        },
                        "focus image placement pending",
                    )
                    .await?;
                    wait(
                        || {
                            ui.get_inline_arrows().row_data(0).is_some()
                                && (0..ui.get_inline_arrows().row_count())
                                    .filter_map(|i| ui.get_inline_arrows().row_data(i))
                                    .any(|x| x.index == a.index && x.opened)
                        },
                        "focus arrow did not expand",
                    )
                    .await?;
                }
                wait(
                    || crate::article::inline::probe::focus_resources(&inline).decoded() == 3,
                    "focus images not decoded",
                )
                .await?;
                ui.invoke_focus_article();
                require(
                    ui.invoke_inline_enter(true, false, true),
                    "wide Enter press rejected",
                )?;
                require(
                    ui.invoke_inline_enter(false, false, true),
                    "wide Enter release rejected",
                )?;
                wait(
                    || {
                        images(&ui, &media)
                            .iter()
                            .any(|(_, _, width)| *width > 100.)
                    },
                    "focus fixture did not enter wide mode",
                )
                .await?;
            }
            if web && let Some(crate::pointer::SurfaceGeometry::Article(g)) = o.output.geometry(&ui)
            {
                let target = (g.flow.top(8) as f64 - f64::from(g.body_height) * 0.6).max(0.);
                ui.invoke_article_scroll(((g.offset - target) / f64::from(g.dpi)) as f32, false);
                wait(
                    || o.output.current(&ui).is_some(),
                    "focus scroll did not paint",
                )
                .await?;
            }
            let resources = crate::article::inline::probe::focus_resources(&inline);
            require(
                !web || resources.wide(),
                "focus fixture did not retain an actual wide choice",
            )?;
            let originals = images(&ui, &media);
            let y = ui.get_article_offset_y();
            author_click(&ui, &o)?;
            wait(
                || {
                    o.article.with_output(&c.updates.borrow(), |p| {
                        p.reply_focus.as_deref() == Some("Alice")
                    }) == Some(true)
                },
                "author click failed to highlight before Back navigation",
            )
            .await?;
            require(
                o.output.with_frame(&ui, |f| {
                    f.glyphs
                        .iter()
                        .any(|g| g.row == 8 && g.col == 3 && g.left.bg == 4)
                }) == Some(true),
                "reply background not blue",
            )?;
            require(
                o.output.with_frame(&ui, |f| {
                    f.glyphs
                        .iter()
                        .filter(|g| g.row == 9 && (3..8).contains(&g.col))
                        .all(|g| g.text == " " && g.left.fg == 0 && g.left.bg == 0)
                }) == Some(true),
                "reply focus exposed a merged author",
            )?;
            if web {
                wait(
                    || resources.retained(&inline) && ui.get_inline_enabled(),
                    "focus reset decoded images or wide state",
                )
                .await?;
                require(ui.get_article_offset_y() == y, "focus moved the viewport")?;
                let model = ui.get_inline_pictures();
                let gif = (0..model.row_count())
                    .filter_map(|i| model.row_data(i))
                    .find(|p| {
                        originals.iter().any(|(i, image, _)| {
                            *i == p.index
                                && image.mime == "image/gif"
                                && image.animation_pixels > image.width * image.height
                        })
                    })
                    .ok_or("focus GIF missing")?;
                let initial = gif
                    .image
                    .to_rgba8()
                    .ok_or("focus GIF pixels missing")?
                    .as_bytes()
                    .to_vec();
                wait(
                    || {
                        (0..ui.get_inline_pictures().row_count())
                            .filter_map(|i| ui.get_inline_pictures().row_data(i))
                            .find(|p| p.index == gif.index)
                            .and_then(|p| p.image.to_rgba8())
                            .is_some_and(|p| p.as_bytes() != initial)
                    },
                    "focused GIF did not resume animation",
                )
                .await?;
            }
            require(
                std::fs::read(&path).map_err(|e| e.to_string())? == before,
                "reply focus wrote persistent rules",
            )?;
            author_click(&ui, &o)?;
            wait(
                || {
                    o.article
                        .with_output(&c.updates.borrow(), |p| p.reply_focus.is_none())
                        == Some(true)
                },
                "second author click did not clear focus",
            )
            .await?;
            open(&ui, &o, 8, 4).await?;
            choose(&ui, &o, "reply-focus").await?;
            require(
                o.article.with_output(&c.updates.borrow(), |p| {
                    p.reply_focus.as_deref() == Some("Alice")
                }) == Some(true),
                "right-click reply focus did not use its acknowledged row",
            )?;
            open(&ui, &o, 8, 4).await?;
            choose(&ui, &o, "reply-focus-clear").await?;
            require(
                o.article
                    .with_output(&c.updates.borrow(), |p| p.reply_focus.is_none())
                    == Some(true),
                "right-click focus clear failed",
            )?;
            ui.window().dispatch_event(Event::KeyPressed {
                text: Key::Shift.into(),
            });
            verification::click(&ui, point(&ui, &o, 8, 4)?, Button::Left);
            ui.window().dispatch_event(Event::KeyReleased {
                text: Key::Shift.into(),
            });
            require(
                o.article
                    .with_output(&c.updates.borrow(), |p| p.reply_focus.is_none())
                    == Some(true),
                "Shift author click toggled reply focus",
            )?;
            crate::article::output_probe::drag(&ui, point(&ui, &o, 8, 3)?, point(&ui, &o, 8, 8)?);
            require(
                o.output.selection_key().is_some_and(|(rows, spans)| {
                    rows == (8..9)
                        && spans
                            .iter()
                            .any(|span| span.row == 8 && span.start == 3 && span.end >= 8)
                }),
                "Shift drag did not select the author text",
            )?;
            require(
                o.article
                    .with_output(&c.updates.borrow(), |p| p.reply_focus.is_none())
                    == Some(true),
                "Shift drag toggled reply focus",
            )?;
            ui.invoke_clear_selection();
            // Capture an old target and menu, then deliver a real new peer frame.
            ui.invoke_filter_reveal();
            wait(
                || o.article.with_output(&c.updates.borrow(), |p| p.revealed) == Some(true),
                "pre-transition reveal failed",
            )
            .await?;
            open(&ui, &o, 4, 4).await?;
            let stale_index = item(&ui, "account-mask-row")?;
            let p = point(&ui, &o, 4, 4)?;
            let dpi = ui.window().scale_factor();
            let stale = o
                .output
                .account_context(
                    &ui,
                    (
                        (p.x - ui.get_terminal_origin_x()) * dpi,
                        (p.y - ui.get_terminal_origin_y()) * dpi,
                    ),
                )
                .ok_or("stale menu target missing")?;
            let disk = std::fs::read(&path).map_err(|e| e.to_string())?;
            c.input(c.updates.borrow().generation, b"\x1b[6~".to_vec())?;
            wait(
                || {
                    matches!(
                        c.updates.borrow().context(),
                        pttstella::navigation::Context::ArticleList { .. }
                    ) && o.output.current(&ui).is_some()
                },
                "new list frame did not acknowledge",
            )
            .await?;
            require(
                o.article.with_output(&c.updates.borrow(), |p| !p.revealed) == Some(true),
                "new source revision retained temporary reveal",
            )?;
            ui.invoke_context_choose(stale_index as i32);
            o.account_action(&stale, Policy::Mask, false);
            require(
                std::fs::read(&path).map_err(|e| e.to_string())? == disk,
                "stale menu or target wrote another frame's rule",
            )?;
            open(&ui, &o, 3, 25).await?;
            choose(&ui, &o, "account-allow").await?;
            require(
                o.rules()
                    .entries
                    .iter()
                    .any(|e| e.id == "alice" && e.policy == Policy::Allow),
                "list author whitespace did not resolve the complete ID",
            )?;
            open(&ui, &o, 3, 25).await?;
            choose(&ui, &o, "account-mask-row").await?;
            require(
                row(&o, &ui, 3).is_some_and(|s| s.chars().all(|c| c == '█')),
                "list row mask did not apply",
            )?;
            ui.invoke_filter_reveal();
            wait(
                || o.article.with_output(&c.updates.borrow(), |p| p.revealed) == Some(true),
                "list reveal failed",
            )
            .await?;
            open(&ui, &o, 3, 25).await?;
            choose(&ui, &o, "account-remove-mask-row").await?;
            require(
                o.rules().entries.len() == 1
                    && o.rules().entries[0].id == "Blocked"
                    && o.rules().entries[0].policy == Policy::Mask
                    && o.article.requests.get() == 0,
                "account actions left rules or sent automatic station pages",
            )?;
            verification::capture(&ui, &directory, "list-after-actions.png")?;
            std::fs::write(directory.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","scope":"native widgets, synthetic foreground/pointer and strict local peer; OS input and IME pending","web":web,"quick_policies_and_removals":true,"note_preserved":true,"masked_copy_link_png":true,"old_output_revoked":true,"focus_click_before_back":true,"merged_id_stays_masked":true,"focus_no_store_write":true,"shift_keeps_selection":true,"inline_originals_wide_anchor_retained":web,"gif_resumed":web,"stale_menu_and_target_rejected":true,"context_save_failure_preserved":true,"second_click_and_menu_toggle":true,"reveal_resets_on_peer_frame":true,"list_author_whitespace":true,"station_bytes_hex":"1b5b367e","automatic_page_requests":0})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            Ok(())
        },
    )
}
