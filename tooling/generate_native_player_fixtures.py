"""Create small CC0 native-player fixtures. No downloaded media or fonts."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

WIDTH, HEIGHT, FPS, SECONDS = 320, 180, 24, 3


def frames(*, wrong_quadrant=False):
    top_left = (0, 255, 0) if wrong_quadrant else (255, 0, 0)
    for number in range(FPS * SECONDS):
        pixels = bytearray()
        for y in range(HEIGHT):
            left, right = (top_left, (0, 0, 255)) if y < HEIGHT // 2 else ((255, 255, 0), (0, 255, 255))
            pixels.extend(bytes(left) * (WIDTH // 2) + bytes(right) * (WIDTH // 2))
        x = (number * 4) % (WIDTH - 16)
        for y in range(HEIGHT - 32, HEIGHT - 16):
            at = (y * WIDTH + x) * 3
            pixels[at:at + 48] = bytes([255]) * 48
        yield pixels


def generate(directory, ffmpeg='ffmpeg', *, wrong_quadrant=False):
    if not shutil.which(ffmpeg):
        raise RuntimeError('Install FFmpeg before generating the owned fixtures')
    directory.mkdir(parents=True, exist_ok=False)
    codecs = {
        'mp4': ['-c:v', 'libx264', '-preset', 'fast', '-crf', '18', '-c:a', 'aac', '-b:a', '32k', '-movflags', '+faststart'],
        'webm': ['-c:v', 'libvpx-vp9', '-deadline', 'realtime', '-cpu-used', '8', '-crf', '35', '-b:v', '0', '-c:a', 'libopus', '-b:a', '32k'],
        'ogv': ['-c:v', 'libtheora', '-q:v', '5', '-c:a', 'libvorbis', '-q:a', '1'],
    }
    entries = []
    for extension, options in codecs.items():
        target = directory / ('quadrants.' + extension)
        command = [ffmpeg, '-hide_banner', '-loglevel', 'error', '-f', 'rawvideo', '-pixel_format', 'rgb24',
                   '-video_size', f'{WIDTH}x{HEIGHT}', '-framerate', str(FPS), '-i', 'pipe:0',
                   '-f', 'lavfi', '-i', f'sine=frequency=440:sample_rate=48000:duration={SECONDS}',
                   '-filter:a', 'volume=0.02', '-shortest', '-pix_fmt', 'yuv420p', *options, str(target)]
        process = subprocess.Popen(command, stdin=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            for frame in frames(wrong_quadrant=wrong_quadrant):
                process.stdin.write(frame)
            process.stdin.close()
            process.wait(timeout=30)
            if process.returncode:
                raise RuntimeError(process.stderr.read(8192).decode('utf-8', errors='replace'))
        except BrokenPipeError as error:
            process.wait(timeout=5)
            raise RuntimeError(process.stderr.read(8192).decode('utf-8', errors='replace')) from error
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            process.stderr.close()
        entries.append({'file': target.name, 'bytes': target.stat().st_size,
                        'sha256': hashlib.sha256(target.read_bytes()).hexdigest(), 'command': command})
    manifest = {'license': 'CC0-1.0', 'source': 'Self-authored pixels and generated sine wave; no external media.',
                'width': WIDTH, 'height': HEIGHT, 'fps': FPS, 'seconds': SECONDS,
                'expected_quadrants': ['green' if wrong_quadrant else 'red', 'blue', 'yellow', 'cyan'],
                'purpose': 'Deliberate GPU mismatch; the production probe still expects red' if wrong_quadrant else 'Positive native-player fixture',
                'motion': '16x16 white square along lower strip', 'audio_hz': 440, 'audio_gain': 0.02,
                'ffmpeg': subprocess.check_output([ffmpeg, '-version'], text=True).splitlines()[0], 'files': entries}
    (directory / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('new_directory', type=Path)
    parser.add_argument('--ffmpeg', default='ffmpeg', help='FFmpeg build with x264, VP9, Theora and Vorbis encoders')
    parser.add_argument('--wrong-quadrant', action='store_true',
                        help='Negative control: use green instead of red without changing the GPU verifier')
    args = parser.parse_args()
    generate(args.new_directory, args.ffmpeg, wrong_quadrant=args.wrong_quadrant)
