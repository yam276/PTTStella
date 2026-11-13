# Winit 0.30.13

This directory comes from the published [winit 0.30.13 crate](https://crates.io/crates/winit/0.30.13), commit `e9809ef54b18499bb4f2cac945719ecc2a61061b`. The crate archive SHA-256 is `a6755fa58a9f8350bd1e472d4c3fcc25f824ec358933bba33306d0b63df5978d`. See [LICENSE](LICENSE).

The macOS event-loop builder adds `with_passive_windows`, which defaults to false. Opting in skips Winit's application activation on launch and its explicit window-focus request. Window creation and visibility changes use AppKit's `orderBack`. AppKit also sees these windows as ineligible to become key or main, and they ignore mouse events. PTTStella opts in only for automated toolkit verification. The manual IME probes and ordinary application use Winit's regular window behavior.

On Windows, the current monitor reports an unknown refresh rate when `DEVMODE.dmDisplayFrequency` is 0 or 1. Both values indicate the display driver's default refresh rate. Slint's redraw timer can then use its existing 60 Hz fallback. [Microsoft DEVMODE documentation](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-devmodea)
