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
use slint::ComponentHandle;
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
    use crate::session::Probe::{ArticleSwitch, ArticleSwitchTerminal};
    if matches!(case, ArticleSwitch | ArticleSwitchTerminal) {
        return Verification::start(ui, directory, "article-switch", move |ui, directory| {
            verify_switch(
                ui,
                directory,
                owner,
                connection,
                case == ArticleSwitchTerminal,
            )
        });
    }
    if case == crate::session::Probe::ArticleSegmented {
        return Verification::start(ui, directory, "segmented-load", move |ui, directory| {
            verify_segment(ui, directory, owner)
        });
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
                let left = ui.get_terminal_origin_x() + ui.get_raster_left();
                let top = ui.get_terminal_origin_y() + ui.get_raster_top();
                for (col, row, expected) in [
                    (2., 10., crate::PointerKind::Back),
                    (60., 0.25, crate::PointerKind::First),
                    (60., 5., crate::PointerKind::PreviousPage),
                    (60., 16., crate::PointerKind::NextPage),
                    (60., 22.75, crate::PointerKind::Last),
                ] {
                    ui.window()
                        .dispatch_event(slint::platform::WindowEvent::PointerMoved {
                            position: slint::LogicalPosition::new(
                                left + col * ui.get_raster_width() / 80.,
                                top + row * ui.get_raster_height() / 24.,
                            ),
                        });
                    require(
                        ui.get_pointer_cursor() == expected,
                        &format!(
                            "terminal hover chose {:?} instead of {expected:?}",
                            ui.get_pointer_cursor()
                        ),
                    )?;
                }
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
                    wait(
                        || {
                            verification::foreground(&ui, true);
                            ui.invoke_focus_article();
                            let view = connection.updates.borrow();
                            owner.core.borrow().cached_reading(view.stamp(), &view)
                                && !owner.core.borrow().dirty()
                                && !owner.core.borrow().has_in_flight()
                        },
                        "cancelled article's last page was not rendered",
                    )
                    .await?;
                    ui.invoke_article_navigate(5);
                    let cached_edge = ui.get_article_offset_y();
                    require(cached_edge > 0., "could not reach cached article edge")?;
                    require(
                        owner.requests.get() == 1,
                        "first End requested station input",
                    )?;
                    owner.local_intent(pttstella::navigation::Intent::NextPage);
                    owner.local_intent(pttstella::navigation::Intent::NextPage);
                    require(
                        owner.requests.get() == 2,
                        "edge did not request exactly one page",
                    )?;
                    wait(
                        || {
                            owner.progress().is_some_and(|(_, _, last, state)| {
                                last == 67 && state == Some(LoadState::Cancelled)
                            }) && ui.get_article_offset_y() > cached_edge
                        },
                        "manual page did not advance the viewport after paint",
                    )
                    .await
                    .map_err(|error| {
                        format!(
                            "{error}; previous offset={cached_edge}, current offset={}, state={}",
                            ui.get_article_offset_y(),
                            owner.diagnostics()
                        )
                    })?;
                    tokio::time::sleep(Duration::from_millis(150)).await;
                    require(
                        owner.requests.get() == 2,
                        "manual paging restarted preloading",
                    )?;

                    ui.invoke_article_load_action(true);
                    wait(
                        || {
                            verification::foreground(&ui, true);
                            ui.invoke_focus_article();
                            owner.requests.get() >= 3
                        },
                        "explicit resume did not fetch",
                    )
                    .await
                    .map_err(|error| format!("{error}; state={}", owner.diagnostics()))?;
                    verification::foreground(&ui, false);
                    wait(
                        || owner.progress().is_some_and(|(_, _, last, _)| last >= 89),
                        "paused response was not ingested",
                    )
                    .await?;
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    require(owner.requests.get() == 3, "paused UI sent a following page")?;
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
            if case == crate::session::Probe::ArticleFull {
                record["cancelled_edge_fetch"] = serde_json::json!({
                    "manual_requests": 1, "applied_after_paint": true, "preload_remained_cancelled": true
                });
            }
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

async fn verify_segment(
    ui: TerminalWindow,
    directory: PathBuf,
    owner: Rc<ArticleSession>,
) -> Result<(), String> {
    let final_line = 4181;
    let ready = || {
        verification::foreground(&ui, true);
        ui.invoke_focus_article();
        ui.get_article_reading_ready() && !owner.core.borrow().has_in_flight()
    };
    until(
        || {
            ready()
                && ui.get_article_load_next_segment()
                && owner.progress().is_some_and(|(_, first, last, state)| {
                    first == 1 && (4000..=4096).contains(&last) && state == Some(LoadState::Limit)
                })
        },
        "the first article segment did not reach its rendered cache boundary",
    )
    .await
    .map_err(|error| format!("{error}; state={}", owner.diagnostics()))?;
    let (old_id, _, first_segment_end, _) = owner.progress().ok_or("first segment missing")?;
    let boundary_requests = owner.requests.get();
    require(
        first_segment_end == 1 + 22 * boundary_requests,
        "first segment requested unexpected pages",
    )?;
    let initial_y = ui.get_article_offset_y();
    require(
        owner.local_intent(pttstella::navigation::Intent::NextPage),
        "article input did not consume PageDown",
    )?;
    require(
        ui.get_article_offset_y() > initial_y,
        "PageDown did not advance within the cached segment",
    )?;
    ui.invoke_article_navigate(5);
    let end_y = ui.get_article_offset_y();
    require(
        (end_y - (ui.get_article_document_height() - ui.get_article_body_height())).abs() < 0.01,
        "End did not reach the cached segment boundary",
    )?;
    tokio::time::sleep(Duration::from_millis(150)).await;
    require(
        owner.requests.get() == boundary_requests
            && owner.progress().is_some_and(|(id, _, _, _)| id == old_id),
        "navigation fetched before the reader reached the segment boundary",
    )?;
    require(ready(), "end-of-segment viewport was not ready for input")?;
    require(
        owner.local_intent(pttstella::navigation::Intent::NextPage),
        "article input did not consume PageDown",
    )?;
    until(
        || {
            ready()
                && owner.progress().is_some_and(|(id, first, last, state)| {
                    id != old_id
                        && first == first_segment_end
                        && last == final_line
                        && state == Some(LoadState::Complete)
                })
                && !owner.core.borrow().dirty()
        },
        "PageDown at the cached segment boundary did not finish the next segment",
    )
    .await
    .map_err(|error| format!("{error}; state={}", owner.diagnostics()))?;
    let rows = {
        let core = owner.core.borrow();
        let packet = core.last_displayed().ok_or("final segment paint missing")?;
        [0, final_line - first_segment_end]
            .into_iter()
            .map(|row| {
                selection::extract(&packet.frame, &Selection::Line { row })
                    .map(|copy| copy.text.trim_end().to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    require(
        rows == [
            format!("Full article synthetic line {first_segment_end:04}"),
            format!("Full article synthetic line {final_line:04}"),
        ],
        "the next segment lost the overlap row or the final row",
    )?;
    require(
        owner.requests.get() == 189,
        "segment boundary duplicated a station request",
    )?;
    verification::capture(&ui, &directory, "continued-segment.png")?;
    owner.shutdown().await;
    require(owner.stopped(), "segmented article event loop did not stop")?;
    std::fs::write(
        directory.join("result.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "status": "passed",
            "scope": "synthetic loopback peer and article keyboard intents",
            "first_segment_end": first_segment_end,
            "second_segment_first": first_segment_end,
            "final_line": final_line,
            "requests_before_continuation": boundary_requests,
            "requests": owner.requests.get(),
            "article_loop_joined": true,
        }))
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

async fn verify_switch(
    ui: TerminalWindow,
    directory: PathBuf,
    owner: Rc<ArticleSession>,
    connection: Rc<Connection>,
    terminal: bool,
) -> Result<(), String> {
    use pttstella::{navigation::Intent, vim::Key};

    let settled = |article: char, station_line: usize| {
        verification::foreground(&ui, true);
        let view = connection.updates.borrow().clone();
        let expected = format!("ARTICLE {article} line {station_line:03}");
        let core = owner.core.borrow();
        let visible_line = if terminal { station_line } else { 1 };
        let visible = format!("ARTICLE {article} line {visible_line:03}");
        view.phase == Phase::Connected
            && !view.input_pending
            && !view.output_pending
            && selection::extract(&view.frame, &Selection::Line { row: 0 })
                .is_ok_and(|line| line.text.trim_end() == expected)
            && !core.dirty()
            && !core.has_in_flight()
            && core.navigation_shown(view.stamp())
            && core.last_displayed().is_some_and(|p| {
                selection::extract(&p.frame, &Selection::Line { row: 0 })
                    .is_ok_and(|line| line.text.trim_end() == visible)
            })
            && if terminal {
                !ui.get_article_active() && core.reading_progress().is_none()
            } else {
                ui.get_article_reading_ready()
                    && core
                        .reading_progress()
                        .is_some_and(|(_, first, last, state)| {
                            first == 1 && last == 45 && state == Some(LoadState::Complete)
                        })
            }
    };
    let ready = |article, line| {
        let owner = owner.clone();
        async move {
            until(
                || settled(article, line),
                "adjacent article page did not settle",
            )
            .await
            .map_err(|error| {
                format!(
                    "{error}: article={article} line={line} {}",
                    owner.diagnostics()
                )
            })
        }
    };
    let send = |bytes: &[u8]| {
        let generation = connection.updates.borrow().generation;
        owner.route_input(generation, bytes.to_vec())
    };
    let arrow = |down: bool, repeat| {
        let name = if down { "ArrowDown" } else { "ArrowUp" };
        require(
            owner.article_arrow(&Key {
                key: name.into(),
                code: name.into(),
                ctrl: false,
                alt: false,
                shift: false,
                repeat,
            }),
            "article direction key bypassed the edge policy",
        )
    };

    if terminal {
        ready('A', 1).await?;
        send(b"\x1b[6~")?;
        ready('A', 23).await?;
        send(b"\x1b[H")?;
        ready('A', 1).await?;
    } else {
        ready('A', 23).await?;
        ui.invoke_article_navigate(4);
        require(
            ui.get_article_offset_y() == 0.,
            "Home did not reach the cached head",
        )?;
        require(
            owner.local_intent(Intent::Lines {
                down: false,
                count: 1,
            }),
            "Vim line movement bypassed the cache",
        )?;
    }
    let first_scope = owner.progress().map(|(id, ..)| id);
    let first_line = if terminal { 1 } else { 23 };
    let before = owner.requests.get();
    arrow(false, true)?;
    owner
        .core
        .borrow_mut()
        .set_article_boundary_navigation(false);
    ui.invoke_article_session_updated();
    ready('A', first_line).await?;
    arrow(false, false)?;
    require(
        owner.requests.get() == before,
        "repeat or disabled option switched article",
    )?;
    owner
        .core
        .borrow_mut()
        .set_article_boundary_navigation(true);
    ui.invoke_article_session_updated();
    ready('A', first_line).await?;
    arrow(false, false)?;
    if terminal {
        ready('B', 1).await?;
        send(b"\x1b[6~")?;
    }
    ready('B', 23).await?;
    let second_scope = owner.progress().map(|(id, ..)| id);
    require(
        terminal || first_scope < second_scope,
        "previous article reused the old cache",
    )?;
    if !terminal {
        ui.invoke_article_navigate(5);
        require(
            ui.get_article_offset_y() > 0.,
            "End did not reach the cached tail",
        )?;
        require(
            owner.local_intent(Intent::NextPage),
            "Page Down bypassed the cache",
        )?;
    }
    let before_next = owner.requests.get();
    arrow(true, true)?;
    require(
        owner.requests.get() == before_next,
        "held Down switched article",
    )?;
    arrow(true, false)?;
    if terminal {
        ready('C', 1).await?;
        send(b"\x1b[6~")?;
    }
    ready('C', 23).await?;
    require(
        terminal || second_scope < owner.progress().map(|(id, ..)| id),
        "next article reused the old cache",
    )?;
    require(
        owner.requests.get() == if terminal { 2 } else { 5 },
        "duplicate article switch or preload",
    )?;
    verification::capture(&ui, &directory, "article-c.png")?;
    std::fs::write(
        directory.join("article-switch.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "terminal": terminal, "requests": owner.requests.get(),
            "scopes": [first_scope, second_scope, owner.progress().map(|(id, ..)| id)],
            "final": owner.diagnostics()
        }))
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}
