"""Portable display files through trusted controls; probe replaces only native pickers."""
import json
import base64
import time
from pathlib import Path

def run(d,output,edit_field,open_fonts):
    output=Path(output);path=output/'display.pttstella.json';settings=output/'probe-settings.json'
    initial_pixels=d.evaluate('document.getElementById("screen").toDataURL("image/png")')
    def action(name,result):
        d.evaluate('delete document.getElementById("font-dialog").dataset.profileResult')
        d.click('#profile-'+name)
        d.wait('document.getElementById("font-dialog").dataset.profileResult === '+repr(result),'profile '+name+' did not '+result)
    def choose_windows():
        # Focus a closed select; CDP cannot reliably drive the OS popup it opens.
        d.evaluate('document.getElementById("font-preset").focus()')
        index=d.evaluate('Array.from(document.getElementById("font-preset").options).findIndex(option=>option.value==="windows")')
        if not isinstance(index,int) or not 0<=index<16:raise ValueError('Windows preset option missing')
        for value,vk in [('Home',36)]+[('ArrowDown',40)]*index:
            for kind in ('keyDown','keyUp'):
                d.call('Input.dispatchKeyEvent',type=kind,key=value,code=value,windowsVirtualKeyCode=vk)
        d.wait('document.getElementById("font-preset").value === "windows"','Windows preset could not be selected')
    def close():
        d.click('#font-cancel');d.wait('!document.getElementById("font-dialog").open','profile draft did not cancel')
    def save():
        d.click('#font-save');d.wait('!document.getElementById("font-dialog").open','profile could not be applied')
    open_fonts();edit_field('#font-profile-name','合成設定 A');action('export','exported')
    custom=path.read_bytes();payload=json.loads(custom)
    (output/'display-profile-preview.png').write_bytes(base64.b64decode(d.call('Page.captureScreenshot',format='png')['data'],validate=True))
    if payload['name']!='合成設定 A' or payload['display']['fonts']!={'kind':'custom','ascii_font':'Courier New','cjk_font':'Microsoft JhengHei'} or 'hover' in payload:
        raise ValueError('portable export did not preserve explicit display scope')
    choose_windows()
    if d.evaluate('document.getElementById("font-size").value')!='18':raise ValueError('switching preset reset layout values')
    save();open_fonts()
    if d.evaluate('document.getElementById("font-preset").value')!='windows':raise ValueError('saved preset intent was inferred as custom')
    edit_field('#font-profile-name','合成設定 B');action('export','exported')
    fixed=path.read_bytes();payload=json.loads(fixed)
    if payload['display']['fonts']!={'kind':'preset','preset':'windows'}:raise ValueError('preset identity was lost during export')
    (output/'display-B.pttstella.json').write_bytes(fixed)
    path.write_bytes(custom);action('import','imported')
    if d.evaluate('document.getElementById("font-ascii").value')!='Courier New':raise ValueError('import did not populate preview')
    before=settings.read_bytes();close()
    if settings.read_bytes()!=before:raise ValueError('cancelled import changed local settings')
    open_fonts();action('import','imported');save()
    deadline=time.monotonic()+5
    while d.evaluate('document.getElementById("screen").toDataURL("image/png")')!=initial_pixels:
        if time.monotonic()>=deadline:raise ValueError('import did not restore the same rendering')
        time.sleep(.02)
    before=settings.read_bytes();open_fonts()
    for bad in [b'broken',b' '*8193,custom.replace(b'"version": 1',b'"version": 99')]:
        path.write_bytes(bad);action('import','rejected')
        if settings.read_bytes()!=before or path.read_bytes()!=bad:raise ValueError('failed import mutated preferences or source')
    path.write_bytes(custom);close()
    if d.evaluate('document.activeElement.id')!='keyboard':raise ValueError('profile dialog did not restore input focus')
    return {'status':'passed','fixed_preset_identity':True,'named_export_import_preview_cancel_and_apply':True,
            'invalid_import_preserves_source_and_preferences':True,
            'scope':'trusted Windows controls and real files; native pickers and other OS excluded'}
