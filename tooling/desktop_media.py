"""Real WebView media interactions against our bounded loopback HTTP fixture server."""
import base64
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import threading
import time
import subprocess

FIXTURES = Path(__file__).resolve().parent.parent / 'tests/fixtures/media'

class FixtureServer:
    def __init__(self):
        self.counts = {}
        self.lock = threading.Lock()
        self.cancelled = threading.Event()
        self.video = b''
        self.jpeg = b''
        self.webp = b''
        owner = self
        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass
            def do_GET(self):
                path = self.path.split('?', 1)[0]
                with owner.lock:
                    owner.counts[path] = owner.counts.get(path, 0) + 1
                    count = owner.counts[path]
                if path in ('/slow.png','/hover-slow.png'):
                    self.send_response(200)
                    self.send_header('Content-Length', '100000')
                    self.end_headers()
                    self.wfile.flush()
                    self.connection.settimeout(5)
                    try:
                        if self.connection.recv(1) == b'':
                            owner.cancelled.set()
                    except (OSError, TimeoutError):
                        pass
                    return
                body, mime = {
                    '/colours.png': ((FIXTURES / 'colours.png').read_bytes(), 'image/png'),
                    '/hover.png': ((FIXTURES / 'colours.png').read_bytes(), 'image/png'),
                    '/motion.gif': ((FIXTURES / 'motion.gif').read_bytes(), 'image/gif'),
                    '/movie.webm': (owner.video, 'video/webm'),
                    '/colours.jpg': (owner.jpeg, 'image/jpeg'),
                    '/colours.webp': (owner.webp, 'image/webp'),
                    # Valid dimensions/container boundaries, but no IDAT: only the real decoder can reject it.
                    '/broken.png': ((FIXTURES / 'colours.png').read_bytes()[:33] + (FIXTURES / 'colours.png').read_bytes()[-12:] if count == 1 else
                                    (FIXTURES / 'colours.png').read_bytes(), 'image/png'),
                }.get(path, (b'not found', 'text/plain'))
                start, end = 0, len(body) - 1
                byte_range = self.headers.get('Range')
                if byte_range and byte_range.startswith('bytes='):
                    first, _, last = byte_range[6:].partition('-')
                    start = int(first or '0')
                    end = min(end, int(last) if last else end)
                partial = byte_range and 0 <= start <= end < len(body)
                self.send_response(206 if partial else 200)
                self.send_header('Content-Type', mime)
                self.send_header('Accept-Ranges', 'bytes')
                self.send_header('Access-Control-Allow-Origin', '*')
                self.send_header('Cache-Control', 'max-age=60')
                if partial:
                    self.send_header('Content-Range', f'bytes {start}-{end}/{len(body)}')
                    body = body[start:end + 1]
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                try:
                    self.wfile.write(body)
                except (OSError, BrokenPipeError):
                    pass
        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.server.daemon_threads = False
        self.thread = threading.Thread(target=self.server.serve_forever)
        self.origin = f'http://127.0.0.1:{self.server.server_port}'
    def __enter__(self):
        self.thread.start()
        return self
    def __exit__(self, *_args):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=6)
        if self.thread.is_alive():
            raise TimeoutError('media fixture server did not stop')

def run(d, output, server):
    # Generate our own codecs fixtures through the same actual WebView host.
    expression = '''(async()=>{
        const c=document.createElement('canvas');c.width=64;c.height=32;
        const x=c.getContext('2d');x.fillStyle='#ff0000';x.fillRect(0,0,32,32);x.fillStyle='#0000ff';x.fillRect(32,0,32,32);
        const jpeg=c.toDataURL('image/jpeg'),webp=c.toDataURL('image/webp');
        const stream=c.captureStream(10),parts=[];
        const recorder=new MediaRecorder(stream,{mimeType:'video/webm;codecs=vp8'});
        recorder.ondataavailable=e=>parts.push(e.data);
        const done=new Promise(resolve=>recorder.onstop=resolve);
        recorder.start();let blue=false;
        const timer=setInterval(()=>{blue=!blue;x.fillStyle=blue?'#0000ff':'#ff0000';x.fillRect(0,0,64,32);},100);
        await new Promise(resolve=>setTimeout(resolve,1800));recorder.stop();await done;
        clearInterval(timer);stream.getTracks().forEach(t=>t.stop());
        const video=await new Promise(resolve=>{const r=new FileReader();r.onload=()=>resolve(r.result);r.readAsDataURL(new Blob(parts,{type:'video/webm'}));});
        return {jpeg,webp,video};
    })()'''
    response = d.call('Runtime.evaluate', expression=expression, awaitPromise=True, returnByValue=True)
    if 'exceptionDetails' in response:
        raise ValueError('WebView could not generate media codec fixtures')
    values = response['result']['value']
    for name, mime in [('jpeg', 'image/jpeg'), ('webp', 'image/webp'), ('video', 'video/webm')]:
        if not values[name].startswith('data:' + mime):
            raise ValueError('required codec fixture unavailable: ' + name)
    server.jpeg, server.webp, server.video = [base64.b64decode(values[name].split(',', 1)[1], validate=True)
                                             for name in ('jpeg', 'webp', 'video')]
    (output / 'synthetic-video.webm').write_bytes(server.video)

    def cell(row, external=False):
        point = d.evaluate('(()=>{const b=document.getElementById("screen").getBoundingClientRect();return {x:b.left+b.width*8/80,y:b.top+b.height*'+str(row-.5)+'/24};})()')
        for kind in ('mousePressed', 'mouseReleased'):
            d.call('Input.dispatchMouseEvent', type=kind, **point, button='left', clickCount=1)
        if external:
            d.wait('document.getElementById("surface").dataset.linkResult === "requested" && !document.getElementById("media-dialog").open', 'external link did not reach the native sink')
        else:
            d.wait('document.getElementById("media-dialog").open', 'media link did not open its dialog')
    def state(value):
        d.wait('document.getElementById("media-dialog").dataset.state === '+repr(value), 'media state did not become ' + value)
    def close(escape=False):
        if escape:
            for kind in ('keyDown', 'keyUp'):
                d.call('Input.dispatchKeyEvent', type=kind, key='Escape', code='Escape', windowsVirtualKeyCode=27)
        else:
            d.click('#media-close')
        d.wait('!document.getElementById("media-dialog").open && document.activeElement.id === "keyboard"', 'media close did not restore input')
        if d.evaluate('document.getElementById("media-viewport").childElementCount') != 0:
            raise ValueError('media nodes retained after close')
    def pixels(selector):
        return d.evaluate('(()=>{const n=document.querySelector('+repr(selector)+'),c=document.createElement("canvas");c.width=64;c.height=32;const x=c.getContext("2d");x.drawImage(n,0,0,64,32);return [...x.getImageData(8,16,1,1).data,...x.getImageData(56,16,1,1).data];})()')

    d.call('Input.dispatchMouseEvent', type='mouseMoved', x=2, y=2)
    before = d.evaluate('document.getElementById("screen").toDataURL()')
    epoch = d.evaluate('document.getElementById("surface").dataset.inputEpoch')
    started = time.monotonic()
    cell(8);state('ready')
    cold_ms = (time.monotonic() - started) * 1000
    if pixels('#media-viewport img') != [255,0,0,255,0,0,255,255]:
        raise ValueError('PNG preview did not decode the independent red/blue expected image')
    if d.evaluate('document.getElementById("surface").dataset.keepAliveReady') != 'false':
        raise ValueError('preview did not pause keepalive')
    d.click('#media-plus');d.click('#media-plus')
    ratio = d.evaluate('(()=>{const b=document.querySelector(".media-image-stage").getBoundingClientRect();return b.width/b.height;})()')
    if abs(ratio - 2) > .001:
        raise ValueError('image zoom changed its aspect ratio')
    d.click('#media-original')
    if d.evaluate('document.querySelector(".media-image-stage").getBoundingClientRect().width') != 64:
        raise ValueError('original-size control failed')
    screenshot = d.call('Page.captureScreenshot', format='png')['data']
    (output / 'media-preview.png').write_bytes(base64.b64decode(screenshot))
    for kind in ('keyDown', 'keyUp'):
        d.call('Input.dispatchKeyEvent', type=kind, key='n', code='KeyN', windowsVirtualKeyCode=78)
    if d.evaluate('document.getElementById("surface").dataset.inputEpoch') != epoch:
        raise ValueError('media dialog leaked a key to PTT')
    d.click('#media-external')
    d.wait('document.getElementById("media-dialog").dataset.external === "true"', 'image external action did not reach native sink')
    close()
    started = time.monotonic()
    cell(8);state('ready')
    warm_ms = (time.monotonic() - started) * 1000
    if d.evaluate('document.getElementById("media-dialog").dataset.cached') != 'true' or server.counts.get('/colours.png') != 1:
        raise ValueError('second image preview was not a real cache hit')
    close()
    for row in (5, 6):
        cell(row);state('ready')
        colours = pixels('#media-viewport img:not([hidden]), #media-viewport canvas:not([hidden])')
        if colours[0] < 240 or colours[6] < 240:
            raise ValueError('JPEG/WebP preview lost red/blue colour content')
        close()
    cell(9);state('ready')
    if d.evaluate('document.getElementById("media-dialog").dataset.motion') != 'paused':
        raise ValueError('motion preview autoplayed')
    d.click('#media-motion')
    clip = d.evaluate('(()=>{const b=document.querySelector("#media-viewport img").getBoundingClientRect();return {x:b.x,y:b.y,width:b.width,height:b.height,scale:1};})()')
    deadline = time.monotonic() + 4
    frames = set()
    while time.monotonic() < deadline and len(frames) < 2:
        frames.add(d.call('Page.captureScreenshot', format='png', clip=clip)['data'])
        time.sleep(.05)
    if len(frames) != 2:
        raise ValueError('actual GIF frames did not animate')
    colours = set()
    for index, frame in enumerate(sorted(frames)):
        path = output / f'media-gif-frame-{index}.png'
        path.write_bytes(base64.b64decode(frame, validate=True))
        result = subprocess.run(['powershell.exe','-NoProfile','-NonInteractive','-File',
                                 str(FIXTURES.parents[2] / 'tooling/inspect_png.ps1'),
                                 '-PngFilePath',str(path),'-CheckMedia'], capture_output=True, text=True, timeout=10, check=True)
        decoded = json.loads(result.stdout)
        if decoded['width'] != 64 or decoded['height'] != 32:
            raise ValueError('unexpected GIF screenshot dimensions')
        colours.add(tuple(decoded['rgba']))
    if colours != {(255,0,0,255),(0,0,255,255)}:
        raise ValueError('rendered GIF frames did not match independent red/blue expected colours')
    d.click('#media-motion')
    frozen = pixels('#media-viewport canvas')
    time.sleep(.65)
    if pixels('#media-viewport canvas') != frozen or d.evaluate('document.querySelector("#media-viewport img").hasAttribute("src")'):
        raise ValueError('GIF pause did not freeze pixels/release active source')
    close();cell(11);state('failed');d.click('#media-retry');state('ready');close()
    if server.counts.get('/broken.png') != 2:
        raise ValueError('decoder-rejected response was cached instead of refetched')
    cell(14)
    deadline = time.monotonic() + 4
    while server.counts.get('/slow.png', 0) < 1 and time.monotonic() < deadline:
        time.sleep(.02)
    started = time.monotonic()
    close()
    if not server.cancelled.wait(3):
        raise ValueError('closed preview did not cancel the HTTP stream')
    cancel_ms = (time.monotonic() - started) * 1000
    counts = dict(server.counts)
    cell(18, external=True)
    if server.counts != counts or d.evaluate('document.getElementById("media-viewport").childElementCount') != 0:
        raise ValueError('YouTube fetched a thumbnail or allocated a preview')
    cell(10);state('ready')
    if d.evaluate('document.querySelector("#media-viewport video").preload') != 'none':
        raise ValueError('video was set to predownload')
    d.evaluate('document.querySelector("#media-viewport video").requestVideoFrameCallback(()=>document.getElementById("media-dialog").dataset.frame="presented")')
    d.click('#media-motion')
    try:
        d.wait('(()=>{const v=document.querySelector("#media-viewport video");return !v.paused&&v.currentTime>0&&v.videoWidth===64&&v.videoHeight===32&&document.getElementById("media-dialog").dataset.frame==="presented";})()', 'trusted video play did not present a frame')
    except TimeoutError as error:
        facts = d.evaluate('(()=>{const v=document.querySelector("#media-viewport video");return {paused:v.paused,time:v.currentTime,width:v.videoWidth,height:v.videoHeight,ready:v.readyState,network:v.networkState,error:v.error?.code,status:document.getElementById("media-status").textContent};})()')
        (output / 'media-video-failure.png').write_bytes(base64.b64decode(d.call('Page.captureScreenshot',format='png')['data']))
        raise ValueError(f'video playback failed: {facts}') from error
    (output / 'media-video.png').write_bytes(base64.b64decode(d.call('Page.captureScreenshot',format='png')['data']))
    close(escape=True)
    d.call('Input.dispatchMouseEvent', type='mouseMoved', x=2, y=2)
    if d.evaluate('document.getElementById("screen").toDataURL()') != before:
        raise ValueError('media browsing changed the terminal canvas')
    return {'status':'passed','png_pixels':True,'jpeg_webp_pixels':True,'aspect_ratio':True,
            'gif_frames_and_pause':True,'video_presented_frame':True,'retry':True,'cache_hit':True,
            'http_cancelled':True,'youtube_external_sink':True,'keyboard_and_canvas_unchanged':True,
            'timings_ms':{'cold_preview':round(cold_ms,2),'memory_cache_preview':round(warm_ms,2),'close_and_http_cancel':round(cancel_ms,2)},
            'measurement_scope':'single loopback 64x32 PNG; includes trusted input/CDP polling; no prior equivalent feature baseline; CPU/process-family memory not measured'}
