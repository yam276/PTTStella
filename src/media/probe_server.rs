//! Owned single-file loopback service for portable native acceptance probes.
use serde::Serialize;
use std::{
    io::{Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    path::Path,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};
#[derive(Clone, Debug, Default, Serialize)]
pub struct Report {
    pub requests: usize,
    pub ranges: Vec<(usize, usize)>,
    pub bytes: usize,
    pub stalled: bool,
    pub joined: bool,
}
pub struct Server {
    address: SocketAddr,
    pub requests: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    active: Arc<Mutex<Option<TcpStream>>>,
    stall: Arc<(Mutex<bool>, Condvar)>,
    join: Option<JoinHandle<Result<Report, String>>>,
    extension: String,
}
fn range(headers: &str, length: usize) -> Result<(usize, usize, bool), String> {
    let Some(value) = headers.lines().find_map(|line| {
        line.split_once(':')
            .filter(|(name, _)| name.eq_ignore_ascii_case("range"))
            .map(|(_, v)| v.trim())
    }) else {
        return Ok((0, length - 1, false));
    };
    let (a, b) = value
        .strip_prefix("bytes=")
        .ok_or("unsupported byte range")?
        .split_once('-')
        .ok_or("invalid range")?;
    let start = a.parse::<usize>().map_err(|_| "invalid range start")?;
    let end = if b.is_empty() {
        length - 1
    } else {
        b.parse::<usize>()
            .map_err(|_| "invalid range end")?
            .min(length - 1)
    };
    if start > end || start >= length {
        return Err("range outside fixture".into());
    }
    Ok((start, end, true))
}
impl Server {
    pub fn start(path: &Path, stall_body: bool) -> Result<Self, String> {
        Self::with_cache(path, stall_body, false)
    }
    pub fn start_cached(path: &Path) -> Result<Self, String> {
        Self::with_cache(path, false, true)
    }
    fn with_cache(path: &Path, stall_body: bool, cached: bool) -> Result<Self, String> {
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .filter(|s| matches!(*s, "mp4" | "webm" | "ogv" | "png" | "gif" | "jpg" | "webp"))
            .ok_or("unsupported owned media fixture extension")?
            .to_owned();
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.is_empty() || bytes.len() > 16 * 1024 * 1024 {
            return Err("fixture payload outside 1..16MiB".into());
        }
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
        let address = listener.local_addr().map_err(|e| e.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(Mutex::new(None));
        let stall = Arc::new((Mutex::new(false), Condvar::new()));
        let (closed, count, socket, paused, kind) = (
            stop.clone(),
            requests.clone(),
            active.clone(),
            stall.clone(),
            extension.clone(),
        );
        let join=std::thread::Builder::new().name("native-player-fixture".into()).spawn(move||{
            let mut report=Report{stalled:stall_body,..Default::default()};
            while !closed.load(Ordering::Acquire){
                let(mut stream,_)=listener.accept().map_err(|e|e.to_string())?;if closed.load(Ordering::Acquire){break;}
                stream.set_read_timeout(Some(Duration::from_secs(2))).map_err(|e|e.to_string())?;
                stream.set_write_timeout(Some(Duration::from_secs(2))).map_err(|e|e.to_string())?;
                *socket.lock().map_err(|_|"fixture socket state poisoned")?=Some(stream.try_clone().map_err(|e|e.to_string())?);
                let mut header=Vec::new();
                while !header.ends_with(b"\r\n\r\n")&&header.len()<8192{let mut byte=[0];match stream.read_exact(&mut byte){Ok(())=>header.push(byte[0]),Err(_)if closed.load(Ordering::Acquire)=>break,Err(e)=>return Err(e.to_string())}}
                if closed.load(Ordering::Acquire){break;}
                let header=std::str::from_utf8(&header).map_err(|e|e.to_string())?;
                let valid=header.starts_with(&format!("GET /fixture.{kind} HTTP/"))||header.starts_with(&format!("HEAD /fixture.{kind} HTTP/"));
                if !valid||!header.ends_with("\r\n\r\n"){return Err("unexpected fixture request".into());}
                let request=count.fetch_add(1,Ordering::AcqRel)+1;if request>64{return Err("fixture request limit exceeded".into());}
                let(start,end,partial)=range(header,bytes.len())?;report.ranges.push((start,end));report.requests=request;
                let mime=match kind.as_str(){"mp4"=>"video/mp4","webm"=>"video/webm","ogv"=>"video/ogg","png"=>"image/png","gif"=>"image/gif","jpg"=>"image/jpeg",_=>"image/webp"};
                let mut response=format!("HTTP/1.1 {}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nCache-Control: {}\r\nConnection: close\r\n",if partial{"206 Partial Content"}else{"200 OK"},end-start+1,if cached{"max-age=60"}else{"no-store"});
                if partial{response.push_str(&format!("Content-Range: bytes {start}-{end}/{}\r\n",bytes.len()));}response.push_str("\r\n");
                stream.write_all(response.as_bytes()).map_err(|e|e.to_string())?;
                if stall_body{
                    let guard=paused.0.lock().map_err(|_|"fixture pause state poisoned")?;
                    let _guard=paused.1.wait_while(guard,|released|!*released).map_err(|_|"fixture pause state poisoned")?;
                    if closed.load(Ordering::Acquire){break;}
                }
                if !header.starts_with("HEAD "){
                    if let Err(e)=stream.write_all(&bytes[start..=end])
                        && !closed.load(Ordering::Acquire)
                        && e.kind()!=std::io::ErrorKind::BrokenPipe
                        && e.kind()!=std::io::ErrorKind::ConnectionReset {
                        return Err(e.to_string());
                    }
                    report.bytes+=end-start+1;
                }
                socket.lock().map_err(|_|"fixture socket state poisoned")?.take();
            }
            report.joined=true;Ok(report)
        }).map_err(|e|e.to_string())?;
        Ok(Self {
            address,
            requests,
            stop,
            active,
            stall,
            join: Some(join),
            extension,
        })
    }
    pub fn url(&self) -> String {
        format!("http://{}/fixture.{}", self.address, self.extension)
    }
    pub fn cancel(&self) -> Result<(), String> {
        if self.stop.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        *self
            .stall
            .0
            .lock()
            .map_err(|_| "fixture pause state poisoned")? = true;
        self.stall.1.notify_all();
        if let Some(socket) = self
            .active
            .lock()
            .map_err(|_| "fixture socket state poisoned")?
            .as_ref()
        {
            let _ = socket.shutdown(Shutdown::Both);
        }
        if self.join.as_ref().is_some_and(|join| !join.is_finished()) {
            match TcpStream::connect_timeout(&self.address, Duration::from_secs(1)) {
                Ok(_) => {}
                // The worker can exit before connect, or accept this wake and
                // close it while connect is finishing. These are all terminal
                // outcomes of our wake socket, not errors from a media request.
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::ConnectionRefused
                            | std::io::ErrorKind::ConnectionReset
                            | std::io::ErrorKind::ConnectionAborted
                    ) => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(())
    }
    pub fn finish(&mut self) -> Result<Report, String> {
        self.cancel()?;
        if let Some(join) = self.join.take() {
            join.join().map_err(|_| "fixture worker panicked")?
        } else {
            Ok(Report::default())
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if self.join.is_some() {
            let _ = self.finish();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::NamedTempFile, Server) {
        let mut file = tempfile::Builder::new().suffix(".mp4").tempfile().unwrap();
        file.write_all(b"owned fixture bytes").unwrap();
        let server = Server::start(file.path(), false).unwrap();
        (file, server)
    }
    #[test]
    fn repeated_cancel_still_joins_and_keeps_the_report() {
        for _ in 0..64 {
            let (_file, mut server) = fixture();
            server.cancel().unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while !server.join.as_ref().unwrap().is_finished() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            let report = server.finish().unwrap();
            assert!(report.joined);
            assert_eq!(report.requests, 0);
            assert!(server.join.is_none());
        }
    }
    #[test]
    fn stalled_response_is_cancelled_and_joined_without_body_bytes() {
        let (file, initial) = fixture();
        drop(initial);
        let mut server = Server::start(file.path(), true).unwrap();
        let mut client = TcpStream::connect(server.address).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        client
            .write_all(b"GET /fixture.mp4 HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = Vec::new();
        while !response.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            client.read_exact(&mut byte).unwrap();
            response.push(byte[0]);
            assert!(response.len() < 1024);
        }
        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        server.cancel().unwrap();
        let report = server.finish().unwrap();
        assert!(report.joined && report.stalled);
        assert_eq!((report.requests, report.bytes), (1, 0));
        let mut body = Vec::new();
        client.read_to_end(&mut body).unwrap();
        assert!(body.is_empty());
    }
    #[test]
    fn failed_worker_is_joined_and_reports_its_original_error() {
        let (_file, mut server) = fixture();
        let mut client = TcpStream::connect(server.address).unwrap();
        client.write_all(b"POST /wrong HTTP/1.1\r\n\r\n").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while !server.join.as_ref().unwrap().is_finished() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(server.finish().unwrap_err(), "unexpected fixture request");
        assert!(server.join.is_none());
    }
    #[test]
    fn byte_ranges_are_exact_and_bounded() {
        assert_eq!(
            range("GET / HTTP/1.1\r\n\r\n", 100).unwrap(),
            (0, 99, false)
        );
        assert_eq!(range("Range: bytes=5-9", 100).unwrap(), (5, 9, true));
        assert_eq!(range("range: bytes=50-", 100).unwrap(), (50, 99, true));
        for value in [
            "Range: bytes=100-",
            "Range: bytes=7-2",
            "Range: bytes=-8",
            "Range: bytes=0-1,3-4",
        ] {
            assert!(range(value, 100).is_err());
        }
    }
}
