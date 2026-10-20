//! Independently authored wire examples; not a serialize/deserialize round trip.
use super::outbound::*;
use pttstella::{media, settings::HoverPosition, vim};
use serde_json::Value;
#[test]
fn outbound_results_match_the_shared_wire_examples() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("probe-bookmarks.json"),
        include_bytes!("../../tests/fixtures/bookmarks.json"),
    )
    .unwrap();
    let bookmarks = super::bookmarks::State::load(Some(directory.path()));
    let station_snapshot = || {
        let target = pttstella::sites::Target::new(
            "telnet://example.com".into(),
            pttstella::encoding::TextEncoding::Gbk,
        )
        .unwrap();
        super::sites::Snapshot {
            sites: pttstella::sites::Sites {
                version: 1,
                default: target.clone(),
                favourites: Vec::new(),
            },
            current: target,
            writable: true,
            warning: None,
        }
    };
    let actual = vec![
        (
            "localization",
            Packet::Localization {
                language: "en-US",
                resource: "connect = Connect\n",
                fallback: "connect = 連線\n",
            },
        ),
        (
            "station_mouse_rejected",
            Packet::StationMouseRejected {
                lease: pttstella::mouse::input::Lease {
                    stamp: pttstella::navigation::Stamp {
                        generation: 7,
                        revision: 4,
                        input_epoch: 2,
                    },
                    mode_epoch: 5,
                    policy_epoch: 3,
                    transaction: 9,
                },
                message: "滑鼠事件未送出".into(),
            },
        ),
        (
            "bookmarks_open",
            Packet::BookmarksOpen {
                request: 10,
                data: bookmarks.snapshot(),
            },
        ),
        (
            "bookmarks_saved",
            Packet::BookmarksResult {
                request: 11,
                data: bookmarks.snapshot(),
                feedback: Feedback::new(SaveStatus::Saved, "已保存"),
            },
        ),
        (
            "bookmarks_rejected",
            Packet::BookmarksResult {
                request: 12,
                data: bookmarks.snapshot(),
                feedback: Feedback::new(SaveStatus::Rejected, "失敗"),
            },
        ),
        ("stop", Packet::Stop),
        ("notice", Packet::Notice("訊息".into())),
        ("surface_probe", Packet::SurfaceProbe("decode".into())),
        ("login_idle", Packet::LoginResult(LoginResult::Idle)),
        (
            "login_opened",
            Packet::LoginResult(LoginResult::Opened {
                account: "Synthetic".into(),
                saved: true,
                message: "已保存".into(),
            }),
        ),
        (
            "login_message",
            Packet::LoginResult(LoginResult::Message {
                message: "取消".into(),
            }),
        ),
        ("login_sent", Packet::LoginResult(LoginResult::Sent)),
        (
            "filters_open",
            Packet::FiltersOpen(FiltersOpen {
                entries: vec![],
                warning: None,
            }),
        ),
        (
            "filters_saved",
            Packet::FiltersResult(FiltersResult::Saved {
                entries: vec![],
                message: "已保存".into(),
            }),
        ),
        (
            "filters_rejected",
            Packet::FiltersResult(FiltersResult::Rejected {
                message: "失敗".into(),
            }),
        ),
        (
            "sites_open",
            Packet::SitesOpen {
                request: 11,
                data: station_snapshot(),
            },
        ),
        (
            "sites_result",
            Packet::SitesResult {
                request: 12,
                data: station_snapshot(),
                feedback: Feedback::new(super::sites::Status::Saved, "已保存"),
            },
        ),
        (
            "tools_opacity",
            Packet::ToolsOpacity(pttstella::settings::ToolOpacity::try_from(65).unwrap()),
        ),
        (
            "tools_access",
            Packet::ToolsAccess(pttstella::settings::ToolAccess {
                right_click_menu: true,
                show_floating_button: false,
            }),
        ),
        ("hover_settings", Packet::HoverSettings(HoverPosition::Off)),
        ("hover_reset", Packet::HoverReset { media: true }),
        ("hover_rejected", Packet::HoverRejected(8)),
        ("media_reset", Packet::MediaReset),
        (
            "media_error",
            Packet::MediaError {
                request: 3,
                message: "錯誤".into(),
            },
        ),
        (
            "media_external",
            Packet::MediaExternal {
                request: 3,
                ok: false,
                message: Some("錯誤".into()),
            },
        ),
        (
            "media_image",
            Packet::MediaImage(MediaImage {
                request: 3,
                data: "AA==".into(),
                mime: "image/png",
                width: 1,
                height: 2,
                animated: false,
                cached: true,
            }),
        ),
        (
            "font_catalog",
            Packet::FontCatalog {
                request: 12,
                families: vec!["Menlo".into(), "Songti TC".into()],
                notice: None,
            },
        ),
        (
            "font_rejected",
            Packet::FontPreview(FontPreview::Rejected {
                request: 4,
                error: "無效".into(),
            }),
        ),
        (
            "profile_cancelled",
            Packet::ProfileResult(ProfileResult::Cancelled),
        ),
        (
            "profile_exported",
            Packet::ProfileResult(ProfileResult::Exported {
                notice: "完成".into(),
            }),
        ),
        (
            "profile_rejected",
            Packet::ProfileResult(ProfileResult::Rejected {
                notice: "無效".into(),
            }),
        ),
        (
            "recent_boards",
            Packet::RecentBoards(RecentBoardsPacket {
                boards: vec![pttstella::board_navigation::BoardName::parse("Stella").unwrap()],
                notice: None,
            }),
        ),
        ("input_rejected", Packet::InputRejected("舊連線".into())),
        (
            "selection",
            Packet::Selection(SelectionReply {
                presentation: 2,
                generation: 7,
                revision: 9,
                request: 11,
                value: None,
            }),
        ),
        (
            "settings_saved_saved",
            Packet::SettingsSaved(Feedback::new(SaveStatus::Saved, "回覆")),
        ),
        (
            "settings_saved_rejected",
            Packet::SettingsSaved(Feedback::new(SaveStatus::Rejected, "回覆")),
        ),
        (
            "navigation_result_sent",
            Packet::NavigationResult(Feedback::new(NavigationStatus::Sent, "回覆")),
        ),
        (
            "destination_result_started",
            Packet::DestinationResult {
                request: 7,
                feedback: Feedback::quiet(DestinationStatus::Started),
            },
        ),
        (
            "destination_result_rejected",
            Packet::DestinationResult {
                request: 8,
                feedback: Feedback::new(DestinationStatus::Rejected, "回覆"),
            },
        ),
        (
            "navigation_result_rejected",
            Packet::NavigationResult(Feedback::new(NavigationStatus::Rejected, "回覆")),
        ),
        (
            "paste_result_queued",
            Packet::PasteResult(Feedback::new(PasteStatus::Queued, "回覆")),
        ),
        (
            "paste_result_preview",
            Packet::PasteResult(Feedback::new(PasteStatus::Preview, "回覆")),
        ),
        (
            "paste_result_cancelled",
            Packet::PasteResult(Feedback::new(PasteStatus::Cancelled, "回覆")),
        ),
        (
            "paste_result_rejected",
            Packet::PasteResult(Feedback::new(PasteStatus::Rejected, "回覆")),
        ),
        (
            "export_result_saved",
            Packet::ExportResult(Feedback::new(ExportStatus::Saved, "回覆")),
        ),
        (
            "export_result_cancelled",
            Packet::ExportResult(Feedback::new(ExportStatus::Cancelled, "回覆")),
        ),
        (
            "export_result_failed",
            Packet::ExportResult(Feedback::new(ExportStatus::Failed, "回覆")),
        ),
        (
            "link_result_requested",
            Packet::LinkResult(Feedback::new(LinkStatus::Requested, "回覆")),
        ),
        (
            "link_result_failed",
            Packet::LinkResult(Feedback::new(LinkStatus::Failed, "回覆")),
        ),
        (
            "media_open",
            Packet::MediaOpen(MediaOpen {
                request: 3,
                target: media::Target {
                    url: "https://example.com/a.png".into(),
                    kind: media::Kind::Image,
                    image_url: None,
                },
            }),
        ),
        (
            "media_hover",
            Packet::MediaHover(MediaHover {
                sequence: 5,
                request: 3,
                target: media::Target {
                    url: "https://example.com/a.png".into(),
                    kind: media::Kind::Image,
                    image_url: None,
                },
                position: HoverPosition::Off,
            }),
        ),
        (
            "keyboard_state",
            Packet::KeyboardState(vim::Status {
                enabled: true,
                mode: "normal",
                pending: false,
                prefix: "4".into(),
                suspended_reason: None,
            }),
        ),
        ("inline_reset", Packet::InlineReset(true)),
        (
            "reading_move",
            Packet::ReadingMove {
                generation: 7,
                article: 2,
                intent: pttstella::navigation::Intent::NextPage,
            },
        ),
        (
            "inline_open",
            Packet::InlineOpen(super::inline_images::Open {
                batch: 2,
                first_line: 1,
                article_id: Some(2),
                presentation: 3,
                generation: 7,
                revision: 4,
                entries: vec![super::inline_images::Entry {
                    index: 1,
                    row: 8,
                    key: 0,
                }],
                limited: false,
            }),
        ),
        (
            "inline_error",
            Packet::InlineError {
                batch: 2,
                index: 1,
                message: "失敗".into(),
            },
        ),
        (
            "inline_image",
            Packet::InlineImage {
                batch: 2,
                index: 1,
                image: MediaImage {
                    request: 2,
                    data: "AQID".into(),
                    mime: "image/png",
                    width: 2,
                    height: 1,
                    animated: false,
                    cached: false,
                },
            },
        ),
        (
            "settings_launch",
            Packet::SettingsLaunch(super::protocol::SettingsFeature::Sites),
        ),
        ("settings_visibility", Packet::SettingsVisibility(true)),
    ];
    let expected: Vec<Value> =
        serde_json::from_str(include_str!("../../tests/fixtures/ui-packets.json")).unwrap();
    assert_eq!(actual.len(), expected.len());
    for ((name, packet), expected) in actual.into_iter().zip(expected) {
        assert_eq!(name, expected["name"].as_str().unwrap());
        assert_eq!(
            serde_json::to_value(packet).unwrap(),
            expected["packet"],
            "{name}"
        );
    }
}

#[test]
fn frame_and_preview_packets_keep_typed_identity_and_data_under_payload() {
    use super::{preferences::Preferences, presentation::sample_packet};
    use pttstella::{
        screen::Screen,
        settings::{DisplayProfile, DisplaySettings},
    };
    let screen = Screen::new(2, 1).unwrap();
    let settings = DisplaySettings {
        version: 1,
        ascii_font: "ASCII".into(),
        cjk_font: "中文".into(),
        font_size: 18.0,
        baseline: 19.0,
    };
    let paint = sample_packet(&screen.frame(), &settings);
    let value = serde_json::to_value(Packet::Paint(&paint)).unwrap();
    assert_eq!(value["kind"], "paint");
    assert_eq!(value["payload"]["generation"], 0);
    assert_eq!(value["payload"]["output_pending"], false);
    assert_eq!(
        value["payload"]["mouse"],
        serde_json::json!({"enabled":true,"policy_epoch":0,"modes":{"tracking":"off","encoding":"legacy","epoch":0},"ready":false,"route":"local","active":null})
    );
    assert_eq!(value["payload"]["settings"]["cjk_font"], "中文");
    assert!(value.get("frame").is_none());
    let directory = tempfile::tempdir().unwrap();
    let preferences = Preferences::load(Some(directory.path())).unwrap();
    let open = serde_json::to_value(preferences.open_packet()).unwrap();
    assert_eq!(open["kind"], "settings_open");
    assert!(open["payload"]["sample"]["frame"].is_object());
    let preview =
        serde_json::to_value(preferences.preview_packet(41, DisplayProfile::default())).unwrap();
    assert_eq!(preview["kind"], "font_preview");
    assert_eq!(preview["payload"]["request"], 41);
    assert!(preview["payload"]["packet"]["frame"].is_object());
}

#[test]
fn localization_bootstrap_matches_the_independent_wire_example() {
    let packet = Packet::Localization {
        language: "en-US",
        resource: "connect = Connect\n",
        fallback: "connect = 連線\n",
    };
    let expected: Value =
        serde_json::from_str(include_str!("../../tests/fixtures/localization.json")).unwrap();
    assert_eq!(serde_json::to_value(packet).unwrap(), expected);
}
