//! One owned media thread/runtime. HTTP, metadata and pixels never block BBS/UI.
use crate::{
    inline_engine::{self, Controls as InlineControls, Engine as InlineEngine},
    media_playback::Playback,
};
use pttstella::media::{
    Target,
    loader::Image,
    preview::{Presentation, Preview},
};
use slint::{Rgba8Pixel, SharedPixelBuffer};
#[cfg(test)]
use std::time::Duration;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::JoinHandle,
};
use tokio::sync::{Notify, mpsc, watch};

#[derive(Clone, Default)]
pub struct State {
    pub request: u64,
    pub pixels: Option<SharedPixelBuffer<Rgba8Pixel>>,
    pub original: Option<Arc<Image>>,
    pub loading: bool,
    pub cached: bool,
    pub playing: bool,
    pub frame: u64,
    pub error: Option<String>,
    pub inspect: bool,
}
pub enum Command {
    Open {
        request: u64,
        generation: u64,
        target: Target,
        presentation: Presentation,
    },
    Inline {
        epoch: u64,
        projection: pttstella::media::inline::Projection<'static>,
        target: Option<pttstella::media::inline::Target>,
    },
    OpenOriginal {
        request: u64,
        image: Arc<Image>,
    },
    Promote(u64),
    Motion(u64, bool),
}
struct Gate {
    request: AtomicU64,
    permit: AtomicBool,
    inline_epoch: AtomicU64,
    inline_automatic: AtomicBool,
    closed: AtomicBool,
    wake: Notify,
}
pub struct Worker {
    send: mpsc::Sender<Command>,
    gate: Arc<Gate>,
    join: Option<JoinHandle<Result<(), String>>>,
    inline_controls: watch::Sender<InlineControls>,
    inline_updates: watch::Receiver<inline_engine::State>,
}
impl Worker {
    pub fn start() -> Result<(Self, watch::Receiver<State>), String> {
        let (send, receive) = mpsc::channel(8);
        let (state, updates) = watch::channel(State::default());
        let (inline_controls, controls) = watch::channel(InlineControls::default());
        let (inline_output, inline_updates) = watch::channel(inline_engine::State::default());
        let gate = Arc::new(Gate {
            request: AtomicU64::new(0),
            permit: AtomicBool::new(false),
            inline_epoch: AtomicU64::new(0),
            inline_automatic: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            wake: Notify::new(),
        });
        let control = gate.clone();
        let join = std::thread::Builder::new()
            .name("slint-native-media".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(run(receive, state, control, controls, inline_output));
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        Ok((
            Self {
                send,
                gate,
                join: Some(join),
                inline_controls,
                inline_updates,
            },
            updates,
        ))
    }
    pub fn command(&self, command: Command) -> Result<(), String> {
        self.send
            .try_send(command)
            .map_err(|_| pttstella::msg!("image-prepare-failed"))
    }
    pub fn inline_automatic(&self, enabled: bool) {
        self.gate.inline_automatic.store(enabled, Ordering::Release);
        self.gate.wake.notify_one();
    }
    pub fn inline_updates(&self) -> watch::Receiver<inline_engine::State> {
        self.inline_updates.clone()
    }
    pub fn inline_scope(&self, epoch: u64) {
        self.gate.inline_epoch.store(epoch, Ordering::Release);
        self.gate.wake.notify_one();
    }
    pub fn inline_controls(&self, controls: InlineControls) {
        self.inline_controls.send_replace(controls);
    }
    pub fn scope(&self, request: u64, permit: bool) {
        self.gate.request.store(request, Ordering::Release);
        self.gate.permit.store(permit, Ordering::Release);
        self.gate.wake.notify_one();
    }
    pub fn finish(&mut self) -> Result<(), String> {
        self.gate.closed.store(true, Ordering::Release);
        self.gate.wake.notify_one();
        if let Some(join) = self.join.take() {
            join.join().map_err(|_| "native media worker panicked")??;
        }
        Ok(())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

fn playback_state(state: &mut State, playback: &Playback) {
    state.pixels = Some(playback.pixels());
    state.frame = playback.frame();
    state.playing = playback.playing();
}
async fn run(
    mut commands: mpsc::Receiver<Command>,
    output: watch::Sender<State>,
    gate: Arc<Gate>,
    mut inline_controls: watch::Receiver<InlineControls>,
    inline_output: watch::Sender<inline_engine::State>,
) {
    let mut preview = Preview::default();
    let mut inline: Option<InlineEngine> = None;
    let mut state = State::default();
    let mut playback: Option<Playback> = None;
    loop {
        if gate.closed.load(Ordering::Acquire) {
            break;
        }
        let current = gate.request.load(Ordering::Acquire);
        if state.request != 0 && state.request != current {
            preview.close();
            playback = None;
            state = State::default();
            output.send_replace(state.clone());
        }
        let epoch = gate.inline_epoch.load(Ordering::Acquire);
        let controls = *inline_controls.borrow_and_update();
        if let Some(engine) = &mut inline {
            if engine.state.epoch != epoch {
                engine.reset(epoch);
                inline_output.send_replace(engine.state.clone());
            }
            if engine.automatic(gate.inline_automatic.load(Ordering::Acquire)) {
                inline_output.send_replace(engine.state.clone());
            }
            if engine.reconcile(controls, state.inspect) {
                inline_output.send_replace(engine.state.clone());
            }
        }
        let inline_due = inline.as_ref().and_then(InlineEngine::deadline);
        if !state.loading
            && state.error.is_none()
            && let Some(player) = &mut playback
        {
            match player.permit(state.inspect && gate.permit.load(Ordering::Acquire)) {
                Ok(true) => {
                    playback_state(&mut state, player);
                    output.send_replace(state.clone());
                }
                Ok(false) => {}
                Err(e) => {
                    state.error = Some(e.to_string());
                    output.send_replace(state.clone());
                }
            }
        }
        let due = playback.as_ref().and_then(Playback::deadline);
        tokio::select! {
            biased;
            _=gate.wake.notified()=>{},
            changed=inline_controls.changed()=>{if changed.is_err(){break;}},
            command=commands.recv()=>{
                let Some(command)=command else { break; };
                match command {
                    Command::Inline{epoch,projection,target} if epoch!=0 && gate.inline_epoch.load(Ordering::Acquire)==epoch=>{
                        let engine=inline.get_or_insert_with(||InlineEngine::new(epoch));
                        if engine.state.epoch!=epoch {engine.reset(epoch);}
                        engine.project(projection,target,gate.inline_automatic.load(Ordering::Acquire));inline_output.send_replace(engine.state.clone());
                    }
                    Command::OpenOriginal{request,image} if gate.request.load(Ordering::Acquire)==request=>{
                        if let Some(engine)=&mut inline && engine.reconcile(*inline_controls.borrow(),true){inline_output.send_replace(engine.state.clone());}
                        preview.close();playback=None;
                        state=State{request,inspect:true,..Default::default()};
                        match Playback::new(image.clone()) {
                            Ok(player)=>{playback_state(&mut state,&player);playback=Some(player);state.original=Some(image);}
                            Err(e)=>state.error=Some(e.to_string()),
                        }
                        if gate.request.load(Ordering::Acquire)==request {output.send_replace(state.clone());}
                    }
                    Command::Open{request,generation,target,presentation} if gate.request.load(Ordering::Acquire)==request=>{
                        playback=None;
                        state=State{request,loading:true,inspect:presentation==Presentation::Inspect,..Default::default()};
                        let opened=if state.inspect {preview.open(generation,target)} else {preview.hover(generation,target)};
                        if let Err(e)=opened {state.loading=false;state.error=Some(e);}
                        output.send_replace(state.clone());
                    }
                    Command::Promote(request) if request==state.request && !state.inspect=>{
                        if let Some(active)=preview.active().cloned() {
                            match preview.open(active.generation,active.target) {
                                Ok(_)=>state.inspect=true, Err(e)=>state.error=Some(e),
                            }
                            output.send_replace(state.clone());
                        }
                    }
                    Command::Motion(request,value) if request==state.request=>{
                        if let Some(player)=&mut playback {
                            match player.motion(value) {
                                Ok(true)=>{playback_state(&mut state,player);output.send_replace(state.clone());}
                                Ok(false)=>{},Err(e)=>{state.error=Some(e.to_string());output.send_replace(state.clone());}
                            }
                        }
                    }
                    _=>{},
                }
            }
            update=preview.update()=>{
                let Some(update)=update else { continue; };
                if gate.request.load(Ordering::Acquire)!=state.request { continue; }
                state.loading=false;
                match update.result {
                    Ok((image,cached))=>{
                        let decoded=Playback::new(image.clone());
                        if gate.request.load(Ordering::Acquire)!=state.request {continue;}
                        match decoded {
                            Ok(player)=>{
                                preview.decoded(update.request,true);
                                playback_state(&mut state,&player);
                                playback=Some(player);
                                state.original=Some(image);state.cached=cached;
                            }
                            Err(e)=>{preview.decoded(update.request,false);state.error=Some(e.to_string());}
                        }
                    }
                    Err(e)=>state.error=Some(e.to_string()),
                }
                output.send_replace(state.clone());
            }
            update=async {match inline.as_mut() {Some(engine)=>engine.batch.update().await,None=>std::future::pending().await}}=>{
                if let Some(engine)=&mut inline && engine.state.epoch==gate.inline_epoch.load(Ordering::Acquire) {
                    engine.loaded(update);
                    engine.automatic(gate.inline_automatic.load(Ordering::Acquire));
                    if engine.state.epoch==gate.inline_epoch.load(Ordering::Acquire) {inline_output.send_replace(engine.state.clone());}
                }
            }
            _=async {match inline_due {Some(at)=>tokio::time::sleep_until(at).await,None=>std::future::pending().await}}=>{
                if let Some(engine)=&mut inline && engine.state.epoch==gate.inline_epoch.load(Ordering::Acquire) {
                    engine.advance();inline_output.send_replace(engine.state.clone());
                }
            }
            _=async { match due {Some(at)=>tokio::time::sleep_until(at).await,None=>std::future::pending().await} }=>{
                if gate.request.load(Ordering::Acquire)!=state.request || !gate.permit.load(Ordering::Acquire){continue;}
                if let Some(player)=&mut playback {
                    match player.advance() {
                        Ok(_)=>playback_state(&mut state,player),
                        Err(e)=>{state.error=Some(e.to_string());player.stop();playback_state(&mut state,player);}
                    }
                    if gate.request.load(Ordering::Acquire)==state.request {output.send_replace(state.clone());}
                }
            }
        }
    }
    preview.close();
    drop(inline);
    inline_output.send_replace(inline_engine::State::default());
    drop(playback);
    output.send_replace(State::default());
}

#[cfg(test)]
mod tests {
    use super::*;
    use pttstella::media::{classify, probe_server::Server};
    use tokio::time::{sleep, timeout};
    fn file(gif: bool) -> tempfile::NamedTempFile {
        let file = tempfile::Builder::new()
            .suffix(if gif { ".gif" } else { ".png" })
            .tempfile()
            .unwrap();
        std::fs::write(
            file.path(),
            if gif {
                include_bytes!("../../../tests/fixtures/media/motion.gif").as_slice()
            } else {
                include_bytes!("../../../tests/fixtures/media/colours.png").as_slice()
            },
        )
        .unwrap();
        file
    }
    async fn state(rx: &mut watch::Receiver<State>, test: impl Fn(&State) -> bool) -> State {
        timeout(Duration::from_secs(5), rx.wait_for(test))
            .await
            .unwrap()
            .unwrap()
            .clone()
    }
    async fn requested(server: &Server) {
        timeout(Duration::from_secs(5), async {
            while server.requests.load(Ordering::Acquire) == 0 {
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }
    fn open(worker: &Worker, server: &Server, request: u64) {
        worker.scope(request, true);
        worker
            .command(Command::Open {
                request,
                generation: 1,
                target: classify(&server.url()).unwrap().unwrap(),
                presentation: Presentation::Inspect,
            })
            .unwrap();
    }
    #[tokio::test]
    async fn successfully_decoded_retry_reuses_the_original_without_http() {
        let file = file(false);
        let mut server = Server::start_cached(file.path()).unwrap();
        let (mut worker, mut rx) = Worker::start().unwrap();
        open(&worker, &server, 1);
        let first = state(&mut rx, |s| s.request == 1 && s.original.is_some()).await;
        assert_eq!(
            &first.pixels.as_ref().unwrap().as_bytes()[..4],
            &[255, 0, 0, 255]
        );
        open(&worker, &server, 2);
        let cached = state(&mut rx, |s| s.request == 2 && s.cached).await;
        assert!(Arc::ptr_eq(
            first.original.as_ref().unwrap(),
            cached.original.as_ref().unwrap()
        ));
        assert_eq!(server.requests.load(Ordering::Acquire), 1);
        worker.finish().unwrap();
        assert!(server.finish().unwrap().joined);
    }
    #[tokio::test]
    async fn decoded_hover_is_static_and_promotes_without_refetch_or_encoded_copy() {
        let file = file(true);
        let mut server = Server::start(file.path(), false).unwrap();
        let (mut worker, mut rx) = Worker::start().unwrap();
        worker.scope(1, false);
        worker
            .command(Command::Open {
                request: 1,
                generation: 1,
                target: classify(&server.url()).unwrap().unwrap(),
                presentation: Presentation::Hover,
            })
            .unwrap();
        let first = state(&mut rx, |s| s.original.is_some()).await;
        assert!(!first.inspect && !first.playing);
        assert_eq!(
            &first.pixels.as_ref().unwrap().as_bytes()[..4],
            &[255, 0, 0, 255]
        );
        sleep(Duration::from_millis(650)).await;
        assert_eq!(rx.borrow().frame, first.frame);
        worker.scope(1, true);
        worker.command(Command::Promote(1)).unwrap();
        let playing = state(&mut rx, |s| s.inspect && s.playing && s.frame > first.frame).await;
        assert!(Arc::ptr_eq(
            first.original.as_ref().unwrap(),
            playing.original.as_ref().unwrap()
        ));
        assert_eq!(server.requests.load(Ordering::Acquire), 1);
        worker.finish().unwrap();
        assert!(server.finish().unwrap().joined);
    }
    #[tokio::test]
    async fn in_flight_hover_promotes_the_same_future_before_response_body_arrives() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let url = format!("http://{}/owned.png", listener.local_addr().unwrap());
        let (seen, ready) = tokio::sync::oneshot::channel();
        let (release, released) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                assert!(head.len() < 8192);
                head.push(socket.read_u8().await.unwrap());
            }
            let bytes = include_bytes!("../../../tests/fixtures/media/colours.png");
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",bytes.len()).as_bytes()).await.unwrap();
            seen.send(()).unwrap();
            released.await.unwrap();
            socket.write_all(bytes).await.unwrap();
            assert!(
                timeout(Duration::from_millis(200), listener.accept())
                    .await
                    .is_err(),
                "promotion sent a second HTTP request"
            );
        });
        let (mut worker, mut rx) = Worker::start().unwrap();
        worker.scope(1, false);
        worker
            .command(Command::Open {
                request: 1,
                generation: 1,
                target: classify(&url).unwrap().unwrap(),
                presentation: Presentation::Hover,
            })
            .unwrap();
        timeout(Duration::from_secs(5), ready)
            .await
            .unwrap()
            .unwrap();
        worker.scope(1, true);
        worker.command(Command::Promote(1)).unwrap();
        state(&mut rx, |s| s.inspect && s.loading).await;
        release.send(()).unwrap();
        let ready = state(&mut rx, |s| s.inspect && s.original.is_some()).await;
        assert_eq!(&ready.pixels.unwrap().as_bytes()[..4], &[255, 0, 0, 255]);
        worker.finish().unwrap();
        timeout(Duration::from_secs(5), peer)
            .await
            .unwrap()
            .unwrap();
    }
    #[tokio::test]
    async fn animated_frames_pause_at_first_and_resume_without_refetch() {
        let file = file(true);
        let mut server = Server::start(file.path(), false).unwrap();
        let (mut worker, mut rx) = Worker::start().unwrap();
        open(&worker, &server, 1);
        state(&mut rx, |s| {
            s.playing && s.pixels.as_ref().is_some_and(|p| p.as_bytes()[2] == 255)
        })
        .await;
        worker.scope(1, false);
        let paused = state(&mut rx, |s| {
            s.original.is_some()
                && !s.playing
                && s.pixels.as_ref().is_some_and(|p| p.as_bytes()[0] == 255)
        })
        .await;
        sleep(Duration::from_millis(650)).await;
        assert_eq!(rx.borrow().frame, paused.frame);
        worker.scope(1, true);
        state(&mut rx, |s| s.playing && s.frame > paused.frame).await;
        worker.command(Command::Motion(1, false)).unwrap();
        state(&mut rx, |s| !s.playing).await;
        assert_eq!(server.requests.load(Ordering::Acquire), 1);
        worker.finish().unwrap();
        assert!(server.finish().unwrap().joined);
    }
    #[tokio::test]
    async fn finite_animations_retain_last_frame_and_replay_only_after_explicit_play() {
        for (fixture, frames) in [
            (
                include_bytes!("../../../tests/fixtures/media/once.gif").as_slice(),
                2,
            ),
            (
                include_bytes!("../../../tests/fixtures/media/twice.gif").as_slice(),
                4,
            ),
        ] {
            let file = file(true);
            std::fs::write(file.path(), fixture).unwrap();
            let mut server = Server::start(file.path(), false).unwrap();
            let (mut worker, mut rx) = Worker::start().unwrap();
            open(&worker, &server, 1);
            let end = state(&mut rx, |s| s.frame == frames && !s.playing).await;
            assert_eq!(&end.pixels.unwrap().as_bytes()[..4], &[0, 0, 255, 255]);
            worker.scope(1, false);
            sleep(Duration::from_millis(250)).await;
            worker.scope(1, true);
            sleep(Duration::from_millis(250)).await;
            assert_eq!(rx.borrow().frame, frames);
            assert!(!rx.borrow().playing);
            worker.command(Command::Motion(1, true)).unwrap();
            let end = state(&mut rx, |s| s.frame == frames * 2 && !s.playing).await;
            assert_eq!(&end.pixels.unwrap().as_bytes()[..4], &[0, 0, 255, 255]);
            assert_eq!(server.requests.load(Ordering::Acquire), 1);
            worker.finish().unwrap();
            assert!(server.finish().unwrap().joined);
        }
    }
    #[tokio::test]
    async fn replacing_a_stalled_source_and_closing_joins_the_actual_worker() {
        let file = file(false);
        let mut slow = Server::start(file.path(), true).unwrap();
        let mut ready = Server::start(file.path(), false).unwrap();
        let (mut worker, mut rx) = Worker::start().unwrap();
        open(&worker, &slow, 1);
        requested(&slow).await;
        open(&worker, &ready, 2);
        state(&mut rx, |s| s.request == 2 && s.pixels.is_some()).await;
        worker.scope(0, false);
        state(&mut rx, |s| s.request == 0 && s.pixels.is_none()).await;
        worker.finish().unwrap();
        let slow = slow.finish().unwrap();
        let ready = ready.finish().unwrap();
        assert!(slow.joined && ready.joined);
        assert_eq!((slow.requests, slow.bytes), (1, 0));
        assert_eq!(ready.requests, 1);
    }
    fn inline_projection(urls: &[String]) -> pttstella::media::inline::Projection<'static> {
        let frame = pttstella::screen::Screen::new(80, 24).unwrap().frame();
        let mut paint = pttstella::presentation::sample_packet(&frame, &Default::default());
        paint.connected = true;
        paint.article = Some(pttstella::article_cache::Reading {
            id: 1,
            first_line: 1,
            last_line: 23,
            end: true,
            load: None,
            next_segment: false,
            reply_count: 0,
            percent_cells: None,
            page_cells: None,
            range_cells: None,
            pages: Vec::new(),
        });
        paint.navigation = pttstella::navigation::Context::ArticleReading {
            first_line: 1,
            last_line: 23,
            percent: 100,
        };
        paint.links = urls
            .iter()
            .enumerate()
            .map(|(i, url)| pttstella::links::Link {
                row: i * 2 + 1,
                start: 0,
                end: 40,
                url: url.clone(),
            })
            .collect();
        pttstella::media::inline::Projection::of(&paint).into_owned()
    }
    fn expand(worker: &Worker, epoch: u64, urls: &[String]) {
        worker.inline_scope(epoch);
        worker
            .command(Command::Inline {
                epoch,
                projection: inline_projection(urls),
                target: Some(pttstella::media::inline::Target::All),
            })
            .unwrap();
    }
    async fn inline_state(
        rx: &mut watch::Receiver<inline_engine::State>,
        test: impl Fn(&inline_engine::State) -> bool,
    ) -> inline_engine::State {
        timeout(Duration::from_secs(5), rx.wait_for(test))
            .await
            .unwrap()
            .unwrap()
            .clone()
    }
    #[tokio::test]
    async fn automatic_opt_out_cancels_stall_and_queue_without_projection_but_keeps_manual_batch() {
        let file = file(false);
        let mut slow = Server::start(file.path(), true).unwrap();
        let mut ready = Server::start_cached(file.path()).unwrap();
        let (mut worker, _) = Worker::start().unwrap();
        let mut updates = worker.inline_updates();
        worker.inline_scope(1);
        worker.inline_automatic(true);
        worker
            .command(Command::Inline {
                epoch: 1,
                projection: inline_projection(&[slow.url(), ready.url()]),
                target: None,
            })
            .unwrap();
        requested(&slow).await;
        inline_state(&mut updates, |s| s.automatic && s.entries.len() == 2).await;
        worker.inline_automatic(false);
        inline_state(&mut updates, |s| {
            s.epoch == 1 && !s.automatic && s.entries.is_empty()
        })
        .await;
        // A queued old follow cannot turn the newly disabled policy back on.
        worker
            .command(Command::Inline {
                epoch: 1,
                projection: inline_projection(&[slow.url(), ready.url()]),
                target: None,
            })
            .unwrap();
        sleep(Duration::from_millis(80)).await;
        assert_eq!(ready.requests.load(Ordering::Acquire), 0);
        assert!(updates.borrow().resources.is_empty());
        expand(&worker, 1, &[ready.url()]);
        let manual = inline_state(&mut updates, |s| !s.automatic && s.resources.len() == 1).await;
        worker.inline_automatic(true);
        worker
            .command(Command::Inline {
                epoch: 1,
                projection: inline_projection(&[ready.url()]),
                target: None,
            })
            .unwrap();
        sleep(Duration::from_millis(40)).await;
        worker.inline_automatic(false);
        sleep(Duration::from_millis(40)).await;
        let current = updates.borrow().clone();
        assert!(!current.automatic);
        assert!(Arc::ptr_eq(
            manual.resources[&0].original.as_ref().unwrap(),
            current.resources[&0].original.as_ref().unwrap()
        ));
        worker.finish().unwrap();
        let slow = slow.finish().unwrap();
        let ready = ready.finish().unwrap();
        assert_eq!((slow.requests, slow.bytes), (1, 0));
        assert_eq!(ready.requests, 1);
        assert!(slow.joined && ready.joined);
    }
    #[tokio::test]
    async fn inline_original_promotes_the_same_arc_without_http_and_both_owners_join() {
        let file = file(false);
        let mut server = Server::start(file.path(), false).unwrap();
        let (mut worker, mut preview) = Worker::start().unwrap();
        let mut images = worker.inline_updates();
        expand(&worker, 1, &[server.url()]);
        let loaded = inline_state(&mut images, |s| {
            s.resources.get(&0).is_some_and(|r| r.original.is_some())
        })
        .await;
        let original = loaded.resources[&0].original.clone().unwrap();
        worker.scope(1, true);
        worker
            .command(Command::OpenOriginal {
                request: 1,
                image: original.clone(),
            })
            .unwrap();
        let opened = state(&mut preview, |s| s.original.is_some()).await;
        assert!(Arc::ptr_eq(&original, opened.original.as_ref().unwrap()));
        assert_eq!(server.requests.load(Ordering::Acquire), 1);
        worker.inline_scope(0);
        inline_state(&mut images, |s| s.epoch == 0 && s.resources.is_empty()).await;
        worker.finish().unwrap();
        assert!(server.finish().unwrap().joined);
    }
    #[tokio::test]
    async fn inline_visible_set_is_bounded_to_two_and_modal_suspends_both_on_same_thread() {
        let file = file(true);
        let mut servers = (0..3)
            .map(|_| Server::start(file.path(), false).unwrap())
            .collect::<Vec<_>>();
        let (mut worker, mut preview) = Worker::start().unwrap();
        let mut images = worker.inline_updates();
        expand(
            &worker,
            1,
            &servers.iter().map(Server::url).collect::<Vec<_>>(),
        );
        worker.inline_controls(InlineControls {
            epoch: 1,
            permit: true,
            visible: [Some(0), Some(1)],
        });
        let loaded = inline_state(&mut images, |s| {
            s.resources.len() == 3 && s.resources.values().filter(|r| r.playing).count() == 2
        })
        .await;
        assert!(
            loaded.resources[&0].playing
                && loaded.resources[&1].playing
                && !loaded.resources[&2].playing
        );
        worker.scope(1, true);
        worker
            .command(Command::OpenOriginal {
                request: 1,
                image: loaded.resources[&2].original.clone().unwrap(),
            })
            .unwrap();
        state(&mut preview, |s| s.inspect && s.playing).await;
        inline_state(&mut images, |s| {
            s.resources.len() == 3 && s.resources.values().all(|r| !r.playing)
        })
        .await;
        worker.scope(0, false);
        worker.inline_controls(InlineControls {
            epoch: 1,
            permit: true,
            visible: [Some(1), Some(2)],
        });
        let restored = inline_state(&mut images, |s| {
            s.resources.get(&2).is_some_and(|r| r.playing)
        })
        .await;
        assert!(!restored.resources[&0].playing && restored.resources[&1].playing);
        worker.finish().unwrap();
        for server in &mut servers {
            let report = server.finish().unwrap();
            assert!(report.joined);
            assert_eq!(report.requests, 1);
        }
    }
    #[tokio::test]
    async fn inline_epoch_replacement_cancels_stalled_download_without_resurrecting_old_batch() {
        let file = file(false);
        let mut slow = Server::start(file.path(), true).unwrap();
        let mut ready = Server::start(file.path(), false).unwrap();
        let (mut worker, _) = Worker::start().unwrap();
        let mut images = worker.inline_updates();
        expand(&worker, 1, &[slow.url()]);
        requested(&slow).await;
        expand(&worker, 2, &[ready.url()]);
        inline_state(&mut images, |s| {
            s.epoch == 2 && s.resources.get(&0).is_some_and(|r| r.original.is_some())
        })
        .await;
        worker.finish().unwrap();
        let slow = slow.finish().unwrap();
        let ready = ready.finish().unwrap();
        assert!(slow.joined && ready.joined);
        assert_eq!((slow.requests, slow.bytes), (1, 0));
        assert_eq!(ready.requests, 1);
    }
}
