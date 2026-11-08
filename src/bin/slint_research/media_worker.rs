//! One owned media thread/runtime. HTTP, metadata and pixels never block BBS/UI.
use pttstella::media::{
    Target,
    loader::Image,
    native_image::Decoder,
    preview::{Presentation, Preview},
};
use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
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
    Promote(u64),
    Motion(u64, bool),
}
struct Gate {
    request: AtomicU64,
    permit: AtomicBool,
    closed: AtomicBool,
    wake: Notify,
}
pub struct Worker {
    send: mpsc::Sender<Command>,
    gate: Arc<Gate>,
    join: Option<JoinHandle<Result<(), String>>>,
}
impl Worker {
    pub fn start() -> Result<(Self, watch::Receiver<State>), String> {
        let (send, receive) = mpsc::channel(8);
        let (state, updates) = watch::channel(State::default());
        let gate = Arc::new(Gate {
            request: AtomicU64::new(0),
            permit: AtomicBool::new(false),
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
                runtime.block_on(run(receive, state, control));
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        Ok((
            Self {
                send,
                gate,
                join: Some(join),
            },
            updates,
        ))
    }
    pub fn command(&self, command: Command) -> Result<(), String> {
        self.send
            .try_send(command)
            .map_err(|_| pttstella::msg!("image-prepare-failed"))
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

fn pixels(frame: pttstella::media::native_image::Frame) -> SharedPixelBuffer<Rgba8Pixel> {
    SharedPixelBuffer::clone_from_slice(
        frame.pixels.as_raw(),
        frame.pixels.width(),
        frame.pixels.height(),
    )
}
async fn run(mut commands: mpsc::Receiver<Command>, output: watch::Sender<State>, gate: Arc<Gate>) {
    let mut preview = Preview::default();
    let mut state = State::default();
    let mut decoder: Option<Decoder> = None;
    let mut due = None;
    let mut enabled = true;
    let mut finished = false;
    let mut loops = 0u32;
    let mut delay = Duration::from_millis(20);
    loop {
        if gate.closed.load(Ordering::Acquire) {
            break;
        }
        let current = gate.request.load(Ordering::Acquire);
        if state.request != 0 && state.request != current {
            preview.close();
            decoder = None;
            due = None;
            state = State::default();
            output.send_replace(state.clone());
        }
        let playing = enabled
            && state.inspect
            && !finished
            && gate.permit.load(Ordering::Acquire)
            && state
                .original
                .as_ref()
                .is_some_and(|image| image.animated_format);
        if playing != state.playing && !state.loading && state.error.is_none() {
            state.playing = playing;
            if playing {
                due = Some(tokio::time::Instant::now() + delay);
            } else {
                due = None;
                if !finished && let Some(source) = state.original.clone() {
                    loops = 0;
                    // Release compositing canvases before rebuilding the first
                    // frame. Never keep two full decoder instances resident.
                    decoder = None;
                    match Decoder::new(source).and_then(|mut next| {
                        let frame = next
                            .next_frame()?
                            .ok_or(pttstella::media::loader::Error::Format)?;
                        delay = frame.delay;
                        state.pixels = Some(pixels(frame));
                        state.frame = state.frame.saturating_add(1);
                        decoder = Some(next);
                        Ok(())
                    }) {
                        Ok(()) => {}
                        Err(e) => state.error = Some(e.to_string()),
                    }
                }
            }
            output.send_replace(state.clone());
        }
        tokio::select! {
            biased;
            _=gate.wake.notified()=>{},
            command=commands.recv()=>{
                let Some(command)=command else{break};
                match command{
                    Command::Open{request,generation,target,presentation} if gate.request.load(Ordering::Acquire)==request=>{
                        decoder=None;due=None;loops=0;enabled=true;finished=false;
                        state=State{request,loading:true,inspect:presentation==Presentation::Inspect,..Default::default()};
                        let opened=if state.inspect {preview.open(generation,target)} else {preview.hover(generation,target)};
                        match opened{Ok(_)=>{},Err(e)=>{state.loading=false;state.error=Some(e);}}
                        output.send_replace(state.clone());
                    }
                    Command::Promote(request) if request==state.request && !state.inspect=>{
                        if let Some(active)=preview.active().cloned() {
                            match preview.open(active.generation,active.target) {
                                Ok(_)=>{state.inspect=true;},Err(e)=>{state.error=Some(e);}
                            }
                            output.send_replace(state.clone());
                        }
                    }
                    Command::Motion(request,value) if request==state.request=>{
                        enabled=value;
                        if value && finished {
                            finished=false;loops=0;decoder=None;
                            match state.original.clone().ok_or(pttstella::media::loader::Error::Format).and_then(Decoder::new).and_then(|mut next| {
                                let frame=next.next_frame()?.ok_or(pttstella::media::loader::Error::Format)?;
                                delay=frame.delay;state.pixels=Some(pixels(frame));state.frame=state.frame.saturating_add(1);decoder=Some(next);Ok(())
                            }) {
                                Ok(())=>{},Err(e)=>state.error=Some(e.to_string()),
                            }
                            output.send_replace(state.clone());
                        }
                    }
                    _=>{}
                }
            }
            update=preview.update()=>{
                let Some(update)=update else{continue};
                if gate.request.load(Ordering::Acquire)!=state.request{continue;}
                state.loading=false;
                match update.result{
                    Ok((image,cached))=>{
                        let result=Decoder::new(image.clone()).and_then(|mut next|{
                            let frame=next.next_frame()?.ok_or(pttstella::media::loader::Error::Format)?;
                            delay=frame.delay;state.pixels=Some(pixels(frame));decoder=Some(next);Ok(())
                        });
                        if gate.request.load(Ordering::Acquire)!=state.request{continue;}
                        match result{
                            Ok(())=>{
                                preview.decoded(update.request,true);
                                enabled=image.mime=="image/gif";
                                state.original=Some(image);state.cached=cached;state.frame=1;state.playing=false;
                            }
                            Err(e)=>{preview.decoded(update.request,false);state.error=Some(e.to_string());}
                        }
                    }
                    Err(e)=>state.error=Some(e.to_string()),
                }
                output.send_replace(state.clone());
            }
            _=async{match due{Some(deadline)=>tokio::time::sleep_until(deadline).await,None=>std::future::pending().await}}=>{
                due=None;
                if gate.request.load(Ordering::Acquire)!=state.request || !gate.permit.load(Ordering::Acquire){continue;}
                if let Some(next)=&mut decoder{
                    let frame=match next.next_frame(){
                        Ok(Some(frame))=>Ok(Some(frame)),
                        Ok(None)=>{
                            loops=loops.saturating_add(1);
                            let repeat=match next.looping{image::metadata::LoopCount::Infinite=>true,image::metadata::LoopCount::Finite(n)=>loops<n.get()};
                            decoder=None;
                            if repeat{
                                match state.original.clone().ok_or(pttstella::media::loader::Error::Format).and_then(Decoder::new){
                                    Ok(mut reset)=>{let frame=reset.next_frame();decoder=Some(reset);frame},Err(e)=>Err(e)
                                }
                            }else{finished=true;Ok(None)}
                        }
                        Err(e)=>Err(e),
                    };
                    match frame{
                        Ok(Some(frame))=>{delay=frame.delay;state.pixels=Some(pixels(frame));state.frame=state.frame.saturating_add(1);due=Some(tokio::time::Instant::now()+delay);}
                        Ok(None)=>{},Err(e)=>{state.error=Some(e.to_string());enabled=false;}
                    }
                    if gate.request.load(Ordering::Acquire)==state.request{output.send_replace(state.clone());}
                }
            }
        }
    }
    preview.close();
    drop(decoder);
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
}
