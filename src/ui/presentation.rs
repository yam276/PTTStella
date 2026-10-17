//! One projection for rendering, selection, links and exports, with freshness checks.
use pttstella::{
    connection::{Phase, View},
    encoding, filters,
    links::{self, Link},
    media::{self, hover::Lease as HoverLease},
    navigation::{self, Context},
    screen::{Frame, Screen},
    settings::DisplaySettings,
};
use serde::Serialize;

#[derive(Clone, Serialize)]
pub(super) struct Paint {
    pub(super) mouse: pttstella::mouse::input::Snapshot,
    pub(super) output_pending: bool,
    pub(super) presentation: u64,
    pub(super) masked_pushes: usize,
    pub(super) revealed: bool,
    pub(super) filter_reason: String,
    pub(super) image_links: Vec<usize>,
    pub(super) hover: Option<HoverLease>,
    pub(super) keep_alive: bool,
    pub(super) keep_alive_ready: bool,
    pub(super) keep_alive_sent: u64,
    pub(super) keep_alive_failure: Option<String>,
    pub(super) input_epoch: u64,
    pub(super) input_pending: bool,
    pub(super) login_pending: bool,
    pub(super) destination: pttstella::navigation_job::Progress,
    pub(super) connected: bool,
    pub(super) navigation: Context,
    pub(super) frame: Frame,
    pub(super) generation: u64,
    pub(super) probe: bool,
    pub(super) links: Vec<Link>,
    pub(super) settings: DisplaySettings,
}
pub(super) fn current_presentation(
    p: &Paint,
    presentation: u64,
    generation: u64,
    revision: u64,
    filters: &filters::State,
    current: &View,
) -> bool {
    !current.output_pending
        && !p.output_pending
        && p.presentation == presentation
        && filters.version() == presentation
        && (p.generation, p.frame.revision) == (generation, revision)
        && (current.generation, current.frame.revision) == (generation, revision)
}

pub(super) fn paint(
    view: &View,
    probe: bool,
    settings: &DisplaySettings,
    filters: &filters::State,
) -> Paint {
    let projection = filters.project(view.generation, &view.frame);
    let links = if view.output_pending {
        Vec::new()
    } else {
        links::detect(&projection.frame)
    };
    let image_links = links
        .iter()
        .enumerate()
        .filter_map(|(index, link)| {
            media::classify(&link.url)
                .ok()
                .flatten()
                .filter(|target| target.kind == media::Kind::Image)
                .map(|_| index)
        })
        .collect();
    Paint {
        mouse: view.mouse,
        output_pending: view.output_pending,
        presentation: filters.version(),
        masked_pushes: projection.matches.len(),
        revealed: projection.revealed,
        filter_reason: projection
            .matches
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>()
            .join("、"),
        image_links,
        hover: None,
        keep_alive: view.keep_alive,
        keep_alive_ready: view.keep_alive_ready,
        keep_alive_sent: view.keep_alive_sent,
        keep_alive_failure: view.keep_alive_failure.clone(),
        input_epoch: view.input_epoch,
        input_pending: view.input_pending || view.output_pending,
        login_pending: view.login_pending,
        destination: view.destination.clone(),
        connected: view.phase == Phase::Connected,
        navigation: view.context(),
        settings: settings.clone(),
        links,
        frame: projection.frame,
        generation: view.generation,
        probe,
    }
}

pub(super) fn sample_packet(frame: &Frame, settings: &DisplaySettings) -> Paint {
    Paint {
        mouse: pttstella::mouse::input::Snapshot::default(),
        output_pending: false,
        presentation: 0,
        masked_pushes: 0,
        revealed: false,
        filter_reason: String::new(),
        image_links: Vec::new(),
        hover: None,
        keep_alive: false,
        keep_alive_ready: false,
        keep_alive_sent: 0,
        keep_alive_failure: None,
        input_epoch: 0,
        input_pending: false,
        login_pending: false,
        destination: pttstella::navigation_job::Progress::Idle,
        connected: false,
        navigation: navigation::analyze(frame),
        frame: frame.clone(),
        generation: 0,
        probe: false,
        links: Vec::new(),
        settings: settings.clone(),
    }
}

pub(super) fn font_sample() -> Result<Frame, String> {
    let mut sample = Screen::new(24, 4).map_err(|error| error.to_string())?;
    sample.feed(b"ASCII xyz 0123456789\r\n\x1b[31;44m\xa4\x1b[32;45m\xa4\x1b[0m ");
    sample.feed(&encoding::encode("中文字型\r\n").map_err(|e| e.to_string())?);
    sample.feed(b"\x1b[31;44m");
    sample.feed(&encoding::encode("█▌◢◣◤◥ ●○◎⊙").map_err(|e| e.to_string())?);
    sample.feed(b"\x1b[0m\r\n");
    sample.feed(&encoding::encode("┌──┐ │  │ └──┘").map_err(|e| e.to_string())?);
    sample.feed(b"\x1b[0m\x1b[?25l");
    Ok(sample.frame())
}

#[cfg(test)]
mod filter_tests {
    use super::*;
    #[test]
    fn presentation_consumers_reject_old_rules_reveal_session_and_frame() {
        let pages: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/filters.json")).unwrap();
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&encoding::encode(pages["reading"].as_str().unwrap()).unwrap());
        let view = View {
            mouse: pttstella::mouse::input::Snapshot::default(),
            output_pending: false,
            frame: screen.frame(),
            generation: 4,
            input_epoch: 0,
            input_pending: false,
            login_pending: false,
            login_guidance: pttstella::login::Guidance::Idle,
            destination: pttstella::navigation_job::Progress::Idle,
            board_visit: None,
            phase: Phase::Connected,
            message: String::new(),
            received: 1,
            keep_alive: false,
            keep_alive_ready: false,
            keep_alive_sent: 0,
            keep_alive_failure: None,
        };
        let mut filters =
            filters::State::new(filters::Rules::default().edit("Alice", "", false).unwrap());
        let p = paint(&view, false, &DisplaySettings::default(), &filters);
        assert_eq!(p.masked_pushes, 2);
        assert_eq!(p.links.len(), 1);
        assert!(current_presentation(
            &p,
            p.presentation,
            view.generation,
            view.frame.revision,
            &filters,
            &view
        ));
        filters.reveal(view.generation, view.frame.revision, true);
        assert!(!current_presentation(
            &p,
            p.presentation,
            view.generation,
            view.frame.revision,
            &filters,
            &view
        ));
        let shown = paint(&view, false, &DisplaySettings::default(), &filters);
        assert_eq!(shown.links.len(), 2);
        filters.replace(filters.rules().clone());
        assert!(!current_presentation(
            &shown,
            shown.presentation,
            view.generation,
            view.frame.revision,
            &filters,
            &view
        ));
        let p = paint(&view, false, &DisplaySettings::default(), &filters);
        let mut current = view.clone();
        current.generation += 1;
        assert!(!current_presentation(
            &p,
            p.presentation,
            view.generation,
            view.frame.revision,
            &filters,
            &current
        ));
        current = view.clone();
        current.frame.revision += 1;
        assert!(!current_presentation(
            &p,
            p.presentation,
            view.generation,
            view.frame.revision,
            &filters,
            &current
        ));
    }
}
