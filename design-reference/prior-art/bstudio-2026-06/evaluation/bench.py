"""Run after cargo build --offline --release. Sequential runs are full relayouts."""
from pathlib import Path
import platform
import resource
import subprocess

root = Path(__file__).resolve().parent
with (root / 'bench.log').open('w') as log:
    print(platform.platform(), 'release defaults opt=3 lto=false strip=false; '
          'Plex only, no fallback/caret assembly', file=log, flush=True)
    for count in (10000, 100000):
        for mode in ([], ['sequential']):
            subprocess.run([str(root / 'target/release/bstudio-prior-art-eval'),
                            'bench', str(count), *mode],
                           stdout=log, stderr=log, check=True)
            print('cumulative_child_maxrss_bytes=',
                  resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
                  file=log, flush=True)
