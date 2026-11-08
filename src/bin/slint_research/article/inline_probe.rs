//! Full Slint article/HTTP path; explicitly synthetic foreground and gestures.
use super::Inline;
use crate::article::session::ArticleSession;
use crate::{
    TerminalWindow,
    verification::{self, Verification, require, wait},
};
use slint::{
    ComponentHandle, Model,
    platform::{Key, PointerEventButton as Button, WindowEvent as Event},
};
use std::{error::Error, path::PathBuf, rc::Rc, time::Duration};
pub(super) fn arrow(ui: &TerminalWindow, index: i32) -> Result<slint::LogicalPosition, String> {
    let model = ui.get_inline_arrows();
    let a = (0..model.row_count())
        .filter_map(|n| model.row_data(n))
        .find(|a| a.index == index)
        .ok_or("inline arrow not visible")?;
    Ok(slint::LogicalPosition::new(
        ui.get_terminal_origin_x() + a.x + a.size / 2.,
        ui.get_terminal_origin_y() + a.y + a.size / 2.,
    ))
}
pub(super) fn picture(ui: &TerminalWindow, index: i32) -> Result<slint::LogicalPosition, String> {
    let model = ui.get_inline_pictures();
    let p = (0..model.row_count())
        .filter_map(|n| model.row_data(n))
        .find(|a| a.index == index)
        .ok_or("inline picture not visible")?;
    Ok(slint::LogicalPosition::new(
        ui.get_terminal_origin_x() + p.x + p.width / 2.,
        ui.get_terminal_origin_y() + p.y + p.height / 2.,
    ))
}
pub(super) fn click(ui: &TerminalWindow, position: slint::LogicalPosition) {
    ui.window().dispatch_event(Event::PointerMoved { position });
    verification::click(ui, position, Button::Left);
}
fn diagnostics(ui: &TerminalWindow, owner: &Inline) -> String {
    let article = owner.article.upgrade();
    let native = article.as_ref().and_then(|a| a.view.borrow().clone());
    let fit = native.as_ref().and_then(|n| {
        crate::article::layout::Layout::fit(
            ui.get_view_width(),
            ui.get_view_height(),
            ui.window().scale_factor(),
            ui.get_zoom(),
            n.rows.get(),
        )
    });
    format!(
        "geometry={} core={} enabled={} arrows={} progress={:?} ui={}x{} dpi={} zoom={} layout={:?} fit={:?} anchor={:?} painted_anchor={:?} painted={:?} flow_height={:?} timer={:?}",
        article
            .as_ref()
            .is_some_and(|a| a.output_geometry().is_some()),
        article
            .as_ref()
            .map(|a| a.diagnostics())
            .unwrap_or_default(),
        ui.get_inline_enabled(),
        ui.get_inline_arrows().row_count(),
        article.as_ref().and_then(|a| a.progress()),
        ui.get_view_width(),
        ui.get_view_height(),
        ui.window().scale_factor(),
        ui.get_zoom(),
        native.as_ref().and_then(|n| n.layout.get()),
        fit,
        native.as_ref().map(|n| n.anchor.get()),
        native.as_ref().map(|n| n.painted_anchor.get()),
        native
            .as_ref()
            .map(|n| n
                .painted
                .borrow()
                .as_ref()
                .map(|p| (p.rows.clone(), p.width, p.row, p.dpi))),
        native.as_ref().and_then(|n| n.flow()).map(|f| f.height()),
        native.as_ref().map(|n| n.timer.running())
    )
}
pub fn start(
    ui: &TerminalWindow,
    owner: Rc<Inline>,
    article: Rc<ArticleSession>,
    directory: PathBuf,
    settings_path: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(
        ui,
        directory,
        "article-inline",
        move |ui, directory| async move {
            wait(
                || {
                    verification::foreground(&ui, true);
                    ui.invoke_focus_article();
                    article.progress().is_some_and(|(_, _, last, state)| {
                        last == 45 && state == Some(pttstella::article_cache::LoadState::Complete)
                    }) && ui.get_inline_enabled()
                        && ui.get_inline_arrows().row_count() == 3
                },
                "inline reading projection/controls missing",
            )
            .await
            .map_err(|e| format!("{e}; {}", diagnostics(&ui, &owner)))?;
            if let Ok(kind) = std::env::var("PTTSTELLA_INLINE_MENU_PROBE") {
                return super::menu_probe::run(&ui, &owner, &article, &directory, &kind).await;
            }
            if std::env::var_os("PTTSTELLA_INLINE_SETTINGS_PROBE").is_some() {
                let media = owner.media.upgrade().ok_or("media owner missing")?;
                media
                    .verify_automatic(&ui, &settings_path, &directory)
                    .await?;
                article.station_handoff();
                wait(
                    || !owner.active.get(),
                    "settings probe handoff retained inline scope",
                )
                .await?;
                return Ok(());
            }
            if let Ok(kind) = std::env::var("PTTSTELLA_INLINE_KEYS_PROBE") {
                return verify_keys(&ui, &owner, &article, &directory, &kind).await;
            }
            require(
                article.requests.get() == 1,
                "inline fixture fetched wrong page count",
            )?;
            require(
                !owner.active.get(),
                "default-off inline started media worker",
            )?;
            let images = owner
                .projection
                .borrow()
                .as_ref()
                .ok_or("inline projection missing")?
                .images();
            require(
                images.len() == 3 && images.iter().map(|t| t.1).collect::<Vec<_>>() == [1, 3, 5],
                "inline duplicate URL placement changed",
            )?;
            let png = images[0].0 as i32;
            let gif = images[1].0 as i32;
            let once = images[2].0 as i32;
            verification::capture(&ui, &directory, "before-arrow.png")?;
            eprintln!(
                "inline probe point {:?} origin {},{}",
                arrow(&ui, png)?,
                ui.get_terminal_origin_x(),
                ui.get_terminal_origin_y()
            );
            wait(
                || {
                    owner
                        .article
                        .upgrade()
                        .and_then(|a| a.output_geometry())
                        .is_some()
                },
                "post-snapshot geometry missing",
            )
            .await?;
            click(&ui, arrow(&ui, png)?);
            wait(
                || {
                    owner.state.borrow().resources.len() == 1
                        && ui.get_inline_enabled()
                        && ui.get_inline_pictures().row_count() == 1
                },
                "single PNG did not decode and install",
            )
            .await?;
            require(
                owner.state.borrow().entries.len() == 1,
                "single expansion opened other images",
            )?;
            let key = owner.state.borrow().entries[0].key;
            {
                let s = owner.state.borrow();
                let r = s.resources.get(&key).ok_or("PNG resource missing")?;
                let first = r.first.as_ref().ok_or("PNG first pixels missing")?;
                require(
                    (first.width(), first.height()) == (64, 32),
                    "inline PNG dimensions differ",
                )?;
                require(
                    first.as_bytes()[..4] == [255, 0, 0, 255]
                        && first.as_bytes()[48 * 4..48 * 4 + 4] == [0, 0, 255, 255],
                    "inline PNG fixture colors differ",
                )?;
            }
            verification::capture(&ui, &directory, "png-inline.png")?;
            let model = ui.get_inline_pictures();
            let tile = model.row_data(0).ok_or("PNG widget missing")?;
            let shot = ui.window().take_snapshot().map_err(|e| e.to_string())?;
            let dpi = ui.window().scale_factor();
            for (x, color) in [(16., [255, 0, 0, 255]), (48., [0, 0, 255, 255])] {
                let sx = ((ui.get_terminal_origin_x() + tile.x + x) * dpi).round() as usize;
                let sy = ((ui.get_terminal_origin_y() + tile.y + 16.) * dpi).round() as usize;
                let offset = (sy * shot.width() as usize + sx) * 4;
                require(
                    offset + 4 <= shot.as_bytes().len()
                        && shot.as_bytes()[offset..offset + 4] == color,
                    "inline PNG actual widget pixels differ",
                )?;
            }
            // A diagonal three-pixel drag is not a click, and modifiers do
            // not turn an inline control into an image-open request.
            let from = picture(&ui, png)?;
            let to = slint::LogicalPosition::new(from.x + 2.5, from.y + 2.5);
            ui.window()
                .dispatch_event(Event::PointerMoved { position: from });
            ui.window().dispatch_event(Event::PointerPressed {
                position: from,
                button: Button::Left,
            });
            ui.window()
                .dispatch_event(Event::PointerMoved { position: to });
            ui.window().dispatch_event(Event::PointerReleased {
                position: to,
                button: Button::Left,
            });
            require(
                !ui.get_media_open(),
                "diagonal image drag became a preview click",
            )?;
            ui.window().dispatch_event(Event::KeyPressed {
                text: Key::Shift.into(),
            });
            click(&ui, arrow(&ui, gif)?);
            ui.window().dispatch_event(Event::KeyReleased {
                text: Key::Shift.into(),
            });
            tokio::time::sleep(Duration::from_millis(30)).await;
            require(
                !ui.get_media_open() && owner.state.borrow().entries.len() == 1,
                "modified inline click opened another source",
            )?;
            // Opened controls and blocked selection must consume the pointer,
            // never fall through into the ordinary URL underneath.
            click(&ui, arrow(&ui, png)?);
            tokio::time::sleep(Duration::from_millis(30)).await;
            require(
                !ui.get_media_open(),
                "opened inline arrow fell through to its URL",
            )?;
            ui.set_selection_active(true);
            wait(
                || !ui.get_inline_enabled(),
                "selection did not block inline actions",
            )
            .await?;
            click(&ui, arrow(&ui, png)?);
            require(
                !ui.get_media_open(),
                "blocked arrow fell through to underlying URL",
            )?;
            ui.set_selection_active(false);
            wait(
                || ui.get_inline_enabled(),
                "selection clear did not restore inline controls",
            )
            .await?;
            // A separate explicit URL click shares the already decoded original.
            let native = article
                .view
                .borrow()
                .clone()
                .ok_or("inline viewport missing")?;
            let layout = native.layout.get().ok_or("inline layout missing")?;
            let geometry = article.output_geometry().ok_or("inline geometry missing")?;
            let crate::pointer::SurfaceGeometry::Article(g) = geometry else {
                return Err("inline surface is not an article".into());
            };
            let position = slint::LogicalPosition::new(
                ui.get_terminal_origin_x() + g.left / g.dpi + layout.logical_width() * 2. / 80.,
                ui.get_terminal_origin_y() + layout.line_height() * 1.5,
            );
            click(&ui, position);
            wait(
                || ui.get_media_open() && ui.get_media_ready(),
                "URL click did not open owned original",
            )
            .await?;
            verification::key(&ui, Key::Escape);
            wait(
                || !ui.get_media_open() && ui.get_inline_enabled(),
                "owned URL preview did not close",
            )
            .await?;
            click(&ui, arrow(&ui, gif)?);
            wait(
                || {
                    owner.state.borrow().resources.values().any(|r| {
                        r.playing && r.pixels.as_ref().is_some_and(|p| p.as_bytes()[2] == 255)
                    }) && ui.get_inline_enabled()
                },
                "visible inline GIF did not animate",
            )
            .await?;
            require(
                owner.state.borrow().resources.len() == 2,
                "single GIF opened the third image",
            )?;
            tokio::time::sleep(Duration::from_millis(30)).await;
            let frames = owner
                .state
                .borrow()
                .resources
                .values()
                .map(|r| r.frame)
                .sum::<u64>();
            let layouts = owner.layout_updates.get();
            let pixels = owner.pixel_updates.get();
            let rasters = article.raster_count();
            wait(
                || {
                    owner
                        .state
                        .borrow()
                        .resources
                        .values()
                        .map(|r| r.frame)
                        .sum::<u64>()
                        > frames
                },
                "inline GIF stopped advancing",
            )
            .await?;
            require(
                owner.layout_updates.get() == layouts
                    && article.raster_count() == rasters
                    && owner.pixel_updates.get() > pixels,
                "GIF frame rebuilt article layout or text raster",
            )?;
            verification::capture(&ui, &directory, "gif-inline.png")?;
            click(&ui, picture(&ui, gif)?);
            wait(
                || ui.get_media_open() && ui.get_media_ready() && ui.get_media_playing(),
                "inline original did not promote to modal",
            )
            .await?;
            wait(
                || owner.state.borrow().resources.values().all(|r| !r.playing),
                "modal did not stop inline animations",
            )
            .await?;
            verification::capture(&ui, &directory, "modal-from-inline.png")?;
            verification::key(&ui, Key::Escape);
            wait(
                || {
                    !ui.get_media_open()
                        && ui.get_article_focused()
                        && owner.state.borrow().resources.values().any(|r| r.playing)
                },
                "modal close did not restore inline playback/focus",
            )
            .await?;
            // A pointer press cannot authorize an expansion after geometry changes.
            let position = arrow(&ui, once)?;
            ui.window().dispatch_event(Event::PointerMoved { position });
            ui.window().dispatch_event(Event::PointerPressed {
                position,
                button: Button::Left,
            });
            ui.set_zoom(1.25);
            wait(
                || ui.get_inline_enabled(),
                "zoomed inline map was not acknowledged",
            )
            .await?;
            ui.window().dispatch_event(Event::PointerReleased {
                position,
                button: Button::Left,
            });
            tokio::time::sleep(Duration::from_millis(50)).await;
            require(
                owner.state.borrow().resources.len() == 2
                    && owner.state.borrow().entries.len() == 2,
                "old geometry press opened a new source",
            )?;
            require(
                article.local_intent(pttstella::navigation::Intent::Last),
                "local End was not consumed",
            )?;
            wait(
                || owner.state.borrow().resources.values().all(|r| !r.playing),
                "offscreen GIF retained playback",
            )
            .await?;
            require(
                article.local_intent(pttstella::navigation::Intent::First),
                "local Home was not consumed",
            )?;
            wait(
                || owner.state.borrow().resources.values().any(|r| r.playing),
                "visible GIF did not resume",
            )
            .await?;
            verification::foreground(&ui, false);
            wait(
                || owner.state.borrow().resources.values().all(|r| !r.playing),
                "background retained inline playback",
            )
            .await?;
            verification::foreground(&ui, true);
            ui.invoke_focus_article();
            wait(
                || owner.state.borrow().resources.values().any(|r| r.playing),
                "foreground did not resume inline playback",
            )
            .await?;
            require(
                article.requests.get() == 1,
                "inline interactions sent additional station keys",
            )?;
            let before_handoff = owner.state.borrow().resources.len();
            article.station_handoff();
            wait(
                || {
                    !ui.get_article_active()
                        && ui.get_inline_pictures().row_count() == 0
                        && !owner.active.get()
                },
                "raw handoff retained inline resources or controls",
            )
            .await?;

            let record = serde_json::json!({"scope":"full actual Slint article widget; synthetic toolkit foreground/pointer/keys; loopback HTTP and strict BBS peer; OS gestures/IME not covered","status":"passed","rows":45,"station_requests":1,"deduplicated_arrow_rows":[1,3,5],"single_png_and_gif_only":true,"opened_and_selection_blocked_arrows_do_not_fall_through":true,"explicit_url_reuses_inline_original":true,"actual_png_snapshot_pixels":true,"modal_pauses_inline":true,"offscreen_and_focus_pause":true,"animation_updates_pixels_only":true,"stale_geometry_press_rejected":true,"raw_handoff_clears_inline":true,"scope_source":owner.scope.get(),"resources_before_raw_handoff":before_handoff,"resources_after_raw_handoff":owner.state.borrow().resources.len(),"diagonal_drag_and_modifier_clicks_rejected":true,"pending":["short/long Enter","wide mode","auto setting UI","picture menu","two central GIF resource peak","four platform native acceptance"]});
            std::fs::write(
                directory.join("result.json"),
                serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        },
    )
}

async fn verify_keys(
    ui: &TerminalWindow,
    owner: &Rc<Inline>,
    article: &Rc<ArticleSession>,
    directory: &std::path::Path,
    kind: &str,
) -> Result<(), String> {
    require(matches!(kind, "short" | "long"), "unknown inline key probe")?;
    verification::capture(ui, directory, "before-enter-input.png")?;
    wait(
        || {
            ui.get_inline_enabled()
                && article.output_geometry().is_some()
                && *owner.last_geometry.borrow() == article.output_geometry()
        },
        "Enter initial painted geometry missing",
    )
    .await?;
    let indices: Vec<_> = owner.candidates.borrow().iter().map(|c| c.0).collect();
    require(
        indices.len() == 3 && !owner.active.get(),
        "key probe initial state differs",
    )?;
    let down = || {
        ui.window().dispatch_event(Event::KeyPressed {
            text: Key::Return.into(),
        })
    };
    let up = || {
        ui.window().dispatch_event(Event::KeyReleased {
            text: Key::Return.into(),
        })
    };
    // Real toolkit keyboard edges must pass through the article's focus path.
    down();
    tokio::time::sleep(Duration::from_millis(70)).await;
    require(
        owner.enter_press.borrow().is_some() && !owner.active.get(),
        "Enter did not arm on keydown without loading",
    )?;
    up();
    wait(
        || owner.state.borrow().resources.len() == 1 && ui.get_inline_enabled(),
        "short Enter did not open one source on release",
    )
    .await?;
    require(
        owner.state.borrow().entries[0].index == indices[0],
        "first Enter skipped the first visible image",
    )?;
    verification::capture(ui, directory, "first-enter.png")?;
    if kind == "short" {
        for count in 2..=3 {
            down();
            up();
            wait(
                || owner.state.borrow().resources.len() == count && ui.get_inline_enabled(),
                "short Enter failed to advance visible image order",
            )
            .await?;
            require(
                owner.wide.borrow().is_empty(),
                "Enter widened before all visible images opened",
            )?;
        }
        require(
            owner
                .state
                .borrow()
                .entries
                .iter()
                .map(|e| e.index)
                .collect::<Vec<_>>()
                == indices,
            "Enter expansion order differs",
        )?;
        let native = article
            .view
            .borrow()
            .clone()
            .ok_or("wide viewport missing")?;
        let layout = native.layout.get().ok_or("wide layout missing")?;
        // The URL row is above the viewport, but its image is still visible.
        ui.invoke_article_scroll(-layout.line_height() * 2.5, false);
        wait(
            || {
                native.flow().is_some_and(|flow| {
                    (flow.top(1) + u64::from(layout.row)) as f64 <= native.position(&flow)
                }) && native.geometry().is_some()
                    && *owner.last_geometry.borrow() == native.geometry()
                    && ui.get_inline_enabled()
                    && ui.get_inline_pictures().row_count() > 0
            },
            "image-only visible candidate geometry missing",
        )
        .await?;
        down();
        up();
        wait(
            || owner.wide.borrow().len() == 1 && ui.get_inline_enabled(),
            "all-open Enter did not widen first visible image",
        )
        .await?;
        let first_key = owner.state.borrow().entries[0].key;
        require(
            owner.wide.borrow().contains(&first_key),
            "wrong image widened",
        )?;
        let p = ui.get_inline_pictures();
        let png = (0..p.row_count())
            .filter_map(|i| p.row_data(i))
            .find(|p| p.key == first_key as i32)
            .ok_or("wide PNG widget missing")?;
        require(
            (png.width - layout.logical_width()).abs() < 0.01 && png.width > 64.,
            "wide image kept original-size limit",
        )?;
        verification::capture(ui, directory, "wide-enter.png")?;
        let shot = ui.window().take_snapshot().map_err(|e| e.to_string())?;
        for (fraction, color) in [(0.25, [255, 0, 0, 255]), (0.75, [0, 0, 255, 255])] {
            let dpi = ui.window().scale_factor();
            let x = ((ui.get_terminal_origin_x() + png.x + png.width * fraction) * dpi).round()
                as usize;
            let y = ((ui.get_terminal_origin_y() + png.y + png.height / 2.) * dpi).round() as usize;
            let offset = (y * shot.width() as usize + x) * 4;
            require(
                offset + 4 <= shot.as_bytes().len() && shot.as_bytes()[offset..offset + 4] == color,
                "wide cropped widget pixels differ",
            )?;
        }
        let wide = owner.wide.borrow().len();
        down();
        require(
            article.local_intent(pttstella::navigation::Intent::Last),
            "key cancellation local End rejected",
        )?;
        tokio::time::sleep(Duration::from_millis(600)).await;
        up();
        require(
            owner.enter_press.borrow().is_none() && owner.wide.borrow().len() == wide,
            "scroll did not cancel held Enter",
        )?;
    } else {
        // A deadline cannot survive blur, geometry change, or modifiers.
        down();
        verification::foreground(ui, false);
        tokio::time::sleep(Duration::from_millis(600)).await;
        up();
        require(
            owner.state.borrow().resources.len() == 1 && owner.enter_press.borrow().is_none(),
            "blur fired held Enter",
        )?;
        verification::foreground(ui, true);
        ui.invoke_focus_article();
        wait(
            || ui.get_inline_enabled() && ui.get_article_focused(),
            "key probe refocus failed",
        )
        .await?;
        down();
        ui.set_zoom(1.25);
        tokio::time::sleep(Duration::from_millis(600)).await;
        up();
        require(
            owner.state.borrow().resources.len() == 1 && owner.enter_press.borrow().is_none(),
            "zoom fired held Enter",
        )?;
        wait(|| ui.get_inline_enabled(), "key zoom ACK missing").await?;
        ui.window().dispatch_event(Event::KeyPressed {
            text: Key::Shift.into(),
        });
        down();
        up();
        ui.window().dispatch_event(Event::KeyReleased {
            text: Key::Shift.into(),
        });
        require(
            owner.enter_press.borrow().is_none() && owner.state.borrow().resources.len() == 1,
            "modified Enter expanded images",
        )?;
        down();
        tokio::time::sleep(Duration::from_millis(250)).await;
        down(); // Repeat cannot arm another timer or become BBS input.
        require(
            owner.state.borrow().resources.len() == 1,
            "long Enter fired before deadline",
        )?;
        wait(
            || owner.state.borrow().resources.len() == 3 && ui.get_inline_enabled(),
            "long Enter did not expand all once",
        )
        .await?;
        down();
        up();
        tokio::time::sleep(Duration::from_millis(100)).await;
        require(
            owner.wide.borrow().is_empty() && owner.state.borrow().entries.len() == 3,
            "long Enter release/repeat became a short action",
        )?;
        verification::capture(ui, directory, "long-enter.png")?;
    }
    require(article.requests.get() == 1, "Enter leaked station input")?;
    article.station_handoff();
    wait(
        || !owner.active.get() && owner.enter_press.borrow().is_none(),
        "handoff retained Enter timer/batch",
    )
    .await?;
    let record = serde_json::json!({"status":"passed","scope":"actual Slint window, synthetic toolkit keyboard/focus; not OS native input","kind":kind,"station_requests":1,"images":3,"release_only_short":true,"visible_order":indices,"raw_handoff_cancels":true,"pending":["native OS Enter and IME acceptance","extreme wide-image native pixels and memory"]});
    std::fs::write(
        directory.join("result.json"),
        serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
