//! Shared scripted append/reentry peer plus the real native article and media.
use super::Inline;
use crate::{
    TerminalWindow,
    article::session::ArticleSession,
    verification::{self, Verification, require},
};
use pttstella::{article_cache::LoadState, connection::Connection, navigation::Intent};
use slint::Model;
use std::{error::Error, path::PathBuf, rc::Rc, sync::Arc, time::Duration};
async fn wait(
    ui: &TerminalWindow,
    mut test: impl FnMut() -> bool,
    message: &str,
) -> Result<(), String> {
    for _ in 0..1500 {
        verification::foreground(ui, true);
        ui.invoke_focus_article();
        if test() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Err(message.into())
}
fn same_anchor(a: pttstella::article_flow::Anchor, b: pttstella::article_flow::Anchor) -> bool {
    // Reflow round trips pixels through normalized rows. This bound is below
    // 0.001 physical pixels even at the maximum row height.
    a.row == b.row && (a.within - b.within).abs() <= 1e-6
}
pub fn start(
    ui: &TerminalWindow,
    inline: Rc<Inline>,
    article: Rc<ArticleSession>,
    connection: Rc<Connection>,
    directory: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    let native_check = std::env::var("PTTSTELLA_SLINT_UPDATES_NATIVE_CHECK").unwrap_or_default();
    Verification::start_inner(
        ui,
        directory,
        "reply-updates",
        !matches!(native_check.as_str(), "1" | "2"),
        move |ui, directory| async move {
            let requested_case =
                std::env::var("PTTSTELLA_SLINT_UPDATES_PROBE").map_err(|e| e.to_string())?;
            let mixed = requested_case.starts_with("mixed-");
            let case = requested_case
                .strip_prefix("mixed-")
                .unwrap_or(&requested_case);
            wait(
                &ui,
                || {
                    article.progress().is_some_and(|(_, _, last, state)| {
                        last == 45 && state == Some(LoadState::Complete)
                    }) && ui.get_article_updates_available()
                        && article
                            .output_identity(&connection.updates.borrow())
                            .is_some()
                },
                "initial full article/updates lease missing",
            )
            .await?;
            require(
                !ui.get_article_updates_enabled(),
                "updates started without opt-in",
            )?;
            let with_image = std::env::var_os("PTTSTELLA_SLINT_UPDATES_ORIGIN").is_some();
            let original = if with_image {
                inline.expand_all();
                wait(
                    &ui,
                    || {
                        inline
                            .state
                            .borrow()
                            .resources
                            .values()
                            .any(|r| r.original.is_some())
                    },
                    "initial inline image did not decode",
                )
                .await?;
                inline
                    .state
                    .borrow()
                    .resources
                    .values()
                    .find_map(|r| r.original.clone())
            } else {
                None
            };
            let resources = if mixed && with_image {
                wait(
                    &ui,
                    || {
                        super::probe::focus_resources(&inline).decoded() == 2
                            && ui.get_inline_enabled()
                    },
                    "mixed PNG/GIF originals did not both decode",
                )
                .await?;
                require(
                    ui.invoke_inline_enter(true, false, true)
                        && ui.invoke_inline_enter(false, false, true),
                    "mixed wide choice was rejected",
                )?;
                wait(
                    &ui,
                    || super::probe::focus_resources(&inline).wide(),
                    "mixed image did not enter wide mode",
                )
                .await?;
                Some(super::probe::focus_resources(&inline))
            } else {
                None
            };
            if mixed {
                wait(
                    &ui,
                    || {
                        article
                            .output_identity(&connection.updates.borrow())
                            .is_some()
                    },
                    "mixed wide geometry did not ACK before account focus",
                )
                .await?;
                require(
                    connection.updates.borrow().account.as_deref() == Some("StellaTest"),
                    "mixed own role was not confirmed by Session",
                )?;
                let view = connection.updates.borrow().clone();
                let id = article
                    .output_identity(&view)
                    .ok_or("mixed focus source missing")?;
                require(
                    article.focus_reply(id, view.stamp(), Some(40)),
                    "mixed Bob highlight refused acknowledged reply",
                )?;
                wait(
                    &ui,
                    || {
                        article
                            .output_identity(&connection.updates.borrow())
                            .is_some()
                            && article.core.borrow().last_displayed().is_some_and(|p| {
                                p.reply_navigation.author_rows == [41, 44]
                                    && p.reply_navigation.mine_rows == [39, 42]
                                    && p.reply_navigation.focused_rows == [40, 43]
                            })
                    },
                    "mixed shared role index not installed",
                )
                .await?;
            }
            ui.set_zoom(1.5);
            wait(
                &ui,
                || {
                    article
                        .output_identity(&connection.updates.borrow())
                        .is_some()
                },
                "zoom geometry did not ACK",
            )
            .await?;
            let native = article.view.borrow().clone().ok_or("reader missing")?;
            if mixed {
                native.navigate(Intent::Last);
            } else {
                native.navigate(Intent::Lines {
                    down: true,
                    count: 15,
                });
            }
            wait(
                &ui,
                || {
                    article
                        .output_identity(&connection.updates.borrow())
                        .is_some()
                },
                "middle reading anchor missing",
            )
            .await?;
            if mixed {
                ui.invoke_reply_navigation_action(0);
                wait(
                    &ui,
                    || {
                        let ready = article.output_identity(&connection.updates.borrow()).is_some();
                        if ready && ui.get_reply_side_marks().row_count() != 4 {
                            // Resource and toolbar reflow preserve a row anchor,
                            // so position at the tail only after their geometry ACK.
                            native.navigate(Intent::Last);
                        }
                        ready && ui.get_reply_side_marks().row_count() == 4
                    },
                    "mixed visible role marks not ready",
                )
                .await.map_err(|e| format!("{e}: marks={},open={},ready={},geometry={},anchor={:?},layout={:?},scope={:?},index={:?}",ui.get_reply_side_marks().row_count(),ui.get_reply_navigation_open(),ui.get_reply_navigation_ready(),native.geometry().is_some(),native.anchor.get(),native.layout.get(),native.scope.get(),article.core.borrow().last_displayed().map(|p| &p.reply_navigation)))?;
                ui.invoke_reply_navigation_action(6);
                ui.invoke_reply_navigation_action(3);
                wait(
                    &ui,
                    || {
                        article
                            .output_identity(&connection.updates.borrow())
                            .is_some()
                    },
                    "mixed focused reply navigation not ready",
                )
                .await?;
                require(
                    article.core.borrow().last_displayed().is_some_and(|p| {
                        p.frame
                            .glyphs
                            .iter()
                            .any(|g| g.row == 40 && g.col == 3 && g.left.bg == 4)
                    }),
                    "mixed Bob reply highlight not blue",
                )?;
            }
            let anchor = native.anchor.get();
            let retained_frame = native.source.borrow().frame.clone();
            eprintln!(
                "updates anchor initial={anchor:?},scope={:?},progress={:?},layout={:?}",
                native.scope.get(),
                article.progress(),
                native.layout.get()
            );
            let initial_epoch = connection.updates.borrow().input_epoch;
            ui.invoke_article_updates_action(0);
            require(
                ui.get_article_updates_enabled(),
                "explicit update action was rejected",
            )?;
            if matches!(case, "cancel" | "blur") {
                wait(
                    &ui,
                    || connection.updates.borrow().input_epoch > initial_epoch,
                    "refresh never sent owned Q",
                )
                .await?;
                if case == "cancel" {
                    ui.invoke_article_updates_action(0);
                } else {
                    verification::foreground(&ui, false);
                }
                for _ in 0..100 {
                    if ui.get_article_updates_recovery() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                require(
                    ui.get_article_updates_recovery() && !ui.get_article_updates_enabled(),
                    "cancel/background did not retain reading recovery",
                )?;
                let view = connection.updates.borrow().clone();
                for bytes in [b"\x1b[D".as_slice(), b"\x1b[C", b"\r", b"\x7f"] {
                    require(
                        article.before_input(&view, bytes),
                        "retained relative key escaped to station",
                    )?;
                }
                for key in ["ArrowLeft", "ArrowRight", "Enter", "Backspace"] {
                    require(
                        article.retained_key(&pttstella::vim::Key {
                            key: key.into(),
                            code: String::new(),
                            ctrl: false,
                            alt: false,
                            shift: false,
                            repeat: false,
                        }),
                        "native named-key admission escaped retained protection",
                    )?;
                }
                tokio::time::sleep(Duration::from_millis(6300)).await;
                require(
                    native.source.borrow().frame == retained_frame,
                    "late query replaced retained original frame",
                )?;
                require(
                    same_anchor(native.anchor.get(), anchor)
                        && article.progress().is_some_and(|(_, first, last, state)| {
                            first == 1 && last == 45 && state == Some(LoadState::Complete)
                        }),
                    &format!(
                        "cancel or late query replaced retained article/anchor: before={anchor:?},after={:?},scope={:?},progress={:?},layout={:?}",
                        native.anchor.get(),
                        native.scope.get(),
                        article.progress(),
                        native.layout.get()
                    ),
                )?;
                verification::foreground(&ui, true);
                if mixed {
                    require(
                        ui.get_reply_side_marks().row_count() == 4
                            && ui.get_reply_navigation_ready(),
                        "retained mixed role marks or local navigation disappeared",
                    )?;
                    require(
                        resources.as_ref().is_none_or(|r| r.retained(&inline)),
                        "retained mixed PNG/GIF originals or wide choice changed",
                    )?;
                    let view = connection.updates.borrow().clone();
                    require(
                        article.export_output(&view, None).is_err(),
                        "retained local roles accidentally authorized stale export",
                    )?;
                    ui.invoke_reply_navigation_action(3);
                    require(
                        ui.get_reply_navigation_count()
                            == pttstella::msg!(
                                "reply-navigation-count",
                                current = 2i64,
                                total = 2i64
                            ),
                        "retained focused replies did not navigate locally",
                    )?;
                    require(
                        article.core.borrow().last_displayed().is_some_and(|p| {
                            p.frame
                                .glyphs
                                .iter()
                                .any(|g| g.row == 40 && g.col == 3 && g.left.bg == 4)
                        }),
                        "retained mixed Bob highlight disappeared",
                    )?;
                }
                let tiles = ui.get_article_tiles();
                let tile_details=tiles.iter().map(|t| {
                    let pixels=t.image.to_rgba8();
                    serde_json::json!({"y":t.y,"height":t.height,"pixels":pixels.as_ref().map(|p|(p.width(),p.height())),"ink":pixels.as_ref().map(|p|p.as_bytes().chunks_exact(4).filter(|rgba|rgba[0]>0||rgba[1]>0||rgba[2]>0).count())})
                }).collect::<Vec<_>>();
                let diagnostic = serde_json::json!({"tiles":tile_details,"geometry":native.geometry().is_some(),"layout":format!("{:?}",native.layout.get()),"painted":format!("{:?}",native.painted.borrow()),"row_cache":native.row_cache.borrow().len(),"timer":native.timer.running(),"anchor":format!("{:?}",native.anchor.get()),"offset_y":ui.get_article_offset_y(),"line_width":ui.get_article_line_width(),"height":ui.get_article_body_height(),"notice":ui.get_input_notice().as_str()});
                std::fs::write(
                    directory.join("retained-pixels.json"),
                    serde_json::to_vec_pretty(&diagnostic).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                eprintln!("retained pixels: {diagnostic}");
                let visible = tiles
                    .iter()
                    .filter(|t| t.y >= 0. && t.y + t.height <= ui.get_article_body_height())
                    .collect::<Vec<_>>();
                require(
                    native.geometry().is_some()
                        && !visible.is_empty()
                        && visible.iter().all(|t| {
                            t.image.to_rgba8().is_some_and(|p| {
                                p.as_bytes()
                                    .chunks_exact(4)
                                    .any(|rgba| rgba[0] > 0 || rgba[1] > 0 || rgba[2] > 0)
                            })
                        }),
                    "retained visible text rows lost their raster pixels",
                )?;
                verification::capture(&ui, &directory, "retained-reading.png")?;
                if native_check == "1" {
                    std::fs::write(directory.join("native-ready.json"),b"{\"prepared_with_toolkit\":true,\"purpose\":\"actual OS screenshot of retained pixels\"}").map_err(|e|e.to_string())?;
                    return Ok(());
                }
                ui.invoke_article_updates_action(2);
                wait(
                    &ui,
                    || !ui.get_article_active() && article.progress().is_none(),
                    "explicit station recovery did not clear cached article",
                )
                .await?;
                require(
                    connection.updates.borrow().context().kind() == "continue_prompt",
                    "late station query not exposed after explicit recovery",
                )?;
                verification::wait(
                    || {
                        article
                            .output_identity(&connection.updates.borrow())
                            .is_some()
                    },
                    "restored terminal never reached a current geometry ACK",
                )
                .await
                .map_err(|e| {
                    format!(
                        "{e}; core={},raster={}x{},offset={},{}",
                        article.diagnostics(),
                        ui.get_raster_width(),
                        ui.get_raster_height(),
                        ui.get_raster_left(),
                        ui.get_raster_top()
                    )
                })?;
                verification::wait(
                    || {
                        ui.get_raster_top() + ui.get_caret_top() >= -1.
                            && ui.get_raster_top() + ui.get_caret_top() + ui.get_caret_height()
                                <= ui.get_view_height() + 1.
                            && ui.get_raster_left() + ui.get_caret_left() >= -1.
                            && ui.get_raster_left()
                                + ui.get_caret_left()
                                + ui.get_raster_width() / 80.
                                <= ui.get_view_width() + 1.
                    },
                    "station recovery did not settle around its current cursor",
                )
                .await
                .map_err(|e| {
                    format!(
                        "{e}; raster={}x{},offset={},{}; caret={},{}+{}; view={}x{}",
                        ui.get_raster_width(),
                        ui.get_raster_height(),
                        ui.get_raster_left(),
                        ui.get_raster_top(),
                        ui.get_caret_left(),
                        ui.get_caret_top(),
                        ui.get_caret_height(),
                        ui.get_view_width(),
                        ui.get_view_height()
                    )
                })?;
                let left = ui.get_raster_left();
                let top = ui.get_raster_top();
                require(
                    left >= (ui.get_view_width() - ui.get_raster_width()).min(0.) - 1.
                        && top >= (ui.get_view_height() - ui.get_raster_height()).min(0.) - 1.,
                    "recovery left the terminal scrolled beyond its installed raster",
                )?;
                require(
                    top + ui.get_caret_top() >= -1.
                        && top + ui.get_caret_top() + ui.get_caret_height()
                            <= ui.get_view_height() + 1.
                        && left + ui.get_caret_left() >= -1.
                        && left + ui.get_caret_left() + ui.get_raster_width() / 80.
                            <= ui.get_view_width() + 1.,
                    &format!(
                        "recovery did not reveal the installed station cursor at the current zoom: raster={}x{},offset={left},{top},caret={},{}+{},view={}x{}",
                        ui.get_raster_width(),
                        ui.get_raster_height(),
                        ui.get_caret_left(),
                        ui.get_caret_top(),
                        ui.get_caret_height(),
                        ui.get_view_width(),
                        ui.get_view_height()
                    ),
                )?;
                if native_check == "2" {
                    std::fs::write(directory.join("native-ready.json"),b"{\"prepared_with_toolkit\":true,\"purpose\":\"actual OS screenshot of unassisted station recovery\"}").map_err(|e|e.to_string())?;
                    return Ok(());
                }
            } else {
                wait(
                    &ui,
                    || {
                        article.progress().is_some_and(|(_, _, last, state)| {
                            last == 48 && state == Some(LoadState::Complete)
                        }) && article
                            .output_identity(&connection.updates.borrow())
                            .is_some()
                            && !article
                                .core
                                .borrow()
                                .updates_status()
                                .is_some_and(|s| s.busy)
                    },
                    "first append did not reach complete native ACK",
                )
                .await
                .map_err(|e| {
                    format!(
                        "{e}; core={},updates={:?},ready={},focused={}",
                        article.diagnostics(),
                        article.core.borrow().updates_status(),
                        ui.get_article_reading_ready(),
                        ui.get_article_focused()
                    )
                })?;
                if !same_anchor(native.anchor.get(), anchor) {
                    verification::capture(&ui, &directory, "first-anchor-change.png")?;
                    return Err(format!(
                        "first refresh moved local reading anchor: before={anchor:?},after={:?},scope={:?},progress={:?},layout={:?}",
                        native.anchor.get(),
                        native.scope.get(),
                        article.progress(),
                        native.layout.get()
                    ));
                }
                require(
                    ui.get_article_updates_unread() == 3,
                    "first append unread count incorrect",
                )?;
                let paused_epoch = connection.updates.borrow().input_epoch;
                if mixed {
                    ui.invoke_find_action(0);
                    ui.invoke_find_edit("REPLY".into());
                    verification::wait(
                        || article.core.borrow().find().reply(0).total == 9,
                        "mixed find did not match nine projected replies",
                    )
                    .await?;
                } else {
                    ui.set_native_pointer_down(true);
                }
                for _ in 0..300 {
                    verification::foreground(&ui, true);
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                require(
                    connection.updates.borrow().input_epoch == paused_epoch,
                    "interaction did not suspend five-second refresh",
                )?;
                wait(
                    &ui,
                    || {
                        article
                            .output_identity(&connection.updates.borrow())
                            .is_some()
                    },
                    "interaction layout did not ACK before refresh resumed",
                )
                .await?;
                // Finding deliberately reveals its first match. The next append
                // must preserve that new user position, not the pre-search one.
                let second_anchor = native.anchor.get();
                if mixed {
                    ui.invoke_find_action(3);
                } else {
                    ui.set_native_pointer_down(false);
                }
                wait(
                    &ui,
                    || {
                        article.progress().is_some_and(|(_, _, last, state)| {
                            last == 51 && state == Some(LoadState::Complete)
                        }) && article
                            .output_identity(&connection.updates.borrow())
                            .is_some()
                            && !article
                                .core
                                .borrow()
                                .updates_status()
                                .is_some_and(|s| s.busy)
                    },
                    "second append did not reach complete native ACK",
                )
                .await?;
                require(
                    same_anchor(native.anchor.get(), second_anchor),
                    &format!(
                        "second refresh moved local reading anchor: before={second_anchor:?},after={:?},scope={:?},progress={:?},layout={:?}",
                        native.anchor.get(),
                        native.scope.get(),
                        article.progress(),
                        native.layout.get()
                    ),
                )?;
                require(
                    ui.get_article_updates_unread() == 6,
                    "two appends did not retain six unread replies",
                )?;
                if let Some(original) = original.as_ref() {
                    require(
                        inline.state.borrow().resources.values().any(|r| {
                            r.original
                                .as_ref()
                                .is_some_and(|r| Arc::ptr_eq(r, original))
                        }),
                        "refresh lost the decoded original image",
                    )?;
                }
                if mixed {
                    require(
                        article.core.borrow().last_displayed().is_some_and(|p| {
                            p.reply_navigation.author_rows == [41, 44, 47, 50]
                                && p.reply_navigation.mine_rows == [39, 42, 45, 48]
                                && p.reply_navigation.focused_rows == [40, 43, 46, 49]
                        }),
                        "mixed updated role index did not extend all three groups",
                    )?;
                    require(
                        resources.as_ref().is_none_or(|r| r.retained(&inline)),
                        "mixed updates or search changed PNG/GIF originals/wide choice",
                    )?;
                }
                let before_unchanged = connection.updates.borrow().input_epoch;
                let pages_before = article.requests.get();
                wait(
                    &ui,
                    || {
                        connection.updates.borrow().input_epoch >= before_unchanged + 8
                            && article
                                .output_identity(&connection.updates.borrow())
                                .is_some()
                            && article
                                .core
                                .borrow()
                                .updates_status()
                                .is_some_and(|s| !s.busy)
                    },
                    "unchanged refresh did not finish its verified tail check",
                )
                .await?;
                require(
                    article.requests.get() == pages_before && ui.get_article_updates_unread() == 6,
                    "unchanged refresh fetched pages or changed unread replies",
                )?;
                require(
                    same_anchor(native.anchor.get(), second_anchor),
                    "unchanged refresh moved the reading anchor",
                )?;
                verification::capture(&ui, &directory, "new-replies.png")?;
                ui.invoke_article_updates_action(1);
                wait(
                    &ui,
                    || {
                        article
                            .output_identity(&connection.updates.borrow())
                            .is_some()
                    },
                    "jump-to-new geometry missing",
                )
                .await?;
                require(
                    ui.get_article_updates_unread() == 0
                        && ui.get_article_document_height()
                            - ui.get_article_body_height()
                            - ui.get_article_offset_y()
                            <= 2.,
                    "new replies action did not reach bottom/clear unread",
                )?;
                ui.invoke_article_updates_action(0);
                require(
                    !ui.get_article_updates_enabled(),
                    "pause action left updates enabled",
                )?;
                let stopped_epoch = connection.updates.borrow().input_epoch;
                for _ in 0..300 {
                    verification::foreground(&ui, true);
                    ui.invoke_focus_article();
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                require(
                    connection.updates.borrow().input_epoch == stopped_epoch,
                    "paused updates sent another refresh",
                )?;
                article.preferences_changed(&pttstella::settings::AppSettings {
                    article_mode: pttstella::settings::ArticleMode::Terminal,
                    ..Default::default()
                })?;
                wait(
                    &ui,
                    || {
                        !ui.get_article_updates_available()
                            && !ui.get_article_updates_visible()
                            && !ui.get_article_updates_enabled()
                    },
                    "terminal mode retained update controls",
                )
                .await?;
            }
            std::fs::write(directory.join("updates-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"case":case,"mixed":mixed,"with_image":with_image,"strict_shared_peer":true,"actual_OS_input":false,"anchor_preserved":true,"explicit_opt_in":true,"cancel_recovery":matches!(case,"cancel"|"blur")})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            Ok(())
        },
    )
}
