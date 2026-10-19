"""Hover requests, cancellation, promotion and placement with real pointer events."""
import time
import base64
from pathlib import Path

def run(d,output,server):
    surface='document.getElementById("surface")'
    hover='document.getElementById("media-hover")'
    def point(col,row):
        return d.evaluate('(()=>{const b=document.getElementById("screen").getBoundingClientRect();return {x:b.left+b.width*'+str(col)+'/80,y:b.top+b.height*'+str(row)+'/24};})()')
    def move(col,row):
        p=point(col,row);d.call('Input.dispatchMouseEvent',type='mouseMoved',**p);return p
    def ready():d.wait('!'+hover+'.hidden && '+hover+'.dataset.state === "ready"','hover image was not ready')
    def hidden():d.wait(hover+'.hidden','hover was not cancelled')
    def click(col,row):
        p=point(col,row)
        for kind in ('mousePressed','mouseReleased'):d.call('Input.dispatchMouseEvent',type=kind,**p,button='left',clickCount=1)
    def choose(index,value):
        d.call('Input.dispatchMouseEvent',type='mouseMoved',x=20,y=2)
        d.reveal_tools('#hover-position')
        # Focus a closed select: the OS popup cannot be driven by CDP.
        d.evaluate('document.getElementById("hover-position").focus()')
        for n,(name,vk) in enumerate([('Home',36)]+[('ArrowDown',40)]*index):
            for kind in ('keyDown','keyUp'):d.call('Input.dispatchKeyEvent',type=kind,key=name,code=name,windowsVirtualKeyCode=vk)
            expected=['top_right','follow_cursor','off'][n]
            d.wait(surface+'.dataset.hoverPosition === '+repr(expected),'hover setting was not acknowledged by its Rust store')
        d.wait('document.getElementById("hover-position").value === '+repr(value),'hover preference did not persist')
        d.close_tools()
        click(70,4.5)
    def count(path):
        with server.lock:return server.counts.get(path,0)
    def wait_count(path,n):
        deadline=time.monotonic()+5
        while count(path)!=n:
            if time.monotonic()>=deadline:raise ValueError('hover HTTP request count mismatch: '+path)
            time.sleep(.02)
    def cancel_eof():
        if not server.cancelled.wait(3):raise ValueError('hover cancellation did not close HTTP request')
    def modal_close():
        d.click('#media-close');d.wait('!document.getElementById("media-dialog").open && document.activeElement.id === "keyboard"','promoted modal did not close')
    epoch=d.evaluate(surface+'.dataset.inputEpoch')
    move(70,4.5);before=d.evaluate('document.getElementById("screen").toDataURL()')
    move(8,.5);time.sleep(.08);move(70,4.5);time.sleep(.4)
    if count('/hover.png')!=0:raise ValueError('quick hover caused a speculative request')
    server.cancelled.clear();move(8,1.5);wait_count('/hover-slow.png',1)
    move(8,.5);ready();cancel_eof()
    request=d.evaluate(hover+'.dataset.request')
    if d.evaluate('document.activeElement.id')!='keyboard' or d.evaluate('document.getElementById("media-dialog").open'):raise ValueError('hover stole editing focus')
    if d.evaluate('getComputedStyle('+hover+').pointerEvents')!='none':raise ValueError('hover intercepts pointer')
    move(12,.5);time.sleep(.4)
    if count('/hover.png')!=1:raise ValueError('same-link motion reloaded the image')
    geometry=d.evaluate('(()=>{const h=document.getElementById("media-hover").getBoundingClientRect(),s=document.querySelector("#media-hover-viewport .media-image-stage").getBoundingClientRect();return {x:h.x,y:h.y,right:h.right,bottom:h.bottom,ratio:s.width/s.height,w:innerWidth,h:innerHeight};})()')
    if abs(geometry['ratio']-2)>.01 or geometry['right']>geometry['w'] or geometry['bottom']>geometry['h']:raise ValueError('hover aspect ratio or viewport bounds changed')
    (Path(output)/'hover-top-right.png').write_bytes(base64.b64decode(d.call('Page.captureScreenshot',format='png')['data'],validate=True))
    click(12,.5)
    d.wait('document.getElementById("media-dialog").open && document.getElementById("media-dialog").dataset.state === "ready"','hover did not promote')
    if d.evaluate('document.getElementById("media-dialog").dataset.request')!=request or count('/hover.png')!=1:raise ValueError('promotion created a second request')
    modal_close()
    server.cancelled.clear();move(8,1.5);wait_count('/hover-slow.png',2)
    request=d.evaluate(hover+'.dataset.request');click(8,1.5)
    d.wait('document.getElementById("media-dialog").open','pending hover did not promote')
    if d.evaluate('document.getElementById("media-dialog").dataset.request')!=request or count('/hover-slow.png')!=2:raise ValueError('pending promotion restarted HTTP')
    modal_close();cancel_eof()
    choose(2,'off');move(8,.5);time.sleep(.45);hidden()
    if count('/hover.png')!=1:raise ValueError('disabled hover fetched an image')
    choose(1,'follow_cursor');p=move(8,.5);ready()
    first=d.evaluate(hover+'.getBoundingClientRect().x');move(16,.5)
    d.wait(hover+'.getBoundingClientRect().x !== '+repr(first),'follow preview did not follow pointer')
    original=d.evaluate('({width:innerWidth,height:innerHeight,canvasWidth:document.getElementById("screen").width,canvasHeight:document.getElementById("screen").height})')
    # Remove letterboxing so the bottom URL is also near the viewport edge.
    d.call('Emulation.setDeviceMetricsOverride',width=1040,height=624,deviceScaleFactor=1,mobile=False)
    d.wait('innerHeight === 624 && document.getElementById("screen").getBoundingClientRect().height > 620','hover edge viewport did not resize')
    p=move(74,22.5);ready()
    geometry=d.evaluate('(()=>{const b=document.getElementById("media-hover").getBoundingClientRect();return {left:b.left,top:b.top,right:b.right,bottom:b.bottom};})()')
    if geometry['right']>p['x']-8 or geometry['bottom']>p['y']-8 or min(geometry['left'],geometry['top'])<0:raise ValueError('follow preview did not flip at right/bottom edges: '+str((geometry,p)))
    (Path(output)/'hover-follow-edge.png').write_bytes(base64.b64decode(d.call('Page.captureScreenshot',format='png')['data'],validate=True))
    d.call('Emulation.clearDeviceMetricsOverride')
    d.wait('innerHeight === '+str(original['height'])+' && document.getElementById("screen").width === '+str(original['canvasWidth'])+' && document.getElementById("screen").height === '+str(original['canvasHeight']),'hover test viewport did not restore')
    move(8,8.5);ready()
    frame=d.evaluate('(()=>{const c=document.querySelector("#media-hover-viewport canvas"),x=c.getContext("2d");return {hidden:c.hidden,pixel:[...x.getImageData(8,8,1,1).data]};})()')
    time.sleep(.3)
    stable=d.evaluate('(()=>{const c=document.querySelector("#media-hover-viewport canvas");return [...c.getContext("2d").getImageData(8,8,1,1).data];})()')
    if frame['hidden'] or frame['pixel']!=stable or stable!=[255,0,0,255]:raise ValueError('hover animation did not stay on its first frame')
    d.call('Input.imeSetComposition',text='未',selectionStart=1,selectionEnd=1);hidden()
    d.call('Input.imeSetComposition',text='',selectionStart=0,selectionEnd=0)
    move(70,4.5);hidden()
    if before!=d.evaluate('document.getElementById("screen").toDataURL()') or epoch!=d.evaluate(surface+'.dataset.inputEpoch'):raise ValueError('hover changed canvas or sent PTT input')
    return {'status':'passed','dwell_and_same_link_single_request':True,'cancel_http_and_promote_ready_or_pending_without_reload':True,
            'top_right_follow_flip_off_and_saved_preference':True,'focus_canvas_and_ptt_input_unchanged':True,
            'animated_first_frame_and_ime_cancellation':True,
            'scope':'trusted Windows pointer with owned HTTP server; live sites and other OS unverified'}
