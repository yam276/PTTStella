"""Bounded CDP input driver for an isolated, synthetic WebView2 probe only."""
import base64
import hashlib
import http.client
import json
import os
import socket
import struct
import time
from urllib.parse import urlsplit


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


def run(port, process):
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
        devtools.call('Input.insertText', text='中')
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
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            notice = devtools.evaluate('document.getElementById("chrome").dataset.attention')
            status = devtools.evaluate('document.querySelector(".status").textContent')
            if notice == 'true' and status == '站端已關閉連線（1008）：synthetic confirmation complete':
                break
            time.sleep(.02)
        else:
            raise TimeoutError('peer close reason was not shown')
        devtools.call('Input.dispatchMouseEvent', type='mouseMoved', x=20, y=100)
        if devtools.evaluate('getComputedStyle(document.querySelector("header")).opacity') != '1':
            raise ValueError('peer close reason was hidden without hover')
        # Trusted acknowledgement lets the probe stop only after inspecting the closed UI.
        for kind in ('keyDown', 'keyUp'):
            devtools.call('Input.dispatchKeyEvent', type=kind, key='ArrowRight', code='ArrowRight', windowsVirtualKeyCode=39)
        return {'status': 'passed', 'focused_after_clicks': focused,
                'system_clipboard_copy': True,
                'peer_close_visible': True,
                'scope': 'isolated WebView2 browser input; no physical keyboard or PTT authentication'}
    finally:
        devtools.close()
