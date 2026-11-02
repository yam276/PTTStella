"""Bounded CDP input driver for an isolated, synthetic WebView2 probe only."""
import base64
import hashlib
import http.client
import json
import os
import socket
import struct
import subprocess
import time
from urllib.parse import urlsplit
from pathlib import Path


def set_clipboard_text(text):
    # stdin carries synthetic UTF-8 data as base64; it is never shell source.
    script = '$value=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String([Console]::In.ReadToEnd())); Set-Clipboard -Value $value'
    result = subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-STA', '-Command', script],
                            input=base64.b64encode(text.encode('utf-8')), capture_output=True, timeout=5,
                            creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
    if result.returncode:
        raise ValueError('could not prepare synthetic clipboard data')


def copied_text_matches(expected):
    """Read the synthetic selection from Windows; never return clipboard contents."""
    import ctypes
    user = ctypes.WinDLL('user32', use_last_error=True)
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    user.GetClipboardData.argtypes = [ctypes.c_uint]
    user.GetClipboardData.restype = ctypes.c_void_p
    kernel.GlobalLock.argtypes = [ctypes.c_void_p]
    kernel.GlobalLock.restype = ctypes.c_void_p
    kernel.GlobalUnlock.argtypes = [ctypes.c_void_p]
    kernel.GlobalSize.argtypes = [ctypes.c_void_p]
    kernel.GlobalSize.restype = ctypes.c_size_t
    deadline = time.monotonic() + 2
    while not user.OpenClipboard(None):
        if time.monotonic() >= deadline:
            raise TimeoutError('synthetic clipboard could not be opened')
        time.sleep(.02)
    try:
        handle = user.GetClipboardData(13)  # CF_UNICODETEXT
        size = kernel.GlobalSize(handle) if handle else 0
        if not 0 < size <= 1024 * 1024:
            return False
        pointer = kernel.GlobalLock(handle)
        if not pointer:
            raise ValueError('synthetic clipboard could not be read')
        try:
            text = ctypes.wstring_at(pointer, size // 2).split(chr(0), 1)[0]
            return text == expected
        finally:
            kernel.GlobalUnlock(handle)
    finally:
        user.CloseClipboard()


class DevTools:
    def __init__(self, address, port):
        if not isinstance(address, str):
            raise ValueError('invalid debug target address')
        url = urlsplit(address)
        if url.scheme != 'ws' or url.hostname != '127.0.0.1' or url.port != port:
            raise ValueError('debug target must belong to this loopback probe')
        self.port = port
        self.address = address
        self.socket = socket.create_connection(('127.0.0.1', port), timeout=5)
        self.reader = self.socket.makefile('rb')
        self.sequence = 0
        try:
            self.handshake(url.path, port)
        except Exception:
            self.close()
            raise

    def handshake(self, path, port):
        key = base64.b64encode(os.urandom(16)).decode()
        request = (f'GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\n'
                   f'Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n')
        self.socket.sendall(request.encode('ascii'))
        status = self.reader.readline(4096)
        headers = {}
        for _ in range(64):
            line = self.reader.readline(4096)
            if line == b'\r\n':
                break
            name, value = line.decode('ascii').split(':', 1)
            headers[name.lower()] = value.strip()
        else:
            raise ValueError('oversized debug handshake')
        expected = base64.b64encode(hashlib.sha1((key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode()
        if b' 101 ' not in status or headers.get('sec-websocket-accept') != expected:
            raise ValueError('debug WebSocket handshake rejected')

    def close(self):
        self.reader.close()
        self.socket.close()

    def open_settings(self):
        self.call('Page.bringToFront')
        self.close_tools()
        self.click('#tools-ball')
        self.click('[data-tools-target="settings"]')
        self.wait('document.getElementById("font-dialog").open', 'in-app settings did not become ready')
        return self

    def exact(self, size):
        data = self.reader.read(size)
        if len(data) != size:
            raise ValueError('debug connection ended before response')
        return data

    def send(self, payload, opcode=1):
        mask = os.urandom(4)
        size = len(payload)
        if size > 65535:
            raise ValueError('debug request exceeds 64 KiB')
        header = bytes([128 | opcode, 128 | size]) if size < 126 else bytes([128 | opcode, 254]) + struct.pack('!H', size)
        self.socket.sendall(header + mask + bytes(value ^ mask[i % 4] for i, value in enumerate(payload)))

    def receive(self):
        result = bytearray()
        while True:
            first, second = self.exact(2)
            opcode, size = first & 15, second & 127
            if first & 112 or second & 128:
                raise ValueError('unsupported debug frame flags')
            if size == 126:
                size = struct.unpack('!H', self.exact(2))[0]
            elif size == 127:
                size = struct.unpack('!Q', self.exact(8))[0]
            if size + len(result) > 1024 * 1024:
                raise ValueError('debug response exceeds 1 MiB')
            payload = self.exact(size)
            if opcode == 9:
                self.send(payload, 10)
                continue
            if opcode == 10:
                continue
            if opcode not in (0, 1):
                raise ValueError('debug connection closed or returned non-text data')
            result.extend(payload)
            if first & 128:
                response = json.loads(result)
                if not isinstance(response, dict):
                    raise ValueError('debug response must be an object')
                return response

    def call(self, method, **params):
        self.sequence += 1
        request = {'id': self.sequence, 'method': method, 'params': params}
        self.send(json.dumps(request).encode('utf-8'))
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            response = self.receive()
            if response.get('id') == self.sequence:
                if 'error' in response:
                    raise ValueError(f'debug operation {method} failed: {response["error"]}')
                result = response.get('result', {})
                if not isinstance(result, dict):
                    raise ValueError('debug result must be an object')
                return result
        raise TimeoutError('debug operation exceeded deadline')

    def evaluate(self, expression):
        result = self.call('Runtime.evaluate', expression=expression, returnByValue=True)
        if 'exceptionDetails' in result:
            raise ValueError('probe DOM evaluation failed')
        return result.get('result', {}).get('value')

    def reveal_tools(self, selector):
        # Follow the production floating-tools entry using trusted clicks.
        # Hidden toolbar controls have no clickable rectangle until their
        # category is selected; moving to the former top hover area does nothing.
        group = self.evaluate('document.querySelector('+json.dumps(selector)+')?.closest("[data-tools-group]")?.dataset.toolsGroup ?? null')
        if group:
            ready = '(()=>{const n=document.querySelector('+json.dumps(selector)+');return !!n && n.getClientRects().length>0 && !!n.closest("[data-tools-group]") && !n.closest("[data-tools-group]").hidden;})()'
            if not self.evaluate(ready):
                action = self.evaluate('document.getElementById("tools-ball").dataset.toolsAction')
                if action == 'back' or self.evaluate('document.getElementById("tools-fan").hidden'):
                    self.click('#tools-ball')
                self.wait('!document.getElementById("tools-fan").hidden', 'tool categories did not open')
                self.click('[data-tools-target="'+group+'"]')
                self.wait(ready, 'tool category did not reveal '+selector)
        return group

    def click(self, selector):
        group = self.reveal_tools(selector)
        self.evaluate('(()=>{const n=document.querySelector('+json.dumps(selector)+');if(n.closest("dialog,[data-tools-group]"))n.scrollIntoView({block:"nearest",inline:"nearest"});})()')
        self.call('Runtime.evaluate', expression='new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))', awaitPromise=True)
        self.wait('(()=>{const n=document.querySelector('+json.dumps(selector)+'),b=n.getBoundingClientRect();return n.contains(document.elementFromPoint(b.left+b.width/2,b.top+b.height/2));})()', 'control is not hit-testable: '+selector)
        position = self.evaluate(f'(()=>{{const b=document.querySelector({json.dumps(selector)}).getBoundingClientRect();'
                                 'return {x:b.left+b.width/2,y:b.top+b.height/2};})()')
        self.call('Input.dispatchMouseEvent', type='mouseMoved', **position, buttons=0)
        for kind in ('mousePressed', 'mouseReleased'):
            self.call('Input.dispatchMouseEvent', type=kind, **position, button='left',
                      buttons=1 if kind == 'mousePressed' else 0, clickCount=1)
        opens_dialog = selector in ('#login-open',
                                    '#destination-open', '#bookmarks-open', '#sites-open')
        if group and not opens_dialog and self.evaluate('document.querySelector('+json.dumps(selector)+').tagName === "BUTTON" && !document.querySelector("dialog[open]")'):
            # A still-open tool page intentionally consumes the next outside
            # click. Close it through the real trigger before testing the BBS.
            self.close_tools()

    def close_tools(self):
        if self.evaluate('document.getElementById("tools-ball").dataset.toolsAction === "back"'):
            self.click('#tools-ball')
        if self.evaluate('!document.getElementById("tools-fan").hidden'):
            self.click('#tools-ball')

    def shortcut(self, key):
        self.call('Input.dispatchKeyEvent', type='keyDown', key='Control', code='ControlLeft', windowsVirtualKeyCode=17, modifiers=2)
        for kind in ('keyDown', 'keyUp'):
            self.call('Input.dispatchKeyEvent', type=kind, key=key, code='Key'+key.upper(),
                      windowsVirtualKeyCode=ord(key.upper()), modifiers=2)
        self.call('Input.dispatchKeyEvent', type='keyUp', key='Control', code='ControlLeft', windowsVirtualKeyCode=17, modifiers=0)

    def wait(self, expression, reason):
        deadline = time.monotonic() + 5
        while not self.evaluate(expression):
            if time.monotonic() >= deadline:
                if getattr(self, 'artifact_output', None):
                    (Path(self.artifact_output)/'failure.png').write_bytes(base64.b64decode(self.call('Page.captureScreenshot',format='png')['data'],validate=True))
                state=self.evaluate('(()=>{const d=document.getElementById("surface"),p=document.getElementById("font-preview");return {surface:{...d?.dataset},font:{...p?.dataset},fontNote:document.getElementById("font-note")?.textContent,preset:document.getElementById("font-preset")?.value,fontSize:document.getElementById("font-size")?.value,rowsTitle:document.getElementById("export-rows")?.title,toolsAction:document.getElementById("tools-ball")?.dataset.toolsAction};})()')
                raise TimeoutError(reason+'; synthetic probe state: '+json.dumps(state,ensure_ascii=False))
            time.sleep(.02)


def debug_pages(port):
    """Discover only the owned probe's bounded loopback page list."""
    connection = http.client.HTTPConnection('127.0.0.1', port, timeout=1)
    try:
        connection.request('GET', '/json/list')
        response = connection.getresponse()
        data = response.read(1024 * 1024 + 1)
        if response.status != 200 or len(data) > 1024 * 1024:
            raise ValueError('invalid debug target response')
        targets = json.loads(data)
        if not isinstance(targets, list) or len(targets) > 8 or any(not isinstance(p, dict) for p in targets):
            raise ValueError('invalid debug target list')
        addresses = []
        for target in targets:
            if target.get('type') != 'page':
                continue
            address = target.get('webSocketDebuggerUrl')
            url = urlsplit(address) if isinstance(address, str) else None
            if url is None or url.scheme != 'ws' or url.hostname != '127.0.0.1' or url.port != port:
                raise ValueError('debug target must belong to this loopback probe')
            addresses.append(address)
        return addresses
    finally:
        connection.close()


def run(port, process, output=None, media_server=None):
    """No credentials, PTT traffic, arbitrary target selection, or OS-wide input."""
    deadline = time.monotonic() + 15
    address = None
    while time.monotonic() < deadline and process.poll() is None:
        try:
            pages = debug_pages(port)
            if len(pages) == 1:
                address = pages[0]
                break
        except (OSError, ValueError, http.client.HTTPException):
            pass
        time.sleep(.05)
    if not address:
        raise TimeoutError('isolated WebView debug target unavailable')
    devtools = DevTools(address, port)
    devtools.artifact_output = output
    try:
        while time.monotonic() < deadline:
            if devtools.evaluate('document.getElementById("surface")?.dataset.probeReady === "true"'):
                break
            time.sleep(.05)
        else:
            raise TimeoutError('synthetic probe did not become ready')
        devtools.call('Page.bringToFront')
        if media_server is None or output is None:
            raise ValueError('media conformance requires the owned loopback fixture server')
        import desktop_media
        media_report = desktop_media.run(devtools, output, media_server)
        # Select only the right half of the synthetic dual-colour Chinese glyph.
        bounds = devtools.evaluate('(()=>{const b=document.getElementById("screen").getBoundingClientRect();return {left:b.left,top:b.top,width:b.width,height:b.height};})()')
        col, row = bounds['width'] / 80, bounds['height'] / 24
        link_x, link_y = bounds['left'] + col * 5.3, bounds['top'] + row * 11.5
        devtools.call('Input.dispatchMouseEvent', type='mousePressed', x=link_x, y=link_y, button='left', clickCount=1)
        devtools.call('Input.dispatchMouseEvent', type='mouseMoved', x=link_x+col*2, y=link_y, button='left', buttons=1)
        devtools.call('Input.dispatchMouseEvent', type='mouseReleased', x=link_x+col*2, y=link_y, button='left', clickCount=1)
        devtools.wait('!!document.getElementById("surface").dataset.selectionReady', 'URL drag did not select text')
        if devtools.evaluate('document.getElementById("surface").dataset.linkResult'):
            raise ValueError('dragging a URL requested a browser launch')
        for kind in ('mousePressed', 'mouseReleased'):
            devtools.call('Input.dispatchMouseEvent', type=kind, x=link_x, y=link_y, button='left', clickCount=1)
        devtools.wait('document.getElementById("surface").dataset.linkResult === "requested"', 'URL click did not reach the opener')
        for kind in ('mousePressed', 'mouseReleased'):
            devtools.call('Input.dispatchMouseEvent', type=kind, x=link_x, y=link_y, button='right', clickCount=1, modifiers=8)
        devtools.wait('!document.getElementById("surface-context-menu").hidden', 'right-click did not open the context menu')
        devtools.click('[data-context-action="select-link"]')
        devtools.wait('!!document.getElementById("surface").dataset.selectionReady', 'context menu did not select the complete URL')
        devtools.shortcut('c')
        if not copied_text_matches('https://example.com/pttstella'):
            raise ValueError('URL clipboard text did not match the displayed URL')
        x, y = bounds['left'] + col * 1.3, bounds['top'] + row * 3.5
        devtools.call('Input.dispatchMouseEvent', type='mousePressed', x=x, y=y, button='left', clickCount=1)
        devtools.call('Input.dispatchMouseEvent', type='mouseMoved', x=x+col*.4, y=y, button='left', buttons=1)
        devtools.call('Input.dispatchMouseEvent', type='mouseReleased', x=x+col*.4, y=y, button='left', clickCount=1)
        deadline = time.monotonic() + 5
        while not devtools.evaluate('!!document.getElementById("surface").dataset.selectionReady'):
            if time.monotonic() >= deadline:
                raise TimeoutError('selection text was not prepared')
            time.sleep(.02)
        for kind in ('keyDown', 'keyUp'):
            devtools.call('Input.dispatchKeyEvent', type=kind, key='c', code='KeyC', windowsVirtualKeyCode=67, modifiers=2)
        if not copied_text_matches('中'):
            raise ValueError('system clipboard did not contain the complete selected glyph')
        before_export = devtools.evaluate('({width:document.getElementById("screen").width,height:document.getElementById("screen").height,left:document.querySelector(".workspace").scrollLeft,top:document.querySelector(".workspace").scrollTop})')
        devtools.call('Input.dispatchMouseEvent', type='mouseMoved', x=20, y=2)
        devtools.click('#export-png')
        devtools.wait('document.getElementById("surface").dataset.exportResult === "saved"', 'PNG export did not complete')
        after_export = devtools.evaluate('({width:document.getElementById("screen").width,height:document.getElementById("screen").height,left:document.querySelector(".workspace").scrollLeft,top:document.querySelector(".workspace").scrollTop})')
        if before_export != after_export:
            raise ValueError('PNG export changed visible geometry or scrolling')
        if output is not None:
            inspection = subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
                                         '-File', str(Path(__file__).with_name('inspect_png.ps1')),
                                         '-PngFilePath', str(Path(output)/'export.png')], capture_output=True, timeout=5,
                                        creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
            if inspection.returncode:
                raise ValueError('PNG could not be independently decoded')
            image = json.loads(inspection.stdout)
            if image != {'width':960,'height':576,'samples':[[0,0,128,255],[128,0,128,255],[128,0,0,255],[0,0,0,255]]}:
                raise ValueError('exported PNG pixels, background or cursor exclusion did not match')
            symbols = subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
                                      '-File', str(Path(__file__).with_name('inspect_png.ps1')),
                                      '-PngFilePath', str(Path(output)/'export.png'), '-CheckSymbols'],
                                     capture_output=True, timeout=5,
                                     creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
            if symbols.returncode:
                raise ValueError('symbol PNG inspection did not run')
            symbol_bounds = json.loads(symbols.stdout)['symbols']
            if len(symbol_bounds) != 4 or any(min(w,h)<6 or abs(w-h)>2 for w,h in symbol_bounds):
                raise ValueError('circular symbol ink was distorted: '+repr(symbol_bounds))
        import desktop_capture
        capture_report = desktop_capture.run(devtools, output)
        import desktop_settings
        settings_report, profiles_report = desktop_settings.run(devtools, output)
        import desktop_hover
        hover_report=desktop_hover.run(devtools,output,media_server)
        devtools.call('Input.dispatchMouseEvent', type='mouseMoved', x=20, y=2)
        devtools.click('#select-all')
        deadline = time.monotonic() + 5
        while devtools.evaluate('document.querySelectorAll(".selection-row").length') != 24:
            if time.monotonic() >= deadline:
                raise TimeoutError('whole-page selection was not rendered')
            time.sleep(.02)
        # Clearing selection restores the original PTT Ctrl+C operation.
        devtools.click('#screen')
        for kind in ('keyDown', 'keyUp'):
            devtools.call('Input.dispatchKeyEvent', type=kind, key='c', code='KeyC', windowsVirtualKeyCode=67, modifiers=2)
        focused = []
        # Real browser mouse defaults and editing, including canvas, letterbox and toolbar.
        for selector, text in (('#screen', 'a'), ('.workspace', 'b'), ('#zoom-in', 'c')):
            if selector == '.workspace':
                for kind in ('mousePressed', 'mouseReleased'):
                    devtools.call('Input.dispatchMouseEvent', type=kind, x=20, y=20, button='left', clickCount=1)
            else:
                if selector == '#zoom-in':
                    devtools.call('Input.dispatchMouseEvent', type='mouseMoved', x=20, y=2)
                devtools.click(selector)
            active = devtools.evaluate('document.activeElement?.id')
            focused.append(active)
            if active != 'keyboard':
                raise ValueError(f'PTT input lost focus after {selector}: {active!r}')
            devtools.call('Input.insertText', text=text)
        devtools.call('Input.imeSetComposition', text='未', selectionStart=1, selectionEnd=1)
        if devtools.evaluate('getComputedStyle(document.getElementById("keyboard")).opacity') != '1':
            raise ValueError('composition host did not become visible')
        inline_host = devtools.evaluate('''(()=>{
            const k=document.getElementById("keyboard"),s=getComputedStyle(k),b=k.getBoundingClientRect();
            return s.outlineStyle==="none" && s.borderTopWidth==="0px" && s.boxShadow==="none"
                && Math.abs(parseFloat(s.lineHeight)-b.height)<1 && k.wrap==="off"
                && b.width>1 && b.width<5*b.height;
        })()''')
        if not inline_host:
            raise ValueError('preedit did not use a borderless, content-sized single-line host')
        placement = devtools.evaluate('(()=>{const k=document.getElementById("keyboard").getBoundingClientRect(),c=document.getElementById("screen").getBoundingClientRect();return {left:k.left-c.left,top:k.top-c.top,frameHeight:c.height};})()')
        # Synthetic frame parks the cursor at column 0, row 18.
        if abs(placement['left']) > 1 or abs(placement['top']-18*placement['frameHeight']/24) > 2:
            raise ValueError('composition host did not follow the PTT cursor')
        if output is not None:
            screenshot = devtools.call('Page.captureScreenshot', format='png')['data']
            (Path(output)/'ime-inline.png').write_bytes(base64.b64decode(screenshot, validate=True))
        devtools.call('Input.imeSetComposition', text='', selectionStart=0, selectionEnd=0)
        devtools.call('Input.imeSetComposition', text='中', selectionStart=1, selectionEnd=1)
        devtools.call('Input.insertText', text='中')
        # Negative regression only: model legacy WebKit's keydown AFTER the real
        # compositionend, with isComposing=false but keyCode=229. It must neither
        # cancel native IME defaults nor leak bytes to the loopback server. These
        # dispatched events do not establish native macOS IME support.
        ime_tail = devtools.evaluate('''(()=>{
            const k=document.getElementById("keyboard");
            const keys=["Enter","Escape","Backspace","ArrowLeft"];
            const nativeDefaults=keys.every(key=>k.dispatchEvent(new KeyboardEvent("keydown",{
                key,code:key,keyCode:229,isComposing:false,bubbles:true,cancelable:true
            })));
            const breaks=["insertLineBreak","insertParagraph"];
            const prevented=breaks.every(inputType=>!k.dispatchEvent(new InputEvent("beforeinput",{
                inputType,data:null,isComposing:false,bubbles:true,cancelable:true
            })));
            // Also exercise the input fallback when beforeinput was non-cancelable.
            for(const inputType of breaks){
                k.dispatchEvent(new InputEvent("beforeinput",{inputType,bubbles:true}));
                k.value="\\n";
                k.dispatchEvent(new InputEvent("input",{inputType,bubbles:true}));
            }
            return nativeDefaults && prevented && k.value==="";
        })()''')
        if not ime_tail:
            raise ValueError('legacy IME confirmation leaked a BBS key or textarea line break')
        # The very next ordinary Enter is a distinct user action and must arrive
        # exactly once, with no timing window that swallows it after composition.
        for kind in ('keyDown', 'keyUp'):
            devtools.call('Input.dispatchKeyEvent', type=kind, key='Enter', code='Enter', windowsVirtualKeyCode=13)
        # A new edit with identical text must not be discarded as a duplicate commit.
        devtools.call('Input.insertText', text='中')
        set_clipboard_text('A\nB')
        devtools.shortcut('v')
        devtools.wait('document.getElementById("paste-dialog").open', 'multiline paste did not open a preview')
        if devtools.evaluate('document.querySelector("#paste-dialog pre").textContent') != 'A\nB':
            raise ValueError('paste preview changed the input text')
        devtools.click('#paste-cancel')
        devtools.wait('!document.getElementById("paste-dialog").open', 'cancelled paste preview did not close')
        set_clipboard_text('中')
        devtools.shortcut('v')
        devtools.wait('document.getElementById("surface").dataset.pasteResult === "queued"', 'single-line paste was not queued')
        set_clipboard_text('A\nB')
        devtools.shortcut('v')
        devtools.wait('document.getElementById("paste-dialog").open', 'confirmed paste preview did not open')
        devtools.click('#paste-send')
        devtools.wait('!document.getElementById("paste-dialog").open', 'confirmed paste preview did not close')
        # The multi-site UI now describes the current site's encoding. Keep the
        # typed rejection plus reason, and the peer still rejects extra bytes.
        for text, expected in [('valid🚀', '無法以目前站點編碼'), ('X'*4097, '超過 4096'), ('\x1b[31mX', '控制字元')]:
            set_clipboard_text(text)
            devtools.shortcut('v')
            devtools.wait('document.getElementById("surface").dataset.pasteResult === "rejected" && document.getElementById("surface-note").textContent.includes('+json.dumps(expected)+')',
                          'invalid paste was not rejected')
        previous = devtools.evaluate('document.getElementById("surface").dataset.revision')
        for key, code in (('ArrowUp', 38), ('Enter', 13)):
            for kind in ('keyDown', 'keyUp'):
                devtools.call('Input.dispatchKeyEvent', type=kind, key=key, code=key, windowsVirtualKeyCode=code)
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if devtools.evaluate('document.getElementById("surface").dataset.revision') != previous:
                break
            time.sleep(.02)
        else:
            raise TimeoutError('WebSocket confirmation frame was not rendered')
        devtools.click('#screen')
        for kind in ('keyDown', 'keyUp'):
            args = {'type': kind, 'key': 'n', 'code': 'KeyN', 'windowsVirtualKeyCode': 78}
            if kind == 'keyDown':
                args['text'] = 'n'
            devtools.call('Input.dispatchKeyEvent', **args)
        for kind in ('keyDown', 'keyUp'):
            devtools.call('Input.dispatchKeyEvent', type=kind, key='Enter', code='Enter', windowsVirtualKeyCode=13)
        import desktop_navigation
        navigation_report=desktop_navigation.run(devtools,output)
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            notice = devtools.evaluate('document.getElementById("chrome").dataset.attention')
            status = devtools.evaluate('document.querySelector(".status").textContent')
            if notice == 'true' and status == '站端已關閉連線（1008）：synthetic confirmation complete':
                break
            time.sleep(.02)
        else:
            active = devtools.evaluate('document.activeElement?.id')
            result = devtools.evaluate('document.getElementById("surface").dataset.pasteResult')
            raise TimeoutError(f'peer close reason was not shown: {status!r}; focus={active!r}; paste={result!r}')
        devtools.call('Input.dispatchMouseEvent', type='mouseMoved', x=20, y=100)
        if devtools.evaluate('getComputedStyle(document.querySelector("header")).opacity') != '1':
            raise ValueError('peer close reason was hidden without hover')
        # Trusted acknowledgement lets the probe stop only after inspecting the closed UI.
        for kind in ('keyDown', 'keyUp'):
            devtools.call('Input.dispatchKeyEvent', type=kind, key='ArrowRight', code='ArrowRight', windowsVirtualKeyCode=39)
        return {'status': 'passed', 'focused_after_clicks': focused,
                'system_clipboard_copy': True,
                'composition_host_position': True,
                'composition_commit_cancel_repeat': True,
                'composition_inline_host': True,
                'legacy_webkit_ime_tail': 'simulated negative events; not native macOS IME',
                'ordinary_enter_after_composition': True,
                'native_paste_preview_and_validation': True,
                'url_click_drag_copy': True,
                'png_geometry_and_pixels': output is not None,
                'circular_symbol_ink_bounds': symbol_bounds,
                'selected_rows_png': capture_report,
                'font_preview_save_cancel_validation': True,
                'mouse_navigation': navigation_report,
                'settings_center':settings_report,
                'display_profiles':profiles_report,
                'hover_preview':hover_report,
                'media': media_report,
                'peer_close_visible': True,
                'scope': 'isolated WebView2 browser input; no physical keyboard or PTT authentication'}
    finally:
        devtools.close()
