"""In-app settings dialog on the owned Windows WebView2 probe."""
import base64
import json
from pathlib import Path


class Settings:
    def __init__(self, main):
        self.main = main
        self.target = main.open_settings()

    def page(self, name):
        self.target.click('[data-settings-category="'+name+'"]')
        self.target.wait('!document.querySelector(\'[data-settings-page="'+name+'"]\').hidden',
                         'settings category did not open')

    def edit(self, selector, value):
        d = self.target
        d.click(selector)
        d.shortcut('a')
        d.call('Input.insertText', text=value)
        d.wait('document.querySelector('+json.dumps(selector)+').value === '+json.dumps(value),
               'settings field did not receive the complete edit')

    def saved(self):
        self.target.wait('document.getElementById("font-dialog").dataset.saveState === "saved"',
                         'settings autosave did not finish')

    def close(self):
        self.target.click('#font-cancel')
        self.target.wait('!document.getElementById("font-dialog").open', 'settings did not flush and close')
        self.main.call('Page.bringToFront')
        self.main.close_tools()


def run(main, output):
    output = Path(output)
    path = output/'probe-settings.json'
    initial = main.evaluate('document.getElementById("screen").toDataURL("image/png")')
    s = Settings(main)
    d = s.target
    for query in ('opacity', '透明度'):
        s.edit('#settings-search', query)
        d.wait('document.querySelectorAll("#settings-results button").length === 1',
               'bilingual settings search did not resolve opacity')
        d.click('#settings-results button')
        d.wait('document.activeElement.id === "tools-opacity"', 'search result did not focus the setting')
    s.page('appearance')
    s.edit('#font-size', '18')
    s.edit('#font-baseline', '19')
    s.edit('#font-ascii', 'Courier New')
    s.saved()
    main.wait('document.getElementById("surface").dataset.fontSize === "18" && document.getElementById("surface").dataset.baseline === "19"',
              'autosaved font metrics did not reach the main renderer')
    pixels = main.evaluate('document.getElementById("screen").toDataURL("image/png")')
    if pixels == initial:
        raise ValueError('autosaved font metrics did not affect actual pixels')
    (output/'settings-after.png').write_bytes(base64.b64decode(pixels.split(',', 1)[1], validate=True))
    saved_bytes = path.read_bytes()
    saved = json.loads(saved_bytes)
    if (saved['display']['font_size'] != 18 or saved['display']['baseline'] != 19
            or saved['display']['fonts']['ascii_font'] != 'Courier New'):
        raise ValueError('autosave did not persist the actual configuration')
    s.close()
    s = Settings(main)
    d = s.target
    s.page('appearance')
    if d.evaluate('document.getElementById("font-size").value') != '18':
        raise ValueError('reopening settings lost the saved value')
    s.edit('#font-size', '100')
    d.wait('document.getElementById("font-dialog").dataset.saveState === "invalid"',
           'invalid partial edit was not retained for correction')
    if path.read_bytes() != saved_bytes or main.evaluate('document.getElementById("surface").dataset.fontSize') != '18':
        raise ValueError('invalid edit changed the stored or effective configuration')
    d.click('#font-defaults')
    s.saved()
    if json.loads(path.read_bytes())['display']['fonts'] != {'kind': 'platform_default'}:
        raise ValueError('appearance defaults were not autosaved')
    s.edit('#font-size', '18')
    s.edit('#font-baseline', '19')
    s.edit('#font-ascii', 'Courier New')
    s.saved()
    import desktop_profiles
    profile_report = desktop_profiles.run(main, output, s)
    return ({'status': 'passed', 'same_webview_modal': True, 'bilingual_search': True,
             'autosave_actual_file_and_pixels': True, 'invalid_edit_preserves_effective_values': True,
             'reopen_and_category_defaults': True,
             'scope': 'trusted owned Windows WebView2 controls; native picker and other OS excluded'}, profile_report)
