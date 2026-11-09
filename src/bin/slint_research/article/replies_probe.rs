//! Full native components over the shared identity fixture's zero-input peer.
use super::session::ArticleSession;
use crate::{
    TerminalWindow,
    verification::{self, Verification, require, wait},
};
use pttstella::{
    connection::Connection,
    context_menu::Action,
    filters::{Policy, Rules},
    selection::Span,
};
use slint::Model;
use std::{error::Error, path::PathBuf, rc::Rc};

async fn ready(
    ui: &TerminalWindow,
    owner: &ArticleSession,
    connection: &Connection,
) -> Result<(), String> {
    wait(
        || {
            verification::foreground(ui, true);
            owner
                .output_identity(&connection.updates.borrow())
                .is_some()
                && ui.get_reply_navigation_ready()
        },
        "reply source never reached its native geometry ACK",
    )
    .await
}
fn count(ui: &TerminalWindow, current: i64, total: i64) -> Result<(), String> {
    let expected = pttstella::msg!("reply-navigation-count", current = current, total = total);
    require(
        ui.get_reply_navigation_count() == expected,
        &format!(
            "wrong selected reply/count: expected={expected}, actual={}, opened={},ready={},group={},offset={}",
            ui.get_reply_navigation_count(),
            ui.get_reply_navigation_open(),
            ui.get_reply_navigation_ready(),
            ui.get_reply_navigation_group(),
            ui.get_article_offset_y()
        ),
    )
}
async fn step(
    ui: &TerminalWindow,
    owner: &ArticleSession,
    connection: &Connection,
    action: i32,
    current: i64,
    total: i64,
) -> Result<(), String> {
    ready(ui, owner, connection).await?;
    ui.invoke_reply_navigation_action(action);
    ready(ui, owner, connection).await?;
    count(ui, current, total)
}
fn point(
    ui: &TerminalWindow,
    owner: &ArticleSession,
    connection: &Connection,
    row: usize,
    col: usize,
) -> Result<slint::LogicalPosition, String> {
    let geometry = owner.output_geometry().ok_or("reply geometry missing")?;
    let rect = owner
        .with_output(&connection.updates.borrow(), |p| {
            geometry.span(
                &Span {
                    row,
                    start: col,
                    end: col + 1,
                },
                &p.frame,
            )
        })
        .flatten()
        .ok_or("reply target outside viewport")?;
    Ok(slint::LogicalPosition::new(
        ui.get_terminal_origin_x() + (rect.left + rect.right) / 2. / geometry.dpi(),
        ui.get_terminal_origin_y() + (rect.top + rect.bottom) / 2. / geometry.dpi(),
    ))
}
async fn menu(
    ui: &TerminalWindow,
    owner: &ArticleSession,
    connection: &Connection,
    row: usize,
    col: usize,
    action: Action,
) -> Result<(), String> {
    verification::foreground(ui, true);
    let position = point(ui, owner, connection, row, col)?;
    verification::click(ui, position, slint::platform::PointerEventButton::Right);
    wait(|| ui.get_context_open(), "reply content menu did not open")
        .await
        .map_err(|e| {
            format!(
                "{e}: row={row},col={col},foreground={},blocked={},pending={},tools={},notice={}",
                ui.get_native_foreground(),
                ui.get_context_source_blocked(),
                ui.get_context_pending(),
                ui.get_tools_open(),
                ui.get_input_notice()
            )
        })?;
    let title = crate::context_menu::label(action);
    let index = ui
        .get_context_entries()
        .iter()
        .position(|e| e.label.as_str().ends_with(&title))
        .ok_or("reply context action missing")?;
    if action == Action::ReplyFocus {
        require(
            !ui.get_context_entries()
                .iter()
                .any(|e| e.label == pttstella::msg!("reply-navigation-title")),
            "background navigation displaced account priority",
        )?;
    }
    ui.invoke_context_choose(index as i32);
    Ok(())
}
pub fn start(
    ui: &TerminalWindow,
    owner: Rc<ArticleSession>,
    connection: Rc<Connection>,
    directory: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(
        ui,
        directory,
        "reply-navigation",
        move |ui, directory| async move {
            wait(
                || {
                    verification::foreground(&ui, true);
                    ui.invoke_focus_article();
                    owner
                        .with_output(&connection.updates.borrow(), |p| {
                            p.reply_navigation.author_rows == [3, 4, 10, 18]
                                && p.reply_navigation.mine_rows == [6, 14]
                        })
                        .unwrap_or(false)
                },
                "shared roles or confirmed login identity missing",
            )
            .await?;
            ready(&ui, &owner, &connection).await?;
            require(
                connection.updates.borrow().account.as_deref() == Some("StellaTest"),
                "mine was guessed rather than Session-confirmed",
            )?;
            wait(
                || ui.get_reply_side_marks().row_count() == 6,
                "visible role marks missing",
            )
            .await?;
            let marks = ui.get_reply_side_marks().iter().collect::<Vec<_>>();
            require(
                marks.iter().map(|m| m.row).collect::<Vec<_>>() == [3, 4, 6, 10, 14, 18],
                "wrong marker rows",
            )?;
            require(
                marks[0].first && !marks[1].first && marks[2].mine && !marks[2].author,
                "merged/own role marks changed",
            )?;
            let original = owner.export_output(&connection.updates.borrow(), None)?;
            let native = owner.view.borrow().clone().ok_or("native reader missing")?;
            native.find_changed();
            ui.invoke_reply_navigation_action(0);
            require(
                ui.get_reply_navigation_available()
                    && !ui.get_reply_navigation_ready()
                    && !ui.get_reply_navigation_open(),
                "a geometry repaint changed chrome or authorized stale navigation",
            )?;
            ready(&ui, &owner, &connection).await?;
            menu(&ui, &owner, &connection, 12, 50, Action::ReplyNavigation).await?;
            require(
                ui.get_reply_navigation_open(),
                "background menu did not open native reply controls",
            )?;
            ready(&ui, &owner, &connection).await?;
            count(&ui, 0, 4)?;
            for (action, current) in [(3, 1), (3, 2), (3, 3), (3, 4), (3, 1), (2, 4)] {
                step(&ui, &owner, &connection, action, current, 4).await?;
            }
            let group_anchor = ui.get_article_offset_y();
            ui.invoke_reply_navigation_action(5);
            require(
                ui.get_article_offset_y() == group_anchor,
                "choosing a reply group jumped the viewport",
            )?;
            count(&ui, 0, 2)?;
            ui.invoke_article_navigate(4);
            ready(&ui, &owner, &connection).await?;
            step(&ui, &owner, &connection, 3, 1, 2).await?;
            step(&ui, &owner, &connection, 3, 2, 2).await?;
            require(
                owner.export_output(&connection.updates.borrow(), None)? == original,
                "UI role navigation changed the exported Rust frame",
            )?;
            ui.invoke_reply_navigation_action(1);
            ui.invoke_find_action(0);
            wait(
                || ui.get_find_open(),
                "search did not open before reply handoff",
            )
            .await?;
            ready(&ui, &owner, &connection).await?;
            ui.invoke_reply_navigation_action(0);
            wait(
                || ui.get_reply_navigation_open() && !ui.get_find_open(),
                "search close/repaint rejected the leased reply toolbar",
            )
            .await?;
            ready(&ui, &owner, &connection).await?;
            verification::capture(&ui, &directory, "roles-and-controls.png")?;
            menu(&ui, &owner, &connection, 7, 4, Action::ReplyFocus).await?;
            wait(
                || {
                    verification::foreground(&ui, true);
                    ui.get_reply_navigation_groups().row_data(2) == Some(true)
                        && owner
                            .output_identity(&connection.updates.borrow())
                            .is_some()
                },
                "marked account group not reprojected",
            )
            .await?;
            ui.invoke_reply_navigation_action(6);
            step(&ui, &owner, &connection, 3, 1, 1).await?;
            step(&ui, &owner, &connection, 2, 1, 1).await?;
            owner.replace_rules(Rules::default().edit_policy(
                "Alice",
                "",
                Policy::MaskRow,
                false,
            )?);
            wait(
                || {
                    verification::foreground(&ui, true);
                    owner
                        .output_identity(&connection.updates.borrow())
                        .is_some()
                        && ui.get_reply_navigation_groups().row_data(0) == Some(false)
                },
                "whole-row mask retained author targets",
            )
            .await?;
            require(
                ui.get_reply_side_marks().iter().all(|m| !m.author),
                "hidden role leaked through a side marker",
            )?;
            require(
                owner.toggle_reveal(),
                "explicit reveal refused current policy",
            )?;
            wait(
                || {
                    verification::foreground(&ui, true);
                    owner
                        .output_identity(&connection.updates.borrow())
                        .is_some()
                        && ui.get_reply_navigation_groups().row_data(0) == Some(true)
                },
                "reveal did not restore author rows",
            )
            .await?;
            owner.replace_rules(Rules::default());
            ready(&ui, &owner, &connection).await?;
            require(
                !ui.get_reply_navigation_open(),
                "new rule scope retained the prior toolbar",
            )?;
            ui.invoke_reply_navigation_action(0);
            require(
                ui.get_reply_navigation_open(),
                "new rule scope could not reopen navigation",
            )?;
            ui.set_zoom(0.5);
            ready(&ui, &owner, &connection).await?;
            verification::capture(&ui, &directory, "wide-gutter-labels.png")?;
            ui.set_zoom(2.);
            ready(&ui, &owner, &connection).await?;
            ui.invoke_article_navigate(4);
            ready(&ui, &owner, &connection).await?;
            ui.invoke_reply_navigation_action(4);
            step(&ui, &owner, &connection, 3, 1, 4).await?;
            step(&ui, &owner, &connection, 3, 2, 4).await?;
            step(&ui, &owner, &connection, 3, 3, 4).await?;
            require(
                ui.get_article_offset_y() > 0.,
                "offscreen reply did not scroll locally",
            )?;
            let held = ui.get_reply_navigation_count();
            ui.set_native_composing(true);
            ui.invoke_reply_navigation_action(3);
            require(
                ui.get_reply_navigation_count() == held,
                "IME state advanced local navigation",
            )?;
            ui.set_native_composing(false);
            verification::foreground(&ui, false);
            ui.invoke_reply_navigation_action(3);
            require(
                ui.get_reply_navigation_count() == held,
                "background input advanced local navigation",
            )?;
            verification::foreground(&ui, true);
            ui.invoke_reply_navigation_action(1);
            require(
                !ui.get_reply_navigation_open(),
                "close retained native reply toolbar",
            )?;
            ready(&ui, &owner, &connection).await?;
            ui.invoke_reply_navigation_action(0);
            require(
                ui.get_reply_navigation_open(),
                "reopen did not retain current role index",
            )?;
            let settings = pttstella::settings::AppSettings {
                article_mode: pttstella::settings::ArticleMode::Terminal,
                ..Default::default()
            };
            owner.preferences_changed(&settings)?;
            wait(
                || {
                    !ui.get_reply_navigation_open()
                        && !ui.get_reply_navigation_available()
                        && ui.get_reply_side_marks().row_count() == 0
                },
                "terminal handoff retained reply index or marks",
            )
            .await?;
            require(
                owner.requests.get() == 0,
                "local reply navigation emitted a station fetch",
            )?;
            std::fs::write(directory.join("reply-navigation.json"),serde_json::to_vec_pretty(&serde_json::json!({"author_rows":[3,4,10,18],"mine_rows":[6,14],"focused_rows":[7],"station_inputs":0,"group_wrap":true,"mask_reveal":true,"source_export_unchanged_before_account_actions":true,"local_scroll":true,"composition_and_background_rejected":true,"toolkit_events":true,"actual_OS_input":false})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            Ok(())
        },
    )
}
