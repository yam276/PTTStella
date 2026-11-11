//! Same static chrome refresh at startup and after a successful locale save.
//! Only strings/models of labels change; no controls or user data are recreated.
use crate::{EditorState, StationColorState, TerminalWindow, UiLabels};
use pttstella::msg;
use slint::{ComponentHandle, ModelRc, VecModel};
pub fn refresh(ui: &TerminalWindow) {
    ui.invoke_filter_status_refresh();
    ui.set_bookmarks_label(msg!("bookmarks-title").into());
    ui.set_sites_label(msg!("sites-title").into());
    let labels = ui.global::<UiLabels>();
    labels.set_updates_station(msg!("article-updates-station").into());
    labels.set_reply_title(msg!("reply-navigation-title").into());
    labels.set_reply_author(msg!("reply-navigation-author").into());
    labels.set_reply_mine(msg!("reply-navigation-mine").into());
    labels.set_reply_both(msg!("reply-navigation-both").into());
    labels.set_reply_focused(msg!("reply-navigation-focused").into());
    labels.set_reply_unconfirmed(msg!("reply-navigation-unconfirmed").into());
    labels.set_reply_previous(msg!("reply-navigation-prev").into());
    labels.set_reply_next(msg!("reply-navigation-next").into());
    labels.set_editor_modes(ModelRc::new(VecModel::from(vec![
        msg!("draft-wysiwyg").into(),
        msg!("draft-plain").into(),
        msg!("draft-source").into(),
    ])));
    labels.set_fullscreen(msg!("menu-fullscreen").into());
    labels.set_about(msg!("settings-about").into());
    labels.set_close(msg!("close").into());
    labels.set_toast_title(msg!("toast-notice-title").into());
    labels.set_toast_connecting(msg!("toast-connecting-title").into());
    labels.set_toast_disconnected(msg!("toast-disconnected-title").into());
    labels.set_toast_error(msg!("toast-connection-error-title").into());
    labels.set_toast_dismiss(msg!("toast-dismiss").into());
    labels.set_local_drafts(msg!("draft-open").into());
    labels.set_article_heading(msg!("slint-article-heading").into());
    labels.set_cancel_load(msg!("article-load-cancel").into());
    labels.set_cancel(msg!("cancel").into());
    labels.set_bookmark_capture(msg!("bookmarks-list-loading").into());
    labels.set_next_section(msg!("article-load-next-segment").into());
    labels.set_resume_load(msg!("article-load-resume").into());
    labels.set_resume_reading(msg!("slint-resume-reading").into());
    labels.set_connect(msg!("connect").into());
    labels.set_reconnect(msg!("toast-reconnect").into());
    labels.set_execute(msg!("slint-execute").into());
    labels.set_select_text(msg!("slint-select-text").into());
    labels.set_drag_scroll(msg!("slint-drag-scroll").into());
    labels.set_terminal_input(msg!("slint-terminal-input").into());
    labels.set_article_input(msg!("slint-article-input").into());
    labels.set_preview_image(msg!("slint-preview-image").into());
    labels.set_expand_image(msg!("slint-expand-image").into());
    labels.set_editor_save(msg!("draft-save").into());
    labels.set_editor_transfer(msg!("draft-transfer").into());
    labels.set_editor_stop(msg!("slint-stop-requested").into());
    labels.set_editor_cancel_job(msg!("slint-cancel-job").into());
    labels.set_editor_close(msg!("close").into());
    labels.set_editor_new(msg!("draft-new").into());
    labels.set_editor_conflict(msg!("draft-conflict-copy").into());
    labels.set_editor_delete(msg!("draft-delete").into());
    labels.set_editor_title(msg!("draft-title").into());
    labels.set_editor_undo(msg!("menu-undo").into());
    labels.set_editor_redo(msg!("menu-redo").into());
    labels.set_editor_selection(msg!("draft-selection").into());
    labels.set_editor_palette(msg!("draft-preview").into());
    labels.set_editor_color_hint(msg!("slint-draft-color-hint").into());
    labels.set_editor_body(msg!("draft-body").into());
    labels.set_editor_confirm(msg!("draft-confirm").into());
    labels.set_editor_confirm_transfer(msg!("draft-transfer-confirm").into());
    labels.set_editor_cancel(msg!("cancel").into());
    labels.set_palette_hint(msg!("editor-color-palette-hint").into());
    labels.set_output_actions(ModelRc::new(VecModel::from(vec![
        msg!("context-copy").into(),
        msg!("context-copy-ansi").into(),
        msg!("context-export-page").into(),
        msg!("context-export-rows").into(),
        msg!("copy-png").into(),
        msg!("copy-rows-png").into(),
        msg!("context-select-all").into(),
        msg!("slint-clear-selection").into(),
    ])));
    ui.set_login_label(msg!("login").into());
    ui.set_login_title(msg!("login-title").into());
    ui.set_login_account_label(msg!("account").into());
    ui.set_login_password_label(msg!("password").into());
    ui.set_login_remember_label(msg!("login-remember").into());
    ui.set_login_forget_label(msg!("login-forget").into());
    ui.set_login_cancel_label(msg!("cancel").into());
    ui.set_login_cancel_attempt_label(msg!("login-cancel-attempt").into());
    ui.set_media_close_label(msg!("close").into());
    ui.set_media_retry_label(msg!("retry").into());
    ui.set_media_external_label(msg!("media-external").into());
    ui.set_media_fit_label(msg!("media-fit").into());
    ui.set_media_original_label(msg!("media-original").into());
    ui.set_media_copy_label(msg!("context-copy-picture").into());
    ui.set_media_save_label(msg!("context-save-picture").into());
    ui.set_media_copy_url_label(msg!("context-copy-link").into());
    ui.set_media_hover_label(msg!("hover-position").into());
    ui.set_media_hover_help(msg!("hover-help").into());
    ui.set_media_hover_choices(slint::ModelRc::new(slint::VecModel::from(vec![
        msg!("hover-top-right").into(),
        msg!("hover-follow").into(),
        msg!("close").into(),
    ])));
    ui.set_media_auto_label(msg!("auto-expand-images").into());
    ui.set_media_auto_help(msg!("auto-expand-images-help").into());
    ui.set_paste_cancel_label(msg!("cancel").into());
    ui.set_paste_send_label(msg!("paste-send").into());
    ui.set_find_title(pttstella::msg!("article-find-title").into());
    ui.set_find_placeholder(pttstella::msg!("article-find-placeholder").into());
    ui.set_find_hint(pttstella::msg!("article-find-hint").into());
    ui.set_find_previous_label(pttstella::msg!("article-find-previous").into());
    ui.set_find_next_label(pttstella::msg!("article-find-next").into());
    ui.set_find_close_label(pttstella::msg!("article-find-close").into());
    let state = ui.global::<EditorState>();
    state.set_palette_labels(crate::palette::labels());
    state.set_foreground_label(pttstella::msg!("editor-color-foreground").into());
    state.set_background_label(pttstella::msg!("editor-color-background").into());
    state.set_selection_labels(ModelRc::new(VecModel::from(vec![
        pttstella::msg!("draft-select-text").into(),
        pttstella::msg!("draft-select-line").into(),
        pttstella::msg!("draft-select-rectangle").into(),
    ])));
    let state = ui.global::<StationColorState>();
    state.set_labels(crate::palette::labels());
    state.set_title(pttstella::msg!("editor-color-title").into());
    state.set_help(pttstella::msg!("editor-color-help").into());
    state.set_foreground_label(pttstella::msg!("editor-color-foreground").into());
    state.set_background_label(pttstella::msg!("editor-color-background").into());
    state.set_sample(pttstella::msg!("editor-color-sample").into());
    state.set_hint(pttstella::msg!("editor-color-palette-hint").into());
    state.set_reset_label(pttstella::msg!("editor-color-reset").into());
    state.set_cancel_label(pttstella::msg!("editor-color-cancel").into());
    state.set_apply_label(pttstella::msg!("editor-color-apply").into());
}

/// Preserve historical diagnostic details, including user/server/OS text.
/// These comparisons are never used for command admission or screen parsing.
pub fn refresh_statuses(ui: &TerminalWindow, previous: pttstella::i18n::Language) {
    let translate = |text: slint::SharedString| -> slint::SharedString {
        pttstella::i18n::application()
            .relocalize_labels(
                previous,
                text.as_str(),
                &[
                    "image-loading",
                    "image-ready",
                    "image-ready-cached",
                    "media-play",
                    "media-stop",
                    "video-play",
                    "video-pause",
                    "video-ready",
                    "video-unsupported",
                    "video-buffering",
                    "media-paused",
                    "media-playing",
                    "station-mouse-on",
                    "station-mouse-off",
                    "station-mouse-local",
                    "station-mouse-active",
                    "station-mouse-blocked",
                    "vault-reading",
                    "login-not-account-prompt",
                    "login-stale",
                    "login-preparing",
                    "login-saved-help",
                    "login-unsaved-help",
                    "login-forgotten",
                    "draft-loading",
                    "draft-saved",
                    "draft-unsaved",
                    "draft-saving",
                    "draft-deleted",
                    "draft-conflict-saved",
                    "slint-draft-busy",
                    "slint-draft-choose",
                    "slint-draft-local",
                    "draft-transferred",
                    "draft-transfer-stopped",
                    "draft-stop-requested",
                    "editor-color-working",
                    "editor-color-done",
                    "editor-color-stopped",
                    "editor-color-stop-requested",
                ],
            )
            .map(Into::into)
            .unwrap_or(text)
    };
    ui.set_result(connection_status(ui.get_result().as_str()).into());
    ui.set_media_status(translate(ui.get_media_status()));
    ui.set_media_motion_label(translate(ui.get_media_motion_label()));
    ui.set_station_mouse_label(translate(ui.get_station_mouse_label()));
    ui.set_station_mouse_status(translate(ui.get_station_mouse_status()));
    ui.set_login_message(translate(ui.get_login_message()));
    let editor = ui.global::<EditorState>();
    editor.set_status(translate(editor.get_status()));
    let station = ui.global::<StationColorState>();
    station.set_status(translate(station.get_status()));
    ui.invoke_keyboard_updated();
    ui.invoke_keep_alive_changed();
}

pub fn connection_status(text: &str) -> String {
    use pttstella::i18n::{Language, application};
    application()
        .relocalize_status(Language::TraditionalChinese, text)
        .or_else(|| application().relocalize_status(Language::English, text))
        .unwrap_or_else(|| text.to_owned())
}
