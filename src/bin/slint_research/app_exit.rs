//! One normal exit owner. Release the root component/context even when
//! minimized; the explicit event loop stays alive for the remaining joins.
use crate::{
    TerminalWindow, article::session::ArticleSession, media::Media, sites_manager::Manager,
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

pub struct Exit {
    ui: slint::Weak<TerminalWindow>,
    media: Option<Weak<Media>>,
    article: Option<Weak<ArticleSession>>,
    sites: Option<Weak<Manager>>,
    filters: Option<Weak<crate::filters_manager::Manager>>,
    bookmarks: Option<Weak<crate::bookmarks_manager::Manager>>,
    requested: Rc<Cell<bool>>,
    outcome: Rc<RefCell<Option<Result<(), String>>>>,
    task: RefCell<Option<slint::JoinHandle<()>>>,
    window: Rc<RefCell<Option<TerminalWindow>>>,
}

impl Exit {
    pub fn mount(
        ui: &TerminalWindow,
        media: Option<&Rc<Media>>,
        article: Option<&Rc<ArticleSession>>,
        sites: Option<&Rc<Manager>>,
        filters: Option<&Rc<crate::filters_manager::Manager>>,
        bookmarks: Option<&Rc<crate::bookmarks_manager::Manager>>,
        requested: Rc<Cell<bool>>,
    ) -> Rc<Self> {
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            media: media.map(Rc::downgrade),
            article: article.map(Rc::downgrade),
            sites: sites.map(Rc::downgrade),
            filters: filters.map(Rc::downgrade),
            bookmarks: bookmarks.map(Rc::downgrade),
            requested,
            outcome: Rc::new(RefCell::new(None)),
            task: RefCell::new(None),
            window: Rc::new(RefCell::new(None)),
        });
        ui.on_app_quit({
            let owner = Rc::downgrade(&owner);
            move || {
                if let Some(owner) = owner.upgrade() {
                    owner.request();
                }
            }
        });
        ui.window().on_close_requested({
            let ui = ui.as_weak();
            move || {
                if let Some(ui) = ui.upgrade() {
                    ui.invoke_app_quit();
                }
                slint::CloseRequestResponse::KeepWindowShown
            }
        });
        owner
    }

    fn request(&self) {
        if self.requested.get() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        self.requested.set(true);
        ui.set_app_closing(true);
        let media = self.media.as_ref().and_then(Weak::upgrade);
        let article = self.article.as_ref().and_then(Weak::upgrade);
        let sites = self.sites.as_ref().and_then(Weak::upgrade);
        let filters = self.filters.as_ref().and_then(Weak::upgrade);
        let bookmarks = self.bookmarks.as_ref().and_then(Weak::upgrade);
        let outcome = self.outcome.clone();
        let window = self.window.clone();
        // The future owns only dependencies, never this owner/its JoinHandle.
        match slint::spawn_local(async move {
            if let Some(media) = &media {
                media.preferences.prepare_app_close();
            }
            if let Some(sites) = sites {
                sites.prepare_app_close();
            }
            if let Some(filters) = filters {
                filters.prepare_app_close();
            }
            if let Some(bookmarks) = bookmarks {
                bookmarks.prepare_app_close();
            }
            if let Some(article) = article {
                article.shutdown().await;
            }
            // Hiding alone retains the renderer on macOS/Windows/X11. Drop
            // the last application-owned root as well; RenderingTeardown then
            // runs with its current GL context without requiring a redraw.
            // The explicit loop survives the absence of visible windows.
            let hidden = release_window(&window);
            let stopped = match media {
                Some(media) => media.shutdown().await,
                None => Ok(()),
            };
            let result = match (hidden, stopped) {
                (Ok(()), Ok(())) => Ok(()),
                (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
                (Err(error), Err(cleanup)) => Err(format!("{error}; media: {cleanup}")),
            };
            if let Err(error) = &result {
                eprintln!("app exit: {error}");
            }
            outcome.replace(Some(result));
            if let Err(error) = slint::quit_event_loop() {
                let previous = outcome.take().and_then(Result::err);
                outcome.replace(Some(Err(match previous {
                    Some(previous) => format!("{previous}; event loop: {error}"),
                    None => error.to_string(),
                })));
            }
        }) {
            Ok(task) => {
                self.task.replace(Some(task));
            }
            Err(error) => {
                // No dependency was changed: allow a later explicit retry.
                self.requested.set(false);
                ui.set_app_closing(false);
                ui.set_input_notice(error.to_string().into());
            }
        }
    }

    pub fn run(&self, ui: TerminalWindow) -> Result<(), Box<dyn std::error::Error>> {
        ui.show()?;
        self.window.replace(Some(ui));
        let result = slint::run_event_loop_until_quit();
        // Also retire the root when the backend unexpectedly exits/errors.
        let hidden = release_window(&self.window);
        result?;
        hidden?;
        Ok(())
    }

    pub fn finish(&self) -> Result<(), String> {
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        if self.requested.get() {
            self.outcome.borrow().clone().ok_or_else(|| {
                "event loop ended before asynchronous app exit completed".to_owned()
            })?
        } else {
            // Backend errors still take every synchronous cleanup path in main.
            Ok(())
        }
    }
}

fn release_window(window: &RefCell<Option<TerminalWindow>>) -> Result<(), String> {
    let ui = window.borrow_mut().take();
    if let Some(ui) = ui {
        let result = ui.hide().map_err(|error| error.to_string());
        drop(ui);
        result
    } else {
        Ok(())
    }
}
