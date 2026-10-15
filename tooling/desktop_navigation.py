"""Trusted browser browsing input against the existing owned loopback probe."""
import base64
from pathlib import Path


def run(devtools, output):
    d=devtools
    def wait_page(kind, first=None, cursor=None):
        expression='document.getElementById("surface").dataset.navigation === '+repr(kind)
        if kind!='unknown': expression+=' && document.getElementById("surface").dataset.navigationReady === "true"'
        if first is not None: expression+=' && document.getElementById("surface").dataset.readingFirst === '+repr(str(first))
        if cursor is not None: expression+=' && document.getElementById("surface").dataset.cursorRow === '+repr(str(cursor))
        d.wait(expression,'navigation page did not become ready: '+kind)
    def point(col,row):
        return d.evaluate('(()=>{const b=document.getElementById("screen").getBoundingClientRect();return {x:b.left+b.width*'+str(col)+'/80,y:b.top+b.height*'+str(row)+'/24};})()')
    def mouse(kind,position,count=1,**kw):
        d.call('Input.dispatchMouseEvent',type=kind,**position,button='left',clickCount=count,**kw)
    def click_cell(col,row,count=1):
        p=point(col,row)
        mouse('mousePressed',p,count);mouse('mouseReleased',p,count)
    def key(name,code):
        for kind in ('keyDown','keyUp'):
            d.call('Input.dispatchKeyEvent',type=kind,key=name,code=name,windowsVirtualKeyCode=code)
    def wheel(delta,ctrl=False):
        d.call('Input.dispatchMouseEvent',type='mouseWheel',**point(50,10),deltaX=0,deltaY=delta,modifiers=2 if ctrl else 0)
    def clear_selection(): click_cell(70,.5)
    def toolbar(selector):
        d.call('Input.dispatchMouseEvent',type='mouseMoved',x=20,y=2)
        d.click(selector)
    def screenshot(name):
        if output is not None:
            data=d.call('Page.captureScreenshot',format='png')['data']
            (Path(output)/name).write_bytes(base64.b64decode(data,validate=True))

    wait_page('article_list',cursor=4)
    toolbar('#zoom-fit')
    # A real keyboard event while the pointer is held invalidates its browsing lease.
    held=point(4,5.5);mouse('mousePressed',held)
    key('ArrowRight',39)
    wait_page('article_list',cursor=3)
    mouse('mouseReleased',held)
    # Titles keep normal click/drag/double-click selection rather than opening rows.
    click_cell(40,4.5);click_cell(40,4.5,count=2)
    # Renderer rounds shared grid edges to pixels; its last edge may be one pixel
    # inside the canvas capacity. The selected row must cover that actual grid.
    d.wait('document.querySelectorAll(".selection-row").length === 1 && Math.abs(document.querySelector(".selection-row").getBoundingClientRect().width-Math.round(Math.min(document.getElementById("screen").width/960,document.getElementById("screen").height/576)*960)/devicePixelRatio)<.01',
           'title double-click did not retain whole-line selection')
    clear_selection()
    held=point(4,3.5);mouse('mousePressed',held)
    d.call('Input.dispatchMouseEvent',type='mouseMoved',**point(5,3.5),button='left',buttons=1)
    mouse('mouseReleased',point(5,3.5))
    d.wait('!!document.getElementById("surface").dataset.selectionReady','dragging a number did not select text')
    clear_selection()
    # Native lostpointercapture must cancel a held click without sending it.
    held=point(4,5.5);mouse('mousePressed',held)
    d.call('Input.dispatchMouseEvent',type='mouseMoved',**held,button='left',buttons=1)
    d.evaluate('document.querySelector(".workspace").releasePointerCapture(Number(document.getElementById("surface").dataset.pointerId))')
    # Pending capture changes are processed at the next native pointer event.
    d.call('Input.dispatchMouseEvent',type='mouseMoved',**held,button='left',buttons=1)
    d.wait('document.getElementById("surface").dataset.pointerActive === "false"','lost capture did not cancel pointer transaction')
    mouse('mouseReleased',held)

    before=d.evaluate('document.getElementById("screen").toDataURL("image/png")')
    d.call('Input.dispatchMouseEvent',type='mouseMoved',**point(4,5.5))
    d.wait('document.getElementById("navigation-marker").dataset.row === "5"','article number hover did not display its marker')
    marker=d.evaluate('(()=>{const m=document.getElementById("navigation-marker"),s=getComputedStyle(m),b=m.getBoundingClientRect();return {background:s.backgroundColor,border:s.borderBottomWidth,width:b.width,screen:document.getElementById("screen").getBoundingClientRect().width};})()')
    if marker['background']!='rgba(0, 0, 0, 0)' or marker['border']!='1px' or marker['width']>marker['screen']*.08:
        raise ValueError('navigation hover obscured the BBS row instead of marking its number')
    if before!=d.evaluate('document.getElementById("screen").toDataURL("image/png")'):
        raise ValueError('article hover recoloured ANSI canvas pixels')
    screenshot('navigation-list-hover.png')
    click_cell(4,5.5)
    # A double-click continuation must not send a second open or select the new page.
    click_cell(4,5.5,count=2)
    wait_page('article_reading',first=1)
    d.call('Input.dispatchMouseEvent',type='mouseMoved',x=20,y=2)
    screenshot('navigation-reading-controls.png')
    if not d.evaluate('document.getElementById("browse-previous").disabled && !document.getElementById("browse-next").disabled'):
        raise ValueError('reading page boundary controls were incorrect')
    wheel(120)
    wait_page('article_reading',first=24)
    # Ctrl+wheel scales; ordinary wheel scrolls the enlarged canvas locally.
    wheel(-120,ctrl=True)
    d.wait('document.getElementById("zoom-level").textContent === "125%"','Ctrl+wheel did not preserve proportional zoom')
    initial_scroll=d.evaluate('document.querySelector(".workspace").scrollTop')
    wheel(120)
    d.wait('document.querySelector(".workspace").scrollTop !== '+str(initial_scroll),'enlarged canvas did not scroll locally')
    toolbar('#zoom-fit')
    toolbar('#browse-previous')
    wait_page('article_reading',first=1)

    d.click('#screen')
    d.call('Input.imeSetComposition',text='未',selectionStart=1,selectionEnd=1)
    wheel(120)
    d.call('Input.imeSetComposition',text='',selectionStart=0,selectionEnd=0)
    toolbar('#font-settings')
    d.wait('document.getElementById("font-dialog").open','settings dialog did not open during reading')
    wheel(120)
    d.click('#font-cancel')
    d.wait('!document.getElementById("font-dialog").open','settings dialog did not close during reading')
    toolbar('#browse-next')
    wait_page('article_reading',first=24)
    toolbar('#browse-next')
    wait_page('article_reading',first=47)
    if not d.evaluate('document.getElementById("browse-next").disabled'):
        raise ValueError('next page remained enabled at 100 percent')
    wheel(120)
    toolbar('#browse-back')
    wait_page('article_list',cursor=4)
    wheel(120);wait_page('article_list',cursor=5)
    wheel(-120);wait_page('article_list',cursor=4)
    key('ArrowRight',39);wait_page('unknown')
    if not d.evaluate('document.getElementById("browse-controls").hidden && document.getElementById("surface").dataset.navigationReady === "false"'):
        raise ValueError('confirmation prompt retained browsing controls')
    wheel(120);click_cell(4,5.5)
    key('ArrowRight',39)
    return {'status':'passed','number_click_and_title_selection':True,'capture_and_input_cancellation':True,
            'hover_preserves_canvas_pixels':True,'reading_buttons_and_wheel':True,
            'zoom_scroll_composition_modal_and_unknown_exclusions':True,
            'scope':'trusted WebView pointer/wheel and production loopback peer; no authenticated PTT'}
