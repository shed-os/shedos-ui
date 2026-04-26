# shedos-screensaver audio reactivity

The screensaver captures audio via cpal, which on ShedOS routes
through the system's ALSA layer (pipewire emulates ALSA via
pipewire-alsa). Five styles consume audio when it's available:
matrix, plasma, starfield, tunnel, and waves. The other three
(logo-bounce, conway, mandala) ignore audio by design.

## Two source modes

`--audio-source mic` opens the **default input device** (microphone,
headset, line-in — whatever your audio configuration has selected).
This is the simplest path; if your microphone works in any other
app, this works.

`--audio-source desktop` looks for an input device whose name
contains `monitor` (the conventional pulse/pipewire loopback
naming, e.g.
`alsa_output.pci-0000_00_1f.3.analog-stereo.monitor`). This lets
the screensaver react to whatever's currently playing (Spotify,
mpv, browser tabs, the lot) without listening through the mic.

If no monitor source is found, `--audio-source desktop` falls back
to the default input.

## Setting up desktop loopback

On ShedOS the pipewire stack already exposes monitor sources for
every active sink. To verify:

```sh
pactl list sources short | grep monitor
```

If you see one or more `*.monitor` entries you're done.

If `pactl` is unavailable or no monitors are listed, the most likely
fixes are:

1. Install pipewire-pulse (already a default ShedOS dep).
2. Restart the user pipewire stack:
   ```sh
   systemctl --user restart pipewire pipewire-pulse wireplumber
   ```
3. Verify pipewire-alsa is enabled (it provides the ALSA pcm
   device that cpal reads through):
   ```sh
   ls /etc/alsa/conf.d/*pipewire*
   ```

## What each style does with audio

- **matrix**: a beat (bass band crosses 1.5× rolling average)
  triggers a 1-frame burst of new trail spawns — feels like rain
  "splashing" on the downbeat.
- **plasma**: bass band deforms the X-axis spatial frequency;
  treble bands deform the Y-axis frequency. The plasma's pattern
  literally breathes with the music.
- **starfield**: a beat doubles the warp factor for that frame —
  an FTL "kick" effect on the downbeat.
- **tunnel**: peak amplitude across all bands scales ring
  brightness up to 1.7× — quiet music = dim tunnel, loud = bright.
- **waves**: bass shrinks wavelength (denser waves); peak amplitude
  scales glyph brightness.

If audio capture fails (no input device, no permission, pipewire
down), the binary emits a single warning and runs without audio
reactivity — the same animations work, just driven by time alone.

## Known caveats

- Latency: cpal hands us the audio in chunks of typically
  ~10–20 ms. Combined with the 16 ms FFT analysis tick, beat
  detection lags the actual audio by ~30–40 ms. For visual
  reactivity this is imperceptible.
- The beat detector warms up over the first ~16 frames before it
  flags any beat at all. Cold-start beats would be spurious (the
  rolling average is zero on first hit).
- Beat detection is a simple bass-band energy threshold. Genres
  with prominent kick drums (electronic, hip-hop) trigger beats
  cleanly; ambient or classical music with no transients won't
  fire beats — but the band-magnitude reactivity in plasma /
  tunnel / waves still reflects the sound.
