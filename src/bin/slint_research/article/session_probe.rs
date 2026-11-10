//! Exact local station workload; toolkit foreground is explicitly synthetic.
use super::session::ArticleSession;
use crate::{
    TerminalWindow,
    verification::{self, Verification, require, wait},
};
use pttstella::{
    article_cache::LoadState,
    connection::{Connection, Phase},
    navigation::Context,
    selection::{self, Selection},
};
use std::{error::Error, path::PathBuf, rc::Rc, time::Duration};
async fn until(mut condition: impl FnMut() -> bool, message: &str) -> Result<(), String> {
    for _ in 0..2250 {
        if condition() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Err(message.into())
}
pub fn start(
    ui: &TerminalWindow,
    owner: Rc<ArticleSession>,
    connection: Rc<Connection>,
    output: Option<Rc<crate::output::Output>>,
    paste: Option<Rc<crate::paste::Paste>>,
    directory: PathBuf,
    case: crate::session::Probe,
) -> Result<Verification, Box<dyn Error>> {
    if matches!(
        case,
        crate::session::Probe::ArticleFind | crate::session::Probe::ArticleFindTerminal
    ) {
        return super::find_probe::start(
            ui,
            owner,
            connection,
            output.ok_or("find output missing")?,
            directory,
        );
    }
    if case == crate::session::Probe::ArticleKeepAlive {
        return crate::keep_alive::probe::start(
            ui,
            owner,
            connection,
            output.ok_or("article keepalive output missing")?,
            directory,
        );
    }
    if case == crate::session::Probe::ArticlePaste {
        return crate::paste::probe::start(
            ui,
            owner,
            connection,
            output.ok_or("article paste output missing")?,
            paste.ok_or("article paste owner missing")?,
            directory,
        );
    }
    if case == crate::session::Probe::ArticleOutput {
        return super::output_probe::start(
            ui,
            owner,
            connection,
            output.ok_or("article output owner missing")?,
            directory,
        );
    }
    let terminal = case == crate::session::Probe::ArticleTerminal;
    let long = case == crate::session::Probe::ArticleLong;
    let stall = case == crate::session::Probe::ArticleStall;
    let input = case == crate::session::Probe::ArticleInput;
    Verification::start(
        ui,
        directory,
        "article-session",
        move |ui, directory| async move {
            wait(
                || {
                    verification::foreground(&ui, true);
                    let view = connection.updates.borrow();
                    view.phase == Phase::Connected
                        && !view.output_pending
                        && matches!(view.context(), Context::ArticleReading { .. })
                },
                "article station first frame missing",
            )
            .await?;
            if std::env::var_os("PTTSTELLA_SLINT_STARTUP_BOUNDARY_PROBE").is_some()
                && case == crate::session::Probe::ArticleFull
            {
                wait(
                    || owner.diagnostics()["article_boundary_navigation"] == true,
                    "saved startup boundary preference missing from first native projection",
                )
                .await?;
                std::fs::write(
                    directory.join("startup-boundary.json"),
                    serde_json::to_vec_pretty(&owner.diagnostics()).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
            }
            if terminal {
                require(output.is_some(), "terminal output owner missing")?;
                wait(
                    || {
                        verification::foreground(&ui, true);
                        output
                            .as_ref()
                            .is_some_and(|output| output.current(&ui).is_some())
                    },
                    "terminal first paint pending",
                )
                .await?;
                tokio::time::sleep(Duration::from_millis(900)).await;
                require(
                    owner.requests.get() == 0
                        && !ui.get_article_active()
                        && owner.progress().is_none(),
                    "terminal mode started article loading",
                )?;
                verification::foreground(&ui, true);
                require(
                    ui.invoke_grid_wheel(-40.),
                    "terminal manual page-down was rejected",
                )?;
                wait(
                    || {
                        matches!(
                            connection.updates.borrow().context(),
                            Context::ArticleReading { first_line: 23, .. }
                        )
                    },
                    "terminal manual page did not arrive",
                )
                .await?;
                wait(
                    || {
                        output
                            .as_ref()
                            .is_some_and(|output| output.current(&ui).is_some())
                    },
                    "terminal second paint pending",
                )
                .await?;

                require(
                    owner.requests.get() == 0 && !ui.get_article_active(),
                    "terminal mode created a cached viewport",
                )?;
                verification::capture(&ui, &directory, "terminal-second-page.png")?;
            } else if input {
                wait(
                    || owner.requests.get() == 1,
                    "article input first turn missing",
                )
                .await?;
                // Resume while the delayed page is still in flight. A second
                // PageDown here would be consumed by the peer's raw-q check.
                ui.invoke_article_load_action(false);
                ui.invoke_article_load_action(true);
                until(
                    || {
                        owner.progress().is_some_and(|(_, _, last, state)| {
                            last == 45 && state == Some(LoadState::Complete)
                        }) && !owner.core.borrow().dirty()
                            && !owner.core.borrow().has_in_flight()
                    },
                    "rapid resume did not finish its original page",
                )
                .await?;
                require(
                    owner.requests.get() == 1,
                    "rapid resume duplicated a station turn",
                )?;
                let generation = connection.updates.borrow().generation;
                let before = ui.get_article_offset_y();
                owner.route_input(generation, b"j".to_vec())?;
                require(
                    ui.get_article_offset_y() > before,
                    "native j did not move the cache",
                )?;
                owner.route_input(generation, b"\x1b[H".to_vec())?;
                require(
                    ui.get_article_offset_y() == 0.,
                    "native Home did not reach the cache start",
                )?;
                owner.route_input(generation, b"\x1b[F".to_vec())?;
                owner.route_input(generation, b" ".to_vec())?;
                require(
                    owner
                        .route_input(generation.wrapping_add(1), b"q".to_vec())
                        .is_err()
                        && ui.get_article_active(),
                    "stale input changed the current surface",
                )?;
                // The typed route above is not an OS focus event. Complete
                // native layout and explicitly activate the synthetic field
                // before simulating a preedit delivered to that field.
                verification::capture(&ui, &directory, "before-composition.png")?;
                wait(
                    || {
                        verification::foreground(&ui, true);
                        ui.invoke_focus_article();
                        ui.get_article_focused()
                            && output.as_ref().is_some_and(|o| o.current(&ui).is_some())
                    },
                    "synthetic article input did not acquire focus",
                )
                .await?;
                owner.begin_composition();
                require(
                    ui.get_article_active() && ui.get_article_focused(),
                    "composition did not retain the real article input field",
                )?;
                owner.end_composition();
                wait(
                    || {
                        ui.get_article_active()
                            && ui.get_article_focused()
                            && !owner.core.borrow().dirty()
                            && !owner.core.borrow().has_in_flight()
                    },
                    "cancelled preedit did not restore the article",
                )
                .await?;
                require(
                    owner.requests.get() == 1,
                    "composition handoff requested another page",
                )?;
                owner.route_input(generation, b"q".to_vec())?;
                wait(
                    || {
                        matches!(
                            connection.updates.borrow().context(),
                            Context::MainMenu { .. }
                        )
                    },
                    "raw q did not reach the station",
                )
                .await?;
                wait(
                    || {
                        !ui.get_article_active()
                            && !ui.get_article_station()
                            && output.as_ref().is_some_and(|o| o.current(&ui).is_some())
                    },
                    "leaving reading did not retire the raw hold and paint the menu",
                )
                .await?;
                let revision = connection.updates.borrow().frame.revision;
                let station_size = (ui.get_view_width(), ui.get_view_height());
                owner.route_input(
                    generation,
                    pttstella::input::encode("中").map_err(|e| e.to_string())?,
                )?;
                require(
                    !ui.get_article_station()
                        && (ui.get_view_width(), ui.get_view_height()) == station_size,
                    "ordinary station input flashed a return bar or resized the viewport",
                )?;
                wait(
                    || {
                        connection.updates.borrow().frame.revision > revision
                            && !connection.updates.borrow().output_pending
                    },
                    "Chinese commit response missing",
                )
                .await?;
                tokio::time::sleep(Duration::from_millis(800)).await;
                require(
                    owner.requests.get() == 1 && owner.progress().is_none(),
                    "raw handoff revived the old article loader",
                )?;
                require(
                    ui.get_input_notice().is_empty(),
                    "hidden article emitted a stale raster error",
                )?;
                verification::capture(&ui, &directory, "raw-station.png")?;
                ui.invoke_article_resume_reading();
                require(
                    !ui.get_article_station(),
                    "explicit reading restore stayed in raw mode",
                )?;
            } else if stall {
                let started = std::time::Instant::now();
                wait(
                    || {
                        verification::foreground(&ui, true);
                        owner.requests.get() == 1
                    },
                    "stalled article command missing",
                )
                .await?;
                until(
                    || {
                        verification::foreground(&ui, true);
                        owner.progress().is_some_and(|(_, _, last, state)| {
                            last == 23 && state == Some(LoadState::Stopped)
                        })
                    },
                    "stalled article did not hit its response deadline",
                )
                .await?;
                require(
                    started.elapsed() >= Duration::from_secs(5),
                    "stalled article stopped before response deadline",
                )?;
                tokio::time::sleep(Duration::from_millis(250)).await;
                require(owner.requests.get() == 1, "stalled article retried")?;
                verification::capture(&ui, &directory, "stopped.png")?;
            } else {
                wait(
                    || owner.requests.get() >= 1,
                    "native article never requested next page",
                )
                .await?;
                if !long {
                    ui.invoke_article_load_action(false);
                    verification::foreground(&ui, false);
                    wait(
                        || {
                            owner.progress().is_some_and(|(_, _, last, state)| {
                                last >= 45 && state == Some(LoadState::Cancelled)
                            })
                        },
                        "late cancelled page was not retained",
                    )
                    .await?;
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    require(
                        owner.requests.get() == 1,
                        "cancel or background state sent another page",
                    )?;
                    verification::foreground(&ui, true);
                    tokio::time::sleep(Duration::from_millis(150)).await;
                    require(
                        owner.requests.get() == 1,
                        "foreground resumed cancelled loading",
                    )?;
                    ui.invoke_article_load_action(true);
                    wait(
                        || {
                            verification::foreground(&ui, true);
                            ui.invoke_focus_article();
                            owner.requests.get() >= 2
                        },
                        "explicit resume did not fetch",
                    )
                    .await
                    .map_err(|error| format!("{error}; state={}", owner.diagnostics()))?;
                    verification::foreground(&ui, false);
                    wait(
                        || owner.progress().is_some_and(|(_, _, last, _)| last >= 67),
                        "paused response was not ingested",
                    )
                    .await?;
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    require(owner.requests.get() == 2, "paused UI sent a following page")?;
                    verification::foreground(&ui, true);
                }
                until(
                    || {
                        // This is a synthetic toolkit run. Keep its explicit
                        // activation after testing background pause above.
                        verification::foreground(&ui, true);
                        ui.invoke_focus_article();
                        owner.progress().is_some_and(|(_, _, last, state)| {
                            last == if long { 837 } else { 441 }
                                && state == Some(LoadState::Complete)
                        }) && owner.core.borrow().pending_identity().is_none()
                            && !owner.core.borrow().dirty()
                    },
                    "full article did not finish and paint",
                )
                .await?;
                require(
                    owner.requests.get() == if long { 37 } else { 19 },
                    "unexpected station request count",
                )?;
                {
                    let core = owner.core.borrow();
                    let packet = core
                        .last_displayed()
                        .ok_or("article has no final acknowledged projection")?;
                    let lines = if long { 837 } else { 441 };
                    for (row, number) in [(0, 1), (lines - 1, lines)] {
                        require(
                            selection::extract(&packet.frame, &Selection::Line { row })?
                                .text
                                .trim_end()
                                == format!("Full article synthetic line {number:04}"),
                            "article endpoint text differs",
                        )?;
                    }
                    require(
                        packet.source_frame.as_ref().is_some_and(|f| f.rows == 24),
                        "PNG source no longer has 24 rows",
                    )?;
                    if long {
                        let glyph = packet
                            .frame
                            .glyphs
                            .iter()
                            .find(|g| g.row == 660 && g.col == 0)
                            .ok_or("historic ink row missing")?;
                        require(
                            glyph.left.fg == 3 && glyph.left.bright,
                            "leading ink reset lost historical colour",
                        )?;
                    }
                }
                ui.invoke_article_navigate(4);
                tokio::time::sleep(Duration::from_millis(30)).await;
                verification::capture(&ui, &directory, "history-top.png")?;
                ui.invoke_article_navigate(5);
                tokio::time::sleep(Duration::from_millis(30)).await;
                verification::capture(&ui, &directory, "history-end.png")?;
                tokio::time::sleep(Duration::from_millis(200)).await;
                require(
                    owner.requests.get() == if long { 37 } else { 19 },
                    "local history refetched station pages",
                )?;
            }
            let mut record = serde_json::json!({"scope":"synthetic local peer plus native toolkit; no trusted OS input or real PTT","terminal":terminal,"long":long,"stall":stall,"requests":owner.requests.get(),"progress":owner.progress().map(|(id,first,last,state)|serde_json::json!({"id":id,"first":first,"last":last,"state":state})),"notice":ui.get_input_notice().as_str(),"article_loop_joined":true});
            if input {
                record["native_input_route"] = serde_json::json!({"rapid_resume_requests":1,"raw_q":1,"uao_chinese_commits":1,"cancelled_preedit_bytes":0,"trusted_os_ime":false});
            }
            if stall {
                verification::foreground(&ui, false);
                let view = connection.updates.borrow().clone();
                {
                    let mut core = owner.core.borrow_mut();
                    core.display_changed();
                    require(
                        core.queue(&view, false, &Default::default()).is_some(),
                        "stale policy fixture could not queue old projection",
                    )?;
                    core.replace_rules(
                        pttstella::filters::Rules::default().edit("Blocked", "", false)?,
                    );
                }
                ui.invoke_article_session_updated();
                wait(
                    || {
                        let core = owner.core.borrow();
                        !core.has_in_flight()
                            && !core.dirty()
                            && core
                                .last_displayed()
                                .is_some_and(|p| p.presentation == core.presentation())
                    },
                    "policy change left an old in-flight projection blocking native paint",
                )
                .await?;
                require(
                    owner.requests.get() == 1,
                    "policy change fetched while backgrounded",
                )?;
                record["stale_policy_ack_retired"] = true.into();
            }
            owner.shutdown().await;
            require(owner.stopped(), "article event loop did not finish cleanup")?;
            std::fs::write(
                directory.join("result.json"),
                serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        },
    )
}
