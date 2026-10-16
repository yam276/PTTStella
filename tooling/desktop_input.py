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

    def click(self, selector):
        position = self.evaluate(f'(()=>{{const b=document.querySelector({json.dumps(selector)}).getBoundingClientRect();'
                                 'return {x:b.left+b.width/2,y:b.top+b.height/2};})()')
        for kind in ('mousePressed', 'mouseReleased'):
            self.call('Input.dispatchMouseEvent', type=kind, **position, button='left', clickCount=1)

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
                raise TimeoutError(reason)
            time.sleep(.02)


def run(port, process, output=None):
    """No credentials, PTT traffic, arbitrary target selection, or OS-wide input."""
    deadline = time.monotonic() + 15
    address = None
    while time.monotonic() < deadline and process.poll() is None:
        try:
            connection = http.client.HTTPConnection('127.0.0.1', port, timeout=1)
            try:
                connection.request('GET', '/json/list')
                response = connection.getresponse()
                targets = json.loads(response.read(1024 * 1024))
                if not isinstance(targets, list) or any(not isinstance(p, dict) for p in targets):
                    raise ValueError('invalid debug target list')
                pages = [p for p in targets if p.get('type') == 'page']
            finally:
                connection.close()
            if len(pages) == 1:
                address = pages[0].get('webSocketDebuggerUrl')
                break
        except (OSError, ValueError, http.client.HTTPException):
            pass
        time.sleep(.05)
    if not address:
        raise TimeoutError('isolated WebView debug target unavailable')
    devtools = DevTools(address, port)
    try:
        while time.monotonic() < deadline:
            if devtools.evaluate('document.getElementById("surface")?.dataset.probeReady === "true"'):
                break
            time.sleep(.05)
        else:
            raise TimeoutError('synthetic probe did not become ready')
        devtools.call('Page.bringToFront')
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
            devtools.call('Input.dispatchMouseEvent', type=kind, x=link_x, y=link_y, button='right', clickCount=1)
        devtools.wait('!!document.getElementById("surface").dataset.selectionReady', 'right-click did not select the URL')
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
        # Edit actual form controls; cancelled previews must not change the screen/store.
        initial_pixels = devtools.evaluate('document.getElementById("screen").toDataURL("image/png")')
        def edit_field(selector, value):
            devtools.click(selector)
            devtools.shortcut('a')
            devtools.call('Input.insertText', text=value)
        def open_fonts():
            devtools.call('Input.dispatchMouseEvent', type='mouseMoved', x=20, y=2)
            devtools.click('#font-settings')
            devtools.wait('document.getElementById("font-dialog").open', 'font dialog did not open')
        open_fonts()
        edit_field('#font-size', '18')
        edit_field('#font-baseline', '19')
        devtools.wait('document.getElementById("font-preview").dataset.fontSize === "18" && document.getElementById("font-preview").dataset.baseline === "19"',
                      'font preview did not render the validated draft')
        devtools.click('#font-cancel')
        devtools.wait('!document.getElementById("font-dialog").open', 'font preview did not cancel')
        if devtools.evaluate('document.getElementById("screen").toDataURL("image/png")') != initial_pixels:
            raise ValueError('cancelled font preview changed the PTT screen')
        if output is not None and (Path(output)/'probe-settings.json').exists():
            raise ValueError('cancelled preview saved preferences')
        open_fonts()
        if devtools.evaluate('document.getElementById("font-size").value') != '20':
            raise ValueError('reopening settings retained an unsaved draft')
        edit_field('#font-size', '18')
        edit_field('#font-baseline', '19')
        edit_field('#font-ascii', 'Courier New')
        devtools.click('#font-save')
        devtools.wait('!document.getElementById("font-dialog").open && document.getElementById("surface").dataset.fontSize === "18" && document.getElementById("surface").dataset.baseline === "19"',
                      'saved font settings did not reach the PTT renderer')
        changed_pixels = devtools.evaluate('document.getElementById("screen").toDataURL("image/png")')
        if changed_pixels == initial_pixels:
            raise ValueError('saved font metrics did not affect rendered glyph pixels')
        if output is not None:
            (Path(output)/'settings-after.png').write_bytes(base64.b64decode(changed_pixels.split(',',1)[1], validate=True))
            saved_bytes = (Path(output)/'probe-settings.json').read_bytes()
            saved = json.loads(saved_bytes)
            if saved['font_size'] != 18 or saved['baseline'] != 19 or saved['ascii_font'] != 'Courier New':
                raise ValueError('saved configuration did not contain the applied settings')
        open_fonts()
        edit_field('#font-size', '100')
        devtools.click('#font-save')
        devtools.wait('document.getElementById("font-note").textContent.includes("字高需介於")', 'invalid font metrics were not rejected')
        if not devtools.evaluate('document.getElementById("font-dialog").open'):
            raise ValueError('failed settings save closed the draft')
        devtools.click('#font-defaults')
        devtools.wait('document.getElementById("font-preview").dataset.fontSize === "20" && document.getElementById("font-preview").dataset.baseline === "20"',
                      'default font metrics were not previewed')
        devtools.click('#font-cancel')
        if output is not None and (Path(output)/'probe-settings.json').read_bytes() != saved_bytes:
            raise ValueError('invalid or cancelled settings replaced saved configuration')
        # Default preview does not commit until the user saves.
        if devtools.evaluate('document.getElementById("surface").dataset.fontSize') != '18':
            raise ValueError('default preview changed the committed settings')
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
        placement = devtools.evaluate('(()=>{const k=document.getElementById("keyboard").getBoundingClientRect(),c=document.getElementById("screen").getBoundingClientRect();return {left:k.left-c.left,top:k.top-c.top,frameHeight:c.height};})()')
        # Synthetic frame parks the cursor at column 0, row 18.
        if abs(placement['left']) > 1 or abs(placement['top']-18*placement['frameHeight']/24) > 2:
            raise ValueError('composition host did not follow the PTT cursor')
        devtools.call('Input.imeSetComposition', text='', selectionStart=0, selectionEnd=0)
        devtools.call('Input.imeSetComposition', text='中', selectionStart=1, selectionEnd=1)
        devtools.call('Input.insertText', text='中')
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
        for text, expected in [('valid🚀', '無法以 Big5-UAO'), ('X'*4097, '超過 4096'), ('\x1b[31mX', '控制字元')]:
            set_clipboard_text(text)
            devtools.shortcut('v')
            devtools.wait('document.getElementById("surface-note").textContent.includes('+json.dumps(expected)+')',
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
                'native_paste_preview_and_validation': True,
                'url_click_drag_copy': True,
                'png_geometry_and_pixels': output is not None,
                'circular_symbol_ink_bounds': symbol_bounds,
                'selected_rows_png': capture_report,
                'font_preview_save_cancel_validation': True,
                'mouse_navigation': navigation_report,
                'peer_close_visible': True,
                'scope': 'isolated WebView2 browser input; no physical keyboard or PTT authentication'}
    finally:
        devtools.close()
