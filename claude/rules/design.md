# Design rules

- `design/*.png` is the reference for the desired look. Each PNG is rendered
  from an HTML mockup in `design/src/` by `design/render.sh` (headless Chromium).
  Edit the HTML, re-render, and commit both.
- Visual language: GNOME / libadwaita (header bar, boxed lists, rounded cards,
  accent colour for active elements). Follow the system light/dark preference.
  Mockups show the dark variant, which suits audio work.
- Information first: the mixer view always shows all 8 channels (meters, gain,
  mute, polarity, name, link state). Detailed editing happens in the channel
  editor.
- Audio conventions: dB values with sign and one decimal, frequencies in
  Hz/kHz, meters in dBu with clip indication, mute in red, active bypass in amber.
- Every control is keyboard accessible and has a text label or tooltip.
- Keep mockups in English.
