#!/usr/bin/env python3
"""Run ONE isolated workload; getrusage avoids macOS time -l's denied sysctl.
Usage: python3 scripts/measure.py /path/to/release/varos-text-spike 10000
RSS is process peak (includes engine, fonts, allocations and result retention),
not a heap/cache measurement. Run each workload in a fresh Python process.
"""
import platform
import resource
import subprocess
import sys

result = subprocess.run([sys.argv[1], "bench", sys.argv[2]], check=False)
usage = resource.getrusage(resource.RUSAGE_CHILDREN)
scale = 1 if platform.system() == "Darwin" else 1024
print(f"peak_rss_bytes={usage.ru_maxrss * scale}")
print(f"child_user_seconds={usage.ru_utime:.6f}; child_system_seconds={usage.ru_stime:.6f}")
sys.exit(result.returncode)
