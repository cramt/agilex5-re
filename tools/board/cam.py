# Read a DE25-Nano verdict off the webcam (README, "Self-tests"): the whole
# LED row is one signal, so a verdict is one region's brightness over time.
#   cam.py record <out.mkv> [seconds]         10 s MJPEG clip, 1920x1080 @ 30
#   cam.py stdmap <clip.mkv> <out.pgm>        per-pixel brightness std (half
#                                             resolution), prints the hottest blobs
#   cam.py trace <clip.mkv> <x0> <y0> <x1> <y1> [--lit L --dark D]
#                                             the box's mean brightness (full-res
#                                             coordinates) -> BLINK <Hz>, LIT or DARK
# Run with the camera on manual exposure (tools/board/selftest.sh sets it).
# Paths to ffmpeg/ffprobe come from FFMPEG (default: ffmpeg on PATH).
import os, subprocess, sys
import numpy as np

FF = os.environ.get('FFMPEG', 'ffmpeg')
FP = os.path.join(os.path.dirname(FF), 'ffprobe') if os.sep in FF else 'ffprobe'
W, H = 1920, 1080


def record(out, secs=10):
    subprocess.run([FF, '-loglevel', 'error', '-y', '-f', 'v4l2', '-input_format', 'mjpeg', '-framerate', '30',
                    '-video_size', f'{W}x{H}', '-i', '/dev/video0', '-t', str(secs), '-c', 'copy', out], check=True)


def frames(clip, crop=None, scale=1):
    """Gray frames (n, h, w) uint8 and their timestamps in seconds."""
    vf = []
    w, h = W, H
    if crop:
        x0, y0, x1, y1 = crop
        w, h = x1 - x0, y1 - y0
        vf.append(f'crop={w}:{h}:{x0}:{y0}')
    if scale != 1:
        w, h = w // scale, h // scale
        vf.append(f'scale={w}:{h}')
    cmd = [FF, '-loglevel', 'error', '-i', clip] + (['-vf', ','.join(vf)] if vf else []) + \
          ['-fps_mode', 'passthrough', '-f', 'rawvideo', '-pix_fmt', 'gray', '-']
    raw = subprocess.run(cmd, capture_output=True, check=True).stdout
    a = np.frombuffer(raw, np.uint8).reshape(-1, h, w)
    pts = subprocess.run([FP, '-loglevel', 'error', '-select_streams', 'v:0', '-show_entries', 'frame=pts_time',
                          '-of', 'csv=p=0', clip], capture_output=True, text=True, check=True).stdout.split()
    t = np.array([float(p) for p in pts if p not in ('', 'N/A')])
    n = min(len(a), len(t))
    return a[:n], t[:n] - t[0]


def stdmap(clip, out):
    a, t = frames(clip, scale=2)
    s = a.astype(np.float32).std(axis=0)
    m = a.astype(np.float32).mean(axis=0)
    with open(out, 'wb') as f:
        f.write(b'P5 %d %d 255\n' % (s.shape[1], s.shape[0]))
        f.write(np.clip(s * (255 / max(s.max(), 1)), 0, 255).astype(np.uint8).tobytes())
    print(f'{len(a)} frames over {t[-1]:.2f} s; std max {s.max():.1f}')
    # Hottest 16x16 (half-res) cells, merged greedily so a blob reports once.
    g = 16
    hs, ws = s.shape[0] // g, s.shape[1] // g
    cell = s[:hs * g, :ws * g].reshape(hs, g, ws, g).mean(axis=(1, 3))
    seen = []
    for k in np.argsort(cell, axis=None)[::-1][:60]:
        cy, cx = divmod(int(k), ws)
        if cell[cy, cx] < 2: break
        if any(abs(cy - y) <= 2 and abs(cx - x) <= 2 for y, x in seen): continue
        seen.append((cy, cx))
        x, y = cx * g * 2, cy * g * 2
        print(f'blob at full-res x {x}..{x + 2 * g} y {y}..{y + 2 * g}: std {cell[cy, cx]:.1f}, mean {m[cy * g:(cy + 1) * g, cx * g:(cx + 1) * g].mean():.0f}')
        if len(seen) >= 8: break


def lockin(clip, hz, out):
    """Per-pixel amplitude at one frequency: finds an LED of known rate among
    status and activity LEDs that also flicker."""
    a, t = frames(clip, scale=2)
    x = a.astype(np.float32) - a.mean(axis=0, dtype=np.float32)
    ph = np.exp(-2j * np.pi * hz * t).astype(np.complex64)
    amp = np.abs(np.tensordot(ph, x, axes=(0, 0))) * 2 / len(t)
    tot = x.std(axis=0) + 1e-3
    with open(out, 'wb') as f:
        f.write(b'P5 %d %d 255\n' % (amp.shape[1], amp.shape[0]))
        f.write(np.clip(amp * (255 / max(amp.max(), 1)), 0, 255).astype(np.uint8).tobytes())
    for k in np.argsort(amp, axis=None)[::-1][:400:40]:
        y, x0 = divmod(int(k), amp.shape[1])
        print(f'full-res ({2 * x0}, {2 * y}): amplitude {amp[y, x0]:.1f} of std {tot[y, x0]:.1f}')


def trace(clip, box, lit=None, dark=None):
    a, t = frames(clip, crop=box)
    y = a.reshape(len(a), -1).astype(np.float64).mean(axis=1)
    lo, hi = np.percentile(y, 5), np.percentile(y, 95)
    mid = (lo + hi) / 2
    fps = (len(t) - 1) / t[-1]
    # A blink is a swing much larger than frame noise; count mid-level crossings
    # with hysteresis so MJPEG noise doesn't add edges.
    noise = np.median(np.abs(np.diff(y)))
    swing = hi - lo
    state, rises, ups = None, [], 0
    band = swing / 4
    for k, v in enumerate(y):
        s = 1 if v > mid + band else 0 if v < mid - band else state
        if state == 0 and s == 1: rises.append(t[k])
        if s == 1: ups += 1
        state = s
    blink = swing > max(8 * noise, 6) and len(rises) >= 2
    print(f'{len(y)} frames, {fps:.1f} fps; mean {y.mean():.1f} p5 {lo:.1f} p95 {hi:.1f} swing {swing:.1f} frame noise {noise:.2f}')
    if blink:
        hz = (len(rises) - 1) / (rises[-1] - rises[0])
        print(f'BLINK {hz:.2f} Hz, duty {ups / len(y):.2f}, {len(rises)} rising edges')
    elif lit is not None and dark is not None:
        print(('LIT' if abs(y.mean() - lit) < abs(y.mean() - dark) else 'DARK') + f' (steady; lit {lit}, dark {dark})')
    else:
        print('STEADY (pass --lit/--dark to name the level)')


if __name__ == '__main__':
    cmd, *a = sys.argv[1:]
    if cmd == 'record': record(a[0], *(a[1:2]))
    elif cmd == 'stdmap': stdmap(*a)
    elif cmd == 'lockin': lockin(a[0], float(a[1]), a[2])
    elif cmd == 'trace':
        opt = {a[k][2:]: float(a[k + 1]) for k in range(5, len(a), 2)}
        trace(a[0], tuple(int(v) for v in a[1:5]), opt.get('lit'), opt.get('dark'))
    else: sys.exit(__doc__)
