"""Real WebView editing events through the Rust keyboard policy and loopback peer."""
import time
def run(d):
    surface='document.getElementById("surface")'
    last_key='none'
    def wait_mode(mode):
        d.wait(surface+'.dataset.keyboardMode === '+repr(mode),'Vim mode did not become '+mode)
    def key(value,ctrl=False):
        nonlocal last_key
        last_key=('Ctrl+' if ctrl else '')+value
        code='Key'+value.upper() if len(value)==1 and value.isalpha() else value
        vk=ord(value.upper()) if len(value)==1 else {'Enter':13,'Escape':27,'ArrowRight':39}[value]
        for kind in ('keyDown','keyUp'):
            args=dict(type=kind,key=value,code=code,windowsVirtualKeyCode=vk,modifiers=2 if ctrl else 0)
            if kind=='keyDown' and len(value)==1 and not ctrl: args['text']=value
            d.call('Input.dispatchKeyEvent',**args)
        # Local no-ops produce no station frame. Exercise a deliberate keyboard
        # cadence so the next key does not race the UI command owner's reply;
        # this probe does not claim to validate maximum-rate key bursts.
        time.sleep(.12)
    def toolbar(selector):
        d.call('Input.dispatchMouseEvent',type='mouseMoved',x=20,y=2);d.click(selector)
    def step(action):
        revision=d.evaluate(surface+'.dataset.revision')
        action()
        d.wait(surface+'.dataset.revision !== '+repr(revision),'Vim did not receive the expected loopback reply after '+last_key)
        d.wait(surface+'.dataset.navigationReady === "true"','Vim reply remained pending')
    d.wait(surface+'.dataset.navigation === "article_reading"','Vim synthetic reading page missing')
    toolbar('#vim-toggle');wait_mode('normal')
    key('g');key('Escape')  # Cancelled prefix must send nothing.
    key('f');wait_mode('once');step(lambda:key('h'));wait_mode('normal')
    toolbar('#vim-once');wait_mode('once');step(lambda:key('d',ctrl=True));wait_mode('normal')
    key('3');step(lambda:key('j'))
    step(lambda:key('u',ctrl=True))
    key('g');step(lambda:key('g'))
    step(lambda:key('G'))
    # Local no-op at the last line; no server bytes are expected.
    key('j');d.wait(surface+'.dataset.navigationReady === "true"','last-line no-op did not finish')
    key('l');d.wait(surface+'.dataset.navigationReady === "true"','forward no-op did not finish')
    step(lambda:key('h'))
    step(lambda:key('l'))
    key('f');wait_mode('once')
    revision=d.evaluate(surface+'.dataset.revision')
    d.call('Input.imeSetComposition',text='中',selectionStart=1,selectionEnd=1)
    d.call('Input.insertText',text='中')
    d.wait(surface+'.dataset.revision !== '+repr(revision),'native-once composition was not sent exactly once')
    wait_mode('normal')
    step(lambda:key('b',ctrl=True))
    key('i');wait_mode('input');key('Escape');wait_mode('normal')
    toolbar('#vim-toggle');wait_mode('input');key('ArrowRight')
    return {'status':'passed','native_once_key_control_and_ime':True,
            'count_lines_half_page_home_end_and_hierarchy':True,
            'cancel_prefix_and_native_input_return':True,
            'scope':'trusted WebView events at a deliberate 120ms key cadence and exact loopback bytes; maximum-rate bursts and native macOS IME unverified'}
