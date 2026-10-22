//! Rust → renderer contract. Reports are not bridge packets.
use super::presentation::Paint;
use pttstella::{
    filters, media, selection,
    settings::{DisplayProfile, DisplaySettings, HoverPosition, ToolOpacity},
    vim,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub(super) enum Packet<'a> {
    SitesOpen {
        request: u64,
        data: super::sites::Snapshot,
    },
    SitesResult {
        request: u64,
        data: super::sites::Snapshot,
        feedback: Feedback<super::sites::Status>,
    },
    StationMouseRejected {
        lease: pttstella::mouse::input::Lease,
        message: String,
    },
    BookmarksOpen {
        request: u64,
        data: super::bookmarks::Snapshot,
    },
    BookmarksResult {
        request: u64,
        data: super::bookmarks::Snapshot,
        feedback: Feedback<SaveStatus>,
    },
    Localization {
        language: &'static str,
        resource: &'static str,
        fallback: &'static str,
    },
    Paint(&'a Paint),
    Stop,
    Notice(String),
    SurfaceProbe(String),
    KeyboardState(vim::Status),
    LoginResult(LoginResult),
    FiltersOpen(FiltersOpen),
    FiltersResult(FiltersResult),
    HoverSettings(HoverPosition),
    ToolsOpacity(ToolOpacity),
    ToolsAccess(pttstella::settings::ToolAccess),
    HoverReset {
        media: bool,
    },
    HoverRejected(u64),
    MediaHover(MediaHover),
    MediaOpen(MediaOpen),
    MediaImage(MediaImage),
    MediaError {
        request: u64,
        message: String,
    },
    MediaExternal {
        request: u64,
        ok: bool,
        message: Option<String>,
    },
    MediaReset,
    InlineOpen(super::inline_images::Open),
    InlineImage {
        batch: u64,
        index: usize,
        image: MediaImage,
    },
    InlineError {
        batch: u64,
        index: usize,
        message: String,
    },
    InlineReset(bool),
    ReadingMove {
        generation: u64,
        article: u64,
        intent: pttstella::navigation::Intent,
    },
    SettingsOpen(Box<SettingsOpen>),
    SettingsVisibility(bool),
    SettingsClose(bool),
    FontPreview(FontPreview),
    FontCatalog {
        request: u64,
        families: Vec<String>,
        notice: Option<String>,
    },
    SettingsSaved(Feedback<SaveStatus>),
    ProfileResult(ProfileResult),
    NavigationResult(Feedback<NavigationStatus>),
    DestinationResult {
        request: u64,
        feedback: Feedback<DestinationStatus>,
    },
    InputRejected(String),
    RecentBoards(RecentBoardsPacket),
    PasteResult(Feedback<PasteStatus>),
    ExportResult(Feedback<ExportStatus>),
    LinkResult(Feedback<LinkStatus>),
    Selection(SelectionReply),
}

#[derive(Serialize)]
pub(super) struct RecentBoardsPacket {
    pub boards: Vec<pttstella::board_navigation::BoardName>,
    pub notice: Option<String>,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(super) enum LoginResult {
    Idle,
    Opened {
        account: String,
        saved: bool,
        message: String,
    },
    Message {
        message: String,
    },
    Sent,
}

#[derive(Serialize)]
pub(super) struct FiltersOpen {
    pub allow_color: u8,
    pub entries: Vec<filters::Entry>,
    pub warning: Option<String>,
}
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(super) enum FiltersResult {
    Preview {
        request: u64,
        entries: Vec<filters::Entry>,
        message: String,
    },
    ImportRejected {
        request: u64,
        message: String,
    },
    Saved {
        allow_color: u8,
        entries: Vec<filters::Entry>,
        message: String,
    },
    Rejected {
        message: String,
    },
}
#[derive(Serialize)]
pub(super) struct MediaOpen {
    pub request: u64,
    pub target: media::Target,
}
#[derive(Serialize)]
pub(super) struct MediaHover {
    pub sequence: u64,
    pub request: u64,
    pub target: media::Target,
    pub position: HoverPosition,
}
#[derive(Serialize)]
pub(super) struct MediaImage {
    pub request: u64,
    pub data: String,
    pub mime: &'static str,
    pub width: usize,
    pub height: usize,
    pub animated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation_pixels: Option<usize>,
    pub cached: bool,
}
#[derive(Serialize)]
pub(super) struct SettingsOpen {
    pub article_mode: pttstella::settings::ArticleMode,
    pub article_boundary_navigation: bool,
    pub hover: HoverPosition,
    pub tools_access: pttstella::settings::ToolAccess,
    pub tools_opacity: ToolOpacity,
    pub language: pttstella::i18n::Language,
    pub profile: DisplayProfile,
    pub values: DisplaySettings,
    pub defaults: DisplaySettings,
    pub presets: BTreeMap<&'static str, DisplaySettings>,
    pub warning: Option<String>,
    pub sample: Paint,
}
#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum FontPreview {
    Ready { request: u64, packet: Box<Paint> },
    Rejected { request: u64, error: String },
}
#[derive(Serialize)]
pub(super) struct ImportedProfile {
    pub profile: DisplayProfile,
    pub name: String,
    pub values: DisplaySettings,
    pub sample: Paint,
}
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum ProfileResult {
    Cancelled,
    Exported {
        notice: String,
    },
    Imported {
        profile: Box<ImportedProfile>,
        notice: String,
    },
    Rejected {
        notice: String,
    },
}
#[derive(Serialize)]
pub(super) struct Feedback<S> {
    pub status: S,
    pub notice: Option<String>,
}
impl<S> Feedback<S> {
    pub(super) fn new(status: S, notice: impl Into<String>) -> Self {
        Self {
            status,
            notice: Some(notice.into()),
        }
    }
    pub(super) fn quiet(status: S) -> Self {
        Self {
            status,
            notice: None,
        }
    }
}
macro_rules! statuses {
    ($($name:ident { $($variant:ident),+ }),+ $(,)?)=>{$(
        #[derive(Serialize)]
        #[serde(rename_all="snake_case")]
        pub(super) enum $name {$($variant),+}
    )+};
}
statuses! {
    NavigationStatus { Sent, Rejected },
    DestinationStatus { Started, Rejected },
    PasteStatus { Queued, Preview, Cancelled, Rejected },
    ExportStatus { Saved, Cancelled, Failed },
    LinkStatus { Requested, Failed },
    SaveStatus { Saved, Rejected },
}
#[derive(Serialize)]
pub(super) struct SelectionReply {
    pub presentation: u64,
    pub generation: u64,
    pub revision: u64,
    pub request: u64,
    pub value: Option<selection::SelectedText>,
}
