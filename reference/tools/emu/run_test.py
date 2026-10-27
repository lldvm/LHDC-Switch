"""End-to-end A2DP test of A2DPWB against a virtual sink - no Bluetooth hardware.

    python tools/emu/setup_env.py          (once)
    python tools/emu/run_test.py [--codecs sbc aac ...] [--duration 5]

For each codec: starts emu_sink.py (Bumble virtual link + A2DP sink), runs
a2dpwb-core.exe --cli with a test tone over H4/TCP, then checks both HCI captures
with a2dpwb_decode:
  - A2DPWB streams the requested codec and a2dpwb_decode finds no problem,
    on the sent side and on the received side
  - the sink received every media packet / frame A2DPWB sent
  - decoded audio (all codecs but LDAC and LHDC V5, which have no open-source
    decoder) is identical on both sides and is the test tone: 1 kHz left,
    1.5 kHz right, amplitude 0.5
The cases of NEGOTIATION run the same way with another sink or command line
(Auto choosing SBC XQ, or SBC on a sink without dual channel or one that
refuses it; Opus and Opus 05 at 48 kHz
from a 44.1 kHz request, Auto choosing Opus over AAC and LHDC V5 over Opus,
but never Opus 05).

Outputs (captures, logs, reports, WAV) go to tools/emu/out/<codec>/.
Exit code 0 when every codec passed. Uses only the Python standard library;
the sink runs in the tools/emu/.venv environment.

    python tools/emu/run_test.py --le-audio [case ...]

runs the experimental LE Audio cases instead (le_audio_tests/: stereo,
two-ase, earbuds, mono-16k and the profiles' own): emu_le_audio_sink.py with
the case's layout, a2dpwb-core.exe --cli --le-audio with the test tone, and
a2dpwb_decode --le-audio on both sides:
  - one LC3 stream of the expected configuration on both sides, consecutive
    ISO SDU sequence numbers, no decoder problems
  - the sinks received every SDU A2DPWB sent; decoded audio identical on both
    sides and the test tone (1 kHz left, 1.5 kHz right; mono: both) in every
    100 ms of every stream but a short gap at its start and end
  - no SDU reached the emulated controller too late for a real one (its log
    says "late SDU"), no underrun in A2DPWB's counters, audio (not counting
    inserted silence) for most of the duration
  - the checks of le_audio_tests/<profile>.py (core.py: each CIS's channel
    allocation within the PACS locations of the sink it goes to)
Outputs go to tools/emu/out/le-<case>/.
"""

import argparse
import math
import os
import queue
import re
import shutil
import struct
import subprocess
import sys
import threading
import time
import wave

import le_audio_tests

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
VENV_PYTHON = os.path.join(HERE, '.venv', 'Scripts', 'python.exe')
if not os.path.exists(VENV_PYTHON) and sys.prefix != sys.base_prefix:
    # No .venv here (e.g. a git worktree): run the sink with the virtual
    # environment this script runs in (one with bumble installed)
    VENV_PYTHON = sys.executable
SINK_ADDRESS = '00:A2:D0:00:00:02'
BASE_PORT = 9101  # A2DP cases; LE Audio cases use BASE_PORT + 100

CODECS = ['sbc', 'sbcxq', 'aac', 'aptx', 'aptxhd', 'aptxll', 'ldac', 'lhdcv5', 'faststream', 'opus', 'opus05']
CONFIG_PATTERN = {  # a2dpwb_decode's "Configuration :" line
    'sbc': r'SBC \d+ Hz, joint stereo, 16 blocks, 8 subbands, loudness, bitpool 2-53$',
    # SBC XQ: dual channel up to bitpool 47 (44.1 kHz) / 43 (48 kHz)
    'sbcxq': r'SBC (44100 Hz, dual channel, .*bitpool 2-47|48000 Hz, dual channel, .*bitpool 2-43)$',
    # Sinks with a low max bitpool: the configuration (and every frame,
    # a2dpwb_decode checks) stays within it
    'sbc-bp35': r'SBC \d+ Hz, joint stereo, 16 blocks, 8 subbands, loudness, bitpool 2-35$',
    'sbcxq-bp45': r'SBC 44100 Hz, dual channel, 16 blocks, 8 subbands, loudness, bitpool 2-45$',
    'aac': r'AAC ', 'aptx': r'aptX \d', 'aptxhd': r'aptX HD \d',
    'aptxll': r'aptX LL \d', 'ldac': r'LDAC \d', 'lhdcv5': r'LHDC V5 \d',
    'faststream': r'FastStream \d+ Hz, direction 0x01 \(music\)',   # no voice backchannel
    'opus': r'Opus 48000 Hz, stereo, 20 ms',
    # PipeWire's choice: 10 ms, one coupled stream, 312 x 1024 bps, no return channel
    'opus05': r'Opus 05 48000 Hz, stereo, 10 ms frames, location 0x00000003, max 312 x 1024 bps, no return channel$',
}
NO_DECODER = {'ldac', 'lhdcv5'}  # frame headers are validated, audio is not decoded
# Further cases: name -> (A2DPWB -c, emu_sink.py arguments, the codec expected,
# more A2DPWB arguments)
NEGOTIATION = {
    'sbcxq-44k': ('sbcxq', [], 'sbcxq', ['--rate', '44100']),            # bitpool 47
    'auto-sbcxq': ('auto', ['--codecs', 'sbc'], 'sbcxq', []),            # Auto takes SBC XQ over SBC
    'auto-sbc-no-xq': ('auto', ['--codecs', 'sbc', '--sbc-no-dual'], 'sbc', []),  # no dual channel: SBC
    # Dual channel listed but refused: SBC XQ falls back to SBC
    'auto-sbcxq-refused': ('auto', ['--codecs', 'sbc', '--sbc-reject-dual'], 'sbc', []),
    # Max bitpool 35: SBC capped there; Auto does not take SBC XQ (needs 43)
    'auto-sbc-bp35': ('auto', ['--codecs', 'sbc', '--sbc-max-bitpool', '35'], 'sbc-bp35', []),
    'sbcxq-bp45': ('sbcxq', ['--sbc-max-bitpool', '45'], 'sbcxq-bp45', ['--rate', '44100']),
    'faststream-44k': ('faststream', [], 'faststream', ['--rate', '44100']),
    # Auto never takes FastStream, even over SBC
    'auto-not-faststream': ('auto', ['--codecs', 'sbc', 'faststream', '--sbc-no-dual'], 'sbc', []),
    # Opus: 48 kHz whatever the capture rate asked for; Auto takes it over
    # AAC and SBC, and LHDC V5 over it
    'opus-from-44k': ('opus', [], 'opus', ['--rate', '44100']),
    'auto-opus': ('auto', ['--codecs', 'sbc', 'aac', 'opus'], 'opus', []),
    'auto-lhdcv5-over-opus': ('auto', ['--codecs', 'sbc', 'aac', 'opus', 'lhdcv5'], 'lhdcv5', []),
    # Opus 05: 48 kHz from a 44.1 kHz request; Auto never takes it (AAC here)
    'opus05-from-44k': ('opus05', [], 'opus05', ['--rate', '44100']),
    'auto-not-opus05': ('auto', ['--codecs', 'sbc', 'aac', 'opus05'], 'aac', []),
}
TONES = ((1000.0, 0.5), (1500.0, 0.5))  # (frequency, amplitude) per channel, as --test-tone


def parse_report(text):
    """First stream of an a2dpwb_decode report."""
    def find(pattern, cast=str):
        m = re.search(pattern, text, re.M)
        return cast(m.group(1)) if m else None
    return {
        'config': find(r'Configuration : (.*)$'),
        'packets': find(r'Media packets : (\d+)', int),
        'frames': find(r'Frames\s+: (\d+)', int),
        'audio_s': find(r'Audio\s+: ([\d.]+) s', float),
        'wav': find(r'WAV\s+: (.*)$'),
        'streams': len(re.findall(r'^=== Stream ', text, re.M)),
        'result_ok': re.search(r'^RESULT: OK', text, re.M) is not None,
        'problems': re.findall(r'^    - (.*)$', text, re.M),
    }


def read_wav(path):
    with wave.open(path) as w:
        ch, width, rate, n = w.getnchannels(), w.getsampwidth(), w.getframerate(), w.getnframes()
        raw = w.readframes(n)
    full = float(1 << (8 * width - 1))
    samples = [[] for _ in range(ch)]
    if width == 2:
        values = struct.unpack(f'<{n * ch}h', raw)
        for c in range(ch):
            samples[c] = [v / full for v in values[c::ch]]
        return rate, samples
    for i in range(n * ch):
        b = raw[i * width:(i + 1) * width]
        if width == 3:
            b += b'\xff' if b[2] & 0x80 else b'\x00'
        samples[i % ch].append(struct.unpack('<i', b)[0] / full)
    return rate, samples


def tone_block(xs, rate, expected):
    """(ok, notes) for one block of samples: each expected (frequency,
    amplitude) there within 0.05, and > 95% of the energy in them"""
    count = len(xs)
    mean_sq = sum(x * x for x in xs) / count
    energy, ok, notes = 0.0, True, []
    for freq, amp in expected:
        k = 2 * math.cos(2 * math.pi * freq / rate)
        s1 = s2 = 0.0
        for x in xs:
            s1, s2 = x + k * s1 - s2, s1
        tone_amp = 2 * math.sqrt(max(s1 * s1 + s2 * s2 - k * s1 * s2, 0.0)) / count
        energy += tone_amp * tone_amp / 2
        ok &= abs(tone_amp - amp) < 0.05
        notes.append(f'{freq:.0f} Hz amp {tone_amp:.3f}')
    share = energy / mean_sq if mean_sq else 0.0
    ok &= share > 0.95
    notes[-1] += f' ({100 * share:.1f}%)'
    return ok, notes


def tone_check(path, tones=None):
    """Returns (ok, description) for the test tone in the middle of the WAV.
    tones: per channel the (frequency, amplitude) pairs expected there
    (default: TONES, one per channel)."""
    tones = tones or tuple((t,) for t in TONES)
    rate, channels = read_wav(path)
    if len(channels) != 2 or len(channels[0]) < rate:
        return False, f'expected >= 1 s of stereo audio, got {len(channels)} ch'
    start, count = len(channels[0]) // 2 - rate // 4, rate // 2
    notes, ok = [], True
    for xs, expected in zip(channels, tones):
        block_ok, block_notes = tone_block(xs[start:start + count], rate, expected)
        ok &= block_ok
        notes += block_notes
    return ok, ', '.join(notes)


# LE Audio: the tone is checked in blocks of TONE_BLOCK_S over the whole
# stream. Only the start and the end may lack it: A2DPWB sends encoded
# silence until its jitter buffer holds a few SDU sets (5 x 10 ms) and LC3
# needs a frame or two to settle; at the end A2DPWB stops the test tone
# 100 ms before the stream (its buffer runs empty: silence). Up to
# MAX_START_GAP_S / MAX_END_GAP_S are excluded there. Any other block without
# the tone (an underrun's silence, a lost or doubled SDU) fails.
TONE_BLOCK_S = 0.1    # whole periods of 1 kHz and 1.5 kHz
MAX_START_GAP_S = 0.3
MAX_END_GAP_S = 0.3
# A2DPWB's underrun counter: the one when the test tone stops before the
# stream (above) is not counted, i.e. underruns in the last END_UNDERRUN_S of
# the last stream
END_UNDERRUN_S = 0.3


def tone_check_stream(path, tones):
    """(ok, description) for the test tone over the whole WAV of a stream,
    except gaps of up to MAX_START_GAP_S at the start and MAX_END_GAP_S at
    the end"""
    rate, channels = read_wav(path)
    n = int(rate * TONE_BLOCK_S)
    if len(channels) != 2 or len(channels[0]) < rate:
        return False, f'expected >= 1 s of stereo audio, got {len(channels)} ch, {len(channels[0]) / rate:.2f} s'
    blocks = len(channels[0]) // n
    good, bad, notes = [], [], None
    for b in range(blocks):
        ok, block_notes = True, []
        for xs, expected in zip(channels, tones):
            block_ok, ch_notes = tone_block(xs[b * n:(b + 1) * n], rate, expected)
            ok &= block_ok
            block_notes += ch_notes
        if ok:
            good.append(b)
            if notes is None and b >= blocks // 2:
                notes = ', '.join(block_notes)   # a typical block, for the summary
        else:
            bad.append((b, ', '.join(block_notes)))
    if not good:
        return False, f'no {TONE_BLOCK_S * 1000:.0f} ms block with the tone in {blocks * TONE_BLOCK_S:.1f} s'
    start_gap, end_gap = good[0] * TONE_BLOCK_S, (blocks - 1 - good[-1]) * TONE_BLOCK_S
    bad = [x for x in bad if good[0] < x[0] < good[-1]]
    desc = (f'{notes or "?"}; {blocks * TONE_BLOCK_S:.1f} s checked, gaps {start_gap:.1f} s at the start, '
            f'{end_gap:.1f} s at the end')
    problems = []
    if start_gap > MAX_START_GAP_S + 1e-9:
        problems.append(f'tone only from {start_gap:.1f} s (at most {MAX_START_GAP_S} s expected)')
    if end_gap > MAX_END_GAP_S + 1e-9:
        problems.append(f'no tone in the last {end_gap:.1f} s (at most {MAX_END_GAP_S} s expected)')
    if bad:
        b, block_notes = bad[0]
        problems.append(f'{len(bad)} block(s) without the tone after it started, first at '
                        f'{b * TONE_BLOCK_S:.1f} s: {block_notes}')
    return not problems, '; '.join(problems + [desc])


def start_sink(port, capture, log_path, script='emu_sink.py', extra=()):
    """Starts a virtual sink; its READY line is in proc.ready_line."""
    log = open(log_path, 'w', encoding='utf-8')
    # The sink writes UTF-8 whatever the system locale (a Japanese Windows
    # would write cp932, and a non-ASCII log line, e.g. MCP's simulated
    # track title, would arrive with U+FFFD)
    env = dict(os.environ, PYTHONIOENCODING='utf-8', PYTHONUTF8='1')
    proc = subprocess.Popen(
        [VENV_PYTHON, '-u', os.path.join(HERE, script), '--port', str(port), '--capture', capture]
        + list(extra),
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding='utf-8', errors='replace', env=env)
    lines = queue.Queue()

    def pump():
        for line in proc.stdout:
            log.write(line)
            log.flush()
            lines.put(line)
        log.close()
    threading.Thread(target=pump, daemon=True).start()
    deadline = time.time() + 30
    while time.time() < deadline:
        try:
            line = lines.get(timeout=0.5)
            if line.startswith('READY'):
                proc.ready_line = line.strip()
                return proc
        except queue.Empty:
            if proc.poll() is not None:
                break
    proc.kill()
    raise RuntimeError(f'virtual sink did not start, see {log_path}')


def run_codec(args, codec, port):
    out = os.path.join(args.out, codec)
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    src_capture = os.path.join(out, 'a2dpwb.pklg')
    sink_capture = os.path.join(out, 'sink.pklg')
    failures = []

    cli_codec, sink_args, expected, a2dpwb_args = NEGOTIATION.get(codec, (codec, [], codec, []))
    sink = start_sink(port, sink_capture, os.path.join(out, 'sink.log'), extra=sink_args)
    try:
        env = dict(os.environ, A2DPWB_CONFIG_DIR=os.path.join(out, 'config'))  # isolated link keys
        with open(os.path.join(out, 'a2dpwb.log'), 'w', encoding='utf-8', errors='replace') as log:
            try:
                rc = subprocess.run(
                    [args.a2dpwb, '--cli', '--hci-tcp', f'127.0.0.1:{port}', '-d', SINK_ADDRESS,
                     '-c', cli_codec, '--test-tone', '--duration', str(args.duration),
                     '--hci-capture', src_capture] + a2dpwb_args
                    + (['--max-packet', str(args.max_packet)] if args.max_packet else []),
                    stdout=log, stderr=subprocess.STDOUT, env=env, timeout=args.duration + 90).returncode
            except subprocess.TimeoutExpired:
                rc = 'timeout'
        if rc != 0:
            failures.append(f'A2DPWB exited with {rc} (see a2dpwb.log)')
    finally:
        sink.terminate()
        sink.wait(10)

    reports = {}
    for side, capture, extra in (('sent', src_capture, []), ('received', sink_capture, ['--received'])):
        if not os.path.exists(capture):
            failures.append(f'{side}: no capture')
            continue
        text = subprocess.run([args.decoder, capture, '-o', os.path.join(out, side)] + extra,
                              capture_output=True, text=True, encoding='utf-8', errors='replace').stdout
        with open(os.path.join(out, f'{side}_report.txt'), 'w', encoding='utf-8') as f:
            f.write(text)
        r = reports[side] = parse_report(text)
        if r['streams'] != 1:
            failures.append(f'{side}: {r["streams"]} streams (expected 1)')
        if not re.match(CONFIG_PATTERN[expected], r['config'] or ''):
            failures.append(f'{side}: configuration "{r["config"]}"')
        if not r['result_ok']:
            failures.extend(f'{side}: {p}' for p in r['problems'] or ['a2dpwb_decode reported problems'])

    sent, received = reports.get('sent'), reports.get('received')
    detail = ''
    if sent and received:
        if (sent['packets'], sent['frames']) != (received['packets'], received['frames']):
            failures.append(f'sent {sent["packets"]} packets / {sent["frames"]} frames, '
                            f'received {received["packets"]} / {received["frames"]}')
        if (sent['audio_s'] or 0) < 0.9 * args.duration:
            failures.append(f'only {sent["audio_s"]} s of audio for --duration {args.duration}')
        detail = f'{sent["packets"]} packets, {sent["frames"]} frames, {sent["audio_s"]} s'
        if expected not in NO_DECODER:
            if not (sent['wav'] and received['wav']):
                failures.append('no decoded WAV')
            else:
                with open(sent['wav'], 'rb') as a, open(received['wav'], 'rb') as b:
                    if a.read() != b.read():
                        failures.append('decoded audio differs between sent and received side')
                ok, notes = tone_check(received['wav'])
                detail += f' | {notes}'
                if not ok:
                    failures.append(f'test tone not found: {notes}')
    return failures, detail


def parse_le_report(text):
    """a2dpwb_decode --le-audio report: as parse_report (configuration and WAV
    of the first stream), packets / frames / audio summed over all streams,
    plus the CIS lines"""
    r = parse_report(text)
    r['cis'] = re.findall(r'^  CIS 0x[0-9A-F]+ +: (.*)$', text, re.M)
    r['wavs'] = re.findall(r'WAV\s+: (.*)$', text, re.M)   # one per stream
    r['text'] = text
    r['packets'] = sum(int(x) for x in re.findall(r'Media packets : (\d+)', text))
    r['frames'] = sum(int(x) for x in re.findall(r'Frames\s+: (\d+)', text))
    r['audio_s'] = round(sum(float(x) for x in re.findall(r'Audio\s+: ([\d.]+) s', text)), 3)
    return r


def run_le_case(args, case, port):
    out = os.path.join(args.out, 'le-' + case.name)
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    src_capture = os.path.join(out, 'a2dpwb.pklg')
    sink_capture = os.path.join(out, 'sink.pklg')
    failures = []

    sink = start_sink(port, sink_capture, os.path.join(out, 'sink.log'), 'emu_le_audio_sink.py',
                      ['--layout', case.layout] + case.sink_args)
    addresses = sink.ready_line.split('sinks=')[1].split(',')
    try:
        env = dict(os.environ, A2DPWB_CONFIG_DIR=os.path.join(out, 'config'))  # isolated bonds
        cmd = [args.a2dpwb, '--cli', '--hci-tcp', f'127.0.0.1:{port}', '--le-audio', '-d', addresses[0]]
        for a in ([] if case.first_address_only else addresses[1:]):
            cmd += ['--le-device', a]
        duration = case.duration or args.duration
        cmd += ['--test-tone', '--duration', str(duration), '--hci-capture', src_capture] + case.a2dpwb_args
        with open(os.path.join(out, 'a2dpwb.log'), 'w', encoding='utf-8', errors='replace') as log:
            try:
                rc = subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT, env=env,
                                    timeout=duration + 120).returncode
            except subprocess.TimeoutExpired:
                rc = 'timeout'
        if case.expect_error is None and rc != 0:
            failures.append(f'A2DPWB exited with {rc} (see a2dpwb.log)')
    finally:
        sink.terminate()
        sink.wait(10)

    if case.expect_error is not None:
        # A2DPWB must refuse with a clear error, not stream
        log_text = read_text(os.path.join(out, 'a2dpwb.log'))
        if rc == 0 or rc == 'timeout':
            failures.append(f'A2DPWB exited with {rc}, expected an error exit')
        if not re.search(case.expect_error, log_text):
            failures.append(f'no error matching "{case.expect_error}" in a2dpwb.log')
        if 'Streaming LE Audio:' in log_text:
            failures.append('A2DPWB streamed although it should have refused')
        return failures, f'exit {rc}'

    sink_captures =[sink_capture] + [os.path.join(out, f'sink-{i + 1}.pklg') for i in range(1, len(addresses))]
    reports = {}
    for side, captures, extra in (('sent', [src_capture], []), ('received', sink_captures, ['--received'])):
        missing = [c for c in captures if not os.path.exists(c)]
        if missing:
            failures.append(f'{side}: no capture {missing}')
            continue
        text = subprocess.run([args.decoder, '--le-audio'] + captures + ['-o', os.path.join(out, side)] + extra,
                              capture_output=True, text=True, encoding='utf-8', errors='replace').stdout
        with open(os.path.join(out, f'{side}_report.txt'), 'w', encoding='utf-8') as f:
            f.write(text)
        r = reports[side] = parse_le_report(text)
        if r['streams'] != case.streams:
            failures.append(f'{side}: {r["streams"]} streams (expected {case.streams})')
        if not re.match(case.config, r['config'] or ''):
            failures.append(f'{side}: configuration "{r["config"]}"')
        if not r['result_ok']:
            failures.extend(f'{side}: {p}' for p in r['problems'] or ['a2dpwb_decode reported problems'])

    sent, received = reports.get('sent'), reports.get('received')
    a2dpwb_log = read_text(os.path.join(out, 'a2dpwb.log'))
    detail = ''
    if sent and received:
        missing = (sent['packets'] or 0) - (received['packets'] or 0)
        if (not case.lossy and (sent['packets'], sent['frames']) != (received['packets'], received['frames'])) \
                or (case.lossy and not 0 <= missing <= 8 * case.streams):
            failures.append(f'sent {sent["packets"]} SDUs / {sent["frames"]} frames, '
                            f'received {received["packets"]} / {received["frames"]}')
        detail = f'{sent["config"]}: {sent["packets"]} SDUs, {sent["frames"]} LC3 frames, {sent["audio_s"]} s'
        if len(sent['wavs']) != len(received['wavs']) or not received['wavs']:
            failures.append(f'decoded WAVs: {len(sent["wavs"])} sent, {len(received["wavs"])} received')
        else:
            if not case.lossy:
                for a_path, b_path in zip(sent['wavs'], received['wavs']):
                    with open(a_path, 'rb') as a, open(b_path, 'rb') as b:
                        if a.read() != b.read():
                            failures.append('decoded audio differs between sent and received side')
            if case.tones:
                # Every stream (the one set up again after a link loss too)
                for index, path in enumerate(received['wavs']):
                    ok, notes = tone_check_stream(path, case.tones)
                    label = f'stream {index + 1}: ' if len(received['wavs']) > 1 else ''
                    detail += f' | {label}{notes}'
                    if not ok:
                        failures.append(f'{label}test tone: {notes}')

    # SDUs that reached the emulated controller too late for a real one
    # (le_audio/controller.py logs them, its own stalls not counted)
    late = re.findall(r'late SDU: CIS (0x[0-9A-F]+) seq \d+ reached the controller ([\d.]+) ms .*?(\d+) late SDU',
                      read_text(os.path.join(out, 'sink.log')))
    if late:
        per_cis = {}
        for h, ms, count in late:
            n, worst = per_cis.get(h, (0, 0.0))
            per_cis[h] = (max(n, int(count)), max(worst, float(ms)))
        for h, (n, worst) in sorted(per_cis.items()):
            failures.append(f'CIS {h}: {n} SDU(s) reached the controller after their SDU interval began '
                            f'(worst {worst} ms; the queue of a real controller would have run empty)')

    # A2DPWB's own counters, per stream set-up ("stopped sending", the
    # counters start anew with each): no underrun (silence inserted while
    # streaming), and the audio sent (not counting the silence while its
    # buffer fills) must cover the duration
    frame_s = (7.5 if sent and re.search(r' 7\.5 ms ', sent['config'] or '') else 10) / 1000
    stops = le_stream_stats(a2dpwb_log, frame_s)
    if not stops:
        failures.append('no "LE Audio: stopped sending" statistics in a2dpwb.log')
    else:
        sets, silence, underruns, at_end = (sum(s[i] for s in stops) for i in range(4))
        detail += f' | {silence} silence sets, {underruns} underruns ({at_end} as the tone stopped)'
        if underruns - at_end > case.max_underruns:
            failures.append(f'{underruns - at_end} underrun(s) (A2DPWB sent silence while streaming; '
                            f'at most {case.max_underruns} allowed)')
        if sent:
            audio_s = (sets - silence) * frame_s
            min_audio = case.min_audio if case.min_audio is not None else 0.6 if case.lossy else 0.9
            if audio_s < min_audio * duration:
                failures.append(f'only {audio_s:.2f} s of audio (without silence) for --duration {duration}')

    ctx = dict(out=out, a2dpwb_log=a2dpwb_log, sink_log=read_text(os.path.join(out, 'sink.log')),
               sent=sent, received=received, sink_addresses=addresses)
    failures.extend(le_audio_tests.check(case, ctx))
    return failures, detail


def le_stream_stats(log, frame_s):
    """Per stream set-up of A2DPWB's log ("sending ISO SDUs" ... "stopped
    sending: N SDU sets ..., S silence, U underruns"): (N, S, U, E), E = the
    underruns of the last stream within END_UNDERRUN_S of its end (as its
    "no audio for SDU set K" warnings say; the test tone stops first)"""
    streams, warned = [], []
    for line in log.splitlines():
        m = re.search(r'LE Audio: no audio for SDU set (\d+)', line)
        if m:
            warned.append(int(m.group(1)))
        m = re.search(r'LE Audio: stopped sending: (\d+) SDU sets .*?(\d+) silence, (\d+) underruns', line)
        if m:
            streams.append([int(m.group(1)), int(m.group(2)), int(m.group(3)), 0, warned])
            warned = []
    if streams:
        last = streams[-1]
        last[3] = sum(1 for k in last[4] if k >= last[0] - END_UNDERRUN_S / frame_s)
    return [tuple(s[:4]) for s in streams]


def read_text(path):
    try:
        with open(path, encoding='utf-8', errors='replace') as f:
            return f.read()
    except OSError:
        return ''


def main_le(args):
    all_cases = le_audio_tests.cases()
    names = [c.name for c in all_cases]
    wanted = args.le_audio or names
    unknown = [n for n in wanted if n not in names]
    if unknown:
        sys.exit(f'Unknown LE Audio case(s) {unknown}; known: {names}')
    results = []
    for index, case in enumerate(c for c in all_cases if c.name in wanted):
        print(f'--- LE Audio {case.name} ...', flush=True)
        failures, detail = run_le_case(args, case, args.base_port + 100 + index)
        results.append((case.name, failures))
        print(f'    {"PASS" if not failures else "FAIL"}  {detail}')
        for failure in failures:
            print(f'      - {failure}')
    passed = sum(1 for _, f in results if not f)
    print(f'\n{passed}/{len(results)} LE Audio cases passed. Details: {args.out}')
    sys.exit(0 if passed == len(results) else 1)


def main():
    parser = argparse.ArgumentParser(description='End-to-end A2DP test of A2DPWB against a virtual sink')
    parser.add_argument('--codecs', nargs='+', choices=CODECS + list(NEGOTIATION),
                        default=CODECS + list(NEGOTIATION),
                        help='codecs and negotiation cases (%(default)s)')
    parser.add_argument('--le-audio', nargs='*', metavar='CASE',
                        help='run the LE Audio (experimental) cases instead (default: all of them)')
    parser.add_argument('--duration', type=int, default=5, help='seconds of streaming per codec')
    parser.add_argument('--max-packet', type=int, help='passed to A2DPWB --max-packet (default: its own)')
    parser.add_argument('--build-dir', default=os.path.join(REPO, 'build'),
                        help="the decoder tool's CMake builds (cmake -S tools/a2dp_decode -B build/a2dp_decode)")
    parser.add_argument('--config', default='Release', help='build configuration (Release / Debug)')
    parser.add_argument('--a2dpwb', help='A2DPWB core executable to test (default: target/release/a2dpwb-core.exe)')
    parser.add_argument('--decoder', help='a2dpwb_decode executable (default: <build-dir>/a2dp_decode/<config>)')
    parser.add_argument('--out', default=os.path.join(HERE, 'out'))
    parser.add_argument('--base-port', type=int, default=BASE_PORT,
                        help='first TCP port (LE Audio cases use +100); change it to run several tests at once')
    args = parser.parse_args()
    args.a2dpwb = args.a2dpwb or os.path.join(REPO, 'target', args.config.lower(), 'a2dpwb-core.exe')
    args.decoder = args.decoder or os.path.join(args.build_dir, 'a2dp_decode', args.config, 'a2dpwb_decode.exe')
    for path, hint in ((VENV_PYTHON, 'run: python tools/emu/setup_env.py'),
                       (args.a2dpwb, 'build A2DPWB'), (args.decoder, 'build a2dpwb_decode')):
        if not os.path.exists(path):
            sys.exit(f'Missing {path} ({hint})')
    if args.le_audio is not None:
        main_le(args)

    results = []
    for index, codec in enumerate(args.codecs):
        print(f'--- {codec} ...', flush=True)
        failures, detail = run_codec(args, codec, args.base_port + index)
        results.append((codec, failures))
        print(f'    {"PASS" if not failures else "FAIL"}  {detail}')
        for failure in failures:
            print(f'      - {failure}')

    passed = sum(1 for _, f in results if not f)
    print(f'\n{passed}/{len(results)} codecs passed. Details: {args.out}')
    sys.exit(0 if passed == len(results) else 1)


if __name__ == '__main__':
    main()
