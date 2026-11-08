//! Full inline context-menu path; all UI input here is explicitly synthetic.
use super::{
    Inline,
    probe::{arrow, click, picture},
};
use crate::{
    TerminalWindow,
    article::session::ArticleSession,
    verification::{self, require, wait},
};
use slint::{
    ComponentHandle, Model,
    platform::{Key, PointerEventButton as Button, WindowEvent as Event},
};
use std::{path::Path, rc::Rc, time::Duration};

fn right(ui: &TerminalWindow, point: slint::LogicalPosition) {
    ui.window()
        .dispatch_event(Event::PointerMoved { position: point });
    verification::click(ui, point, Button::Right);
}
async fn opened(ui: &TerminalWindow) -> Result<(), String> {
    wait(|| ui.get_context_open(), "inline context menu did not open").await
}
fn menu(ui: &TerminalWindow) -> Vec<(String, bool)> {
    let m = ui.get_context_entries();
    (0..m.row_count())
        .filter_map(|i| m.row_data(i))
        .map(|e| (e.label.to_string(), e.enabled))
        .collect()
}
async fn ready(
    ui: &TerminalWindow,
    owner: &Inline,
    article: &ArticleSession,
) -> Result<(), String> {
    wait(
        || {
            ui.get_inline_enabled()
                && article.output_geometry().is_some()
                && *owner.last_geometry.borrow() == article.output_geometry()
        },
        "inline menu geometry missing",
    )
    .await
}
pub(super) async fn run(
    ui: &TerminalWindow,
    owner: &Rc<Inline>,
    article: &Rc<ArticleSession>,
    directory: &Path,
    kind: &str,
) -> Result<(), String> {
    require(
        matches!(kind, "basic" | "stall" | "error" | "all"),
        "unknown inline menu probe",
    )?;
    if kind == "all" {
        return all_from_zero(ui, owner, article, directory).await;
    }
    let images = owner
        .projection
        .borrow()
        .as_ref()
        .ok_or("menu projection missing")?
        .images();
    let index = if kind == "basic" {
        images[0].0
    } else {
        images[2].0
    } as i32;
    verification::capture(ui, directory, "before-menu-input.png")?;
    ready(ui, owner, article).await?;
    click(ui, arrow(ui, index)?);
    wait(
        || owner.state.borrow().entries.len() == 1 && ui.get_inline_pictures().row_count() == 1,
        "menu image entry missing",
    )
    .await?;
    if kind == "basic" {
        wait(
            || owner.state.borrow().resources.len() == 1,
            "menu PNG not decoded",
        )
        .await?;
    } else if kind == "error" {
        wait(
            || {
                owner
                    .state
                    .borrow()
                    .resources
                    .values()
                    .any(|r| r.error.is_some())
            },
            "invalid GIF did not fail",
        )
        .await?;
    } else {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    ready(ui, owner, article).await?;
    let p = picture(ui, index)?;
    right(ui, p);
    opened(ui).await?;
    let expected = [
        pttstella::msg!("context-copy-picture"),
        pttstella::msg!("context-save-picture"),
        pttstella::msg!("context-copy-link"),
        pttstella::msg!("context-open-link-external"),
    ];
    let entries = menu(ui);
    require(
        entries.iter().map(|e| &e.0).eq(expected.iter()),
        "picture menu order/labels differ",
    )?;
    require(
        entries.iter().map(|e| e.1).collect::<Vec<_>>()
            == [kind == "basic", kind == "basic", true, true],
        "picture menu capability flags differ",
    )?;
    require(
        !ui.get_media_open(),
        "picture right-click became a modal preview",
    )?;
    verification::capture(ui, directory, "picture-menu.png")?;
    if kind != "basic" {
        ui.set_input_notice("disabled-picture-sentinel".into());
        ui.invoke_context_choose(0);
        require(
            !ui.get_context_open()
                && !ui.get_output_busy()
                && ui.get_input_notice() == "disabled-picture-sentinel",
            "disabled picture action performed output",
        )?;
        right(ui, picture(ui, index)?);
        opened(ui).await?;
        verification::key(ui, Key::Return); // First enabled item is Copy URL.
        wait(
            || !ui.get_output_busy() && ui.get_input_notice() == pttstella::msg!("context-copied"),
            "unready picture URL copy failed",
        )
        .await?;
        ready(ui, owner, article).await?;
        right(ui, picture(ui, index)?);
        opened(ui).await?;
        article.station_handoff();
        wait(
            || !ui.get_context_open() && !owner.active.get(),
            "unready menu survived handoff",
        )
        .await?;
    } else {
        verification::key(ui, Key::Escape);
        require(
            !ui.get_context_open() && !ui.get_media_open() && ui.get_article_focused(),
            "menu Esc did not restore article focus",
        )?;
        right(ui, picture(ui, index)?);
        opened(ui).await?;
        verification::key(ui, Key::Tab);
        require(
            !ui.get_context_open() && ui.get_article_focused(),
            "menu Tab escaped article focus",
        )?;
        // Right drag cancels; Shift+right still requests the picture menu.
        let p = picture(ui, index)?;
        let q = slint::LogicalPosition::new(p.x + 2.5, p.y + 2.5);
        ui.window().dispatch_event(Event::PointerPressed {
            position: p,
            button: Button::Right,
        });
        ui.window()
            .dispatch_event(Event::PointerMoved { position: q });
        ui.window().dispatch_event(Event::PointerReleased {
            position: q,
            button: Button::Right,
        });
        require(!ui.get_context_open(), "right drag opened picture menu")?;
        ui.window().dispatch_event(Event::KeyPressed {
            text: Key::Shift.into(),
        });
        right(ui, p);
        ui.window().dispatch_event(Event::KeyReleased {
            text: Key::Shift.into(),
        });
        opened(ui).await?;
        require(!ui.get_media_open(), "Shift+right became a preview click")?;
        verification::key(ui, Key::Escape);
        right(ui, picture(ui, index)?);
        opened(ui).await?;
        verification::key(ui, Key::Return);
        wait(
            || !ui.get_output_busy() && ui.get_input_notice() == pttstella::msg!("export-copied"),
            "inline original PNG copy failed",
        )
        .await?;
        require(
            ui.get_vim_available(),
            "copy completion retained disabled keyboard actions",
        )?;
        ready(ui, owner, article).await?;
        right(ui, picture(ui, index)?);
        opened(ui).await?;
        verification::key(ui, Key::Home);
        verification::key(ui, Key::DownArrow);
        verification::key(ui, Key::DownArrow);
        verification::key(ui, Key::Return);
        wait(
            || !ui.get_output_busy() && ui.get_input_notice() == pttstella::msg!("context-copied"),
            "inline URL copy failed",
        )
        .await?;
        ready(ui, owner, article).await?;
        right(ui, picture(ui, index)?);
        opened(ui).await?;
        verification::key(ui, Key::End);
        verification::key(ui, Key::Return);
        require(
            ui.get_input_notice().as_str() == images[0].2,
            "picture Open Link changed URL or did not reach probe sink",
        )?;
        // Shift+background bypasses floating tools under the original policy.
        ready(ui, owner, article).await?;
        let crate::pointer::SurfaceGeometry::Article(g) =
            article.output_geometry().ok_or("menu geometry lost")?
        else {
            return Err("expected article geometry".into());
        };
        let point = slint::LogicalPosition::new(
            ui.get_terminal_origin_x() + g.left / g.dpi + 2. * g.line_width as f32 / 80. / g.dpi,
            ui.get_terminal_origin_y()
                + ((g.flow.top(0) as f64 - g.offset) / f64::from(g.dpi)) as f32
                + g.row as f32 / 2. / g.dpi,
        );
        ui.window().dispatch_event(Event::KeyPressed {
            text: Key::Shift.into(),
        });
        right(ui, point);
        ui.window().dispatch_event(Event::KeyReleased {
            text: Key::Shift.into(),
        });
        opened(ui).await?;
        let entries = menu(ui);
        let all = entries
            .iter()
            .position(|e| e.0 == pttstella::msg!("context-inline") && e.1)
            .ok_or("content menu All unavailable")?;
        ui.invoke_context_choose(all as i32);
        wait(
            || owner.state.borrow().resources.len() == 3 && ui.get_inline_enabled(),
            "menu All did not open one shared batch",
        )
        .await?;
        right(ui, picture(ui, images[1].0 as i32)?);
        opened(ui).await?;
        verification::key(ui, Key::Return);
        wait(
            || !ui.get_output_busy() && ui.get_input_notice() == pttstella::msg!("export-copied"),
            "GIF menu did not copy original red first frame",
        )
        .await?;
        ready(ui, owner, article).await?;
        right(ui, picture(ui, index)?);
        opened(ui).await?;
        ui.set_zoom(1.25);
        wait(|| !ui.get_context_open(), "menu survived geometry change").await?;
        ui.set_input_notice("stale-picture-sentinel".into());
        ui.invoke_context_choose(0);
        tokio::time::sleep(Duration::from_millis(50)).await;
        require(
            !ui.get_output_busy() && ui.get_input_notice() == "stale-picture-sentinel",
            "stale menu performed output",
        )?;
        ready(ui, owner, article).await?;
        right(ui, picture(ui, index)?);
        opened(ui).await?;
        article.station_handoff();
        wait(
            || !ui.get_context_open() && !owner.active.get(),
            "picture menu survived raw handoff",
        )
        .await?;
    }
    require(article.requests.get() == 1, "picture menu leaked BBS bytes")?;
    std::fs::write(directory.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","scope":"actual Slint widget and shared menu policy with synthetic toolkit gestures/foreground; clipboard readback only in explicit fixture probe","kind":kind,"station_requests":1,"picture_menu_order":expected,"copy_save_enabled":kind=="basic","source_handoff_clears":true,"pending":["native OS save picker","four target native acceptance","main video/editor and resource performance"]})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    Ok(())
}

async fn all_from_zero(
    ui: &TerminalWindow,
    owner: &Rc<Inline>,
    article: &Rc<ArticleSession>,
    directory: &Path,
) -> Result<(), String> {
    verification::capture(ui, directory, "before-all.png")?;
    ready(ui, owner, article).await?;
    require(!owner.active.get(), "all-from-zero already owns a batch")?;
    let crate::pointer::SurfaceGeometry::Article(g) =
        article.output_geometry().ok_or("all geometry missing")?
    else {
        return Err("expected article".into());
    };
    let point = slint::LogicalPosition::new(
        ui.get_terminal_origin_x() + g.left / g.dpi + 2. * g.line_width as f32 / 80. / g.dpi,
        ui.get_terminal_origin_y() + g.row as f32 / 2. / g.dpi,
    );
    ui.window().dispatch_event(Event::KeyPressed {
        text: Key::Shift.into(),
    });
    right(ui, point);
    ui.window().dispatch_event(Event::KeyReleased {
        text: Key::Shift.into(),
    });
    opened(ui).await?;
    let all = menu(ui)
        .iter()
        .position(|e| e.0 == pttstella::msg!("context-inline") && e.1)
        .ok_or("all-from-zero action missing")?;
    ui.invoke_context_choose(all as i32);
    wait(
        || owner.state.borrow().resources.len() == 3,
        "all-from-zero resources missing",
    )
    .await?;
    ready(ui, owner, article).await?;
    let p = ui
        .get_inline_pictures()
        .row_data(0)
        .ok_or("first all picture missing")?;
    require(
        (p.source_width, p.source_height) == (64, 32) && p.width == 64. && p.image_height == 32.,
        "first picture retained placeholder clip geometry",
    )?;
    let pixels = p.image.to_rgba8().ok_or("first all image is not RGBA")?;
    require(
        pixels.as_bytes()[..4] == [255, 0, 0, 255]
            && pixels.as_bytes()[48 * 4..48 * 4 + 4] == [0, 0, 255, 255],
        "all static model has wrong source pixels",
    )?;
    verification::capture(ui, directory, "all-from-zero.png")?;
    let shot = ui.window().take_snapshot().map_err(|e| e.to_string())?;
    for (x, color) in [(16., [255, 0, 0, 255]), (48., [0, 0, 255, 255])] {
        let dpi = ui.window().scale_factor();
        let sx = ((ui.get_terminal_origin_x() + p.x + x) * dpi).round() as usize;
        let sy = ((ui.get_terminal_origin_y() + p.y + 16.) * dpi).round() as usize;
        let offset = (sy * shot.width() as usize + sx) * 4;
        require(
            offset + 4 <= shot.as_bytes().len() && shot.as_bytes()[offset..offset + 4] == color,
            "all static widget pixels differ",
        )?;
    }
    require(
        article.requests.get() == 1,
        "all-from-zero leaked station bytes",
    )?;
    article.station_handoff();
    wait(
        || !owner.active.get() && ui.get_inline_pictures().row_count() == 0,
        "all-from-zero scope not cleared",
    )
    .await?;
    std::fs::write(directory.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","kind":"all","scope":"Slint full component with synthetic Shift/right and menu choose; model RGBA and snapshot pixels independently checked; native dirty-region acceptance is separate CUA evidence","zero_initial_batch":true,"images":3,"static_clip_and_rgba":true,"static_widget_pixels":true,"station_requests":1,"handoff_clears":true})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    Ok(())
}
