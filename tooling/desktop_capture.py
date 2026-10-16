"""Selected-row PNGs through trusted WebView input and an independent decoder."""
import json
from pathlib import Path
import subprocess


def run(d, output):
    if output is None:
        raise ValueError('row capture requires isolated output artifacts')
    output = Path(output)

    def toolbar(selector):
        d.call('Input.dispatchMouseEvent', type='mouseMoved', x=20, y=2)
        d.click(selector)

    def point(col, row):
        return d.evaluate('(()=>{const b=document.getElementById("screen").getBoundingClientRect();return {x:b.left+b.width*'+str(col)+'/80,y:b.top+b.height*'+str(row)+'/24};})()')

    def mouse(kind, col, row):
        d.call('Input.dispatchMouseEvent', type=kind, **point(col, row), button='left', clickCount=1,
               **({'buttons': 1} if kind == 'mouseMoved' else {}))

    def inspect(start, end):
        before = d.evaluate('({pixels:document.getElementById("screen").toDataURL(),left:document.querySelector(".workspace").scrollLeft,top:document.querySelector(".workspace").scrollTop})')
        toolbar('#export-rows')
        d.wait('document.getElementById("surface").dataset.exportResult === "saved"', 'selected rows PNG did not save')
        after = d.evaluate('({pixels:document.getElementById("screen").toDataURL(),left:document.querySelector(".workspace").scrollLeft,top:document.querySelector(".workspace").scrollTop})')
        if before != after:
            raise ValueError('row export changed visible pixels or scrolling')
        result = subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
                                 '-File', str(Path(__file__).with_name('inspect_png.ps1')),
                                 '-PngFilePath', str(output/'export-rows.png'),
                                 '-ReferenceFilePath', str(output/'export.png'),
                                 '-StartRow', str(start), '-EndRow', str(end)],
                                capture_output=True, timeout=5,
                                creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
        if result.returncode:
            raise ValueError('selected rows could not be independently decoded: '+result.stderr.decode(errors='replace'))
        if json.loads(result.stdout) != {'width': 960, 'height': (end-start)*24, 'region_matches_reference': True}:
            raise ValueError('selected rows did not exactly match the complete ANSI renderer')
        (output/f'capture-lines-{start+1}-{end}.png').write_bytes((output/'export-rows.png').read_bytes())

    # The existing right-half selection must export the whole dual-colour row.
    d.wait('!document.getElementById("export-rows").disabled', 'row export not enabled for selection')
    inspect(3, 4)
    toolbar('#zoom-fit')
    # Reverse range includes graphics and the last blank row, without trimming.
    for start, end in [(23, 19), (0, 3)]:
        mouse('mousePressed', 5.5, start+.5)
        mouse('mouseMoved', 4.5, end+.5)
        mouse('mouseReleased', 4.5, end+.5)
        first, last = min(start, end), max(start, end)+1
        d.wait('!!document.getElementById("surface").dataset.selectionReady && document.getElementById("export-rows").title.includes('+repr(f'第 {first+1}–{last} 行')+')',
               'selected row endpoints did not become ready')
        if start == 0:
            toolbar('#zoom-in')
        inspect(first, last)
        toolbar('#zoom-fit')
    toolbar('#select-all')
    d.wait('document.querySelectorAll(".selection-row").length === 24', 'all rows not selected for capture')
    inspect(0, 24)
    # A normal click clears the capability; no selection cannot become a crop.
    mouse('mousePressed', 60, 10.5)
    mouse('mouseReleased', 60, 10.5)
    d.wait('document.getElementById("export-rows").disabled', 'clear selection left row export enabled')
    return {'status': 'passed', 'single_reverse_first_last_and_all_rows': True,
            'independent_pixel_equality': True, 'zoom_scroll_and_visible_pixels_preserved': True,
            'scope': 'owned Windows WebView fixture; native save picker excluded'}
