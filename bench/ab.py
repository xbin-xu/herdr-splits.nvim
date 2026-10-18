#!/usr/bin/env python3
"""A/B benchmark: Python helper vs the Rust binary, same decision tree.

Alternating sampling cancels machine drift. Both variants run the full decision
path with the socket stubbed out, so the measurement is process start-up plus
the work this project actually owns:

* Python: `python -S scripts/herdr-nav.py left`, transport monkey-patched onto
  a canned responder (see `harness.py` in herdr-splits-slim for the same trick);
* Rust: `scripts/bin/herdr-splits nav left --replay <fixture>`, the binary's own
  built-in replay transport.

Neither run touches the real socket, so the numbers are comparable and repeatable
(and this works on a machine where herdr runs elevated).

Usage:
  python bench/ab.py [rounds] [--scripts DIR] [--bin PATH]
"""

import argparse
import os
import statistics
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))

# A single-scripted-response fixture: pane.get says "nvim pane", so the run
# forwards a chord. That is the shortest realistic decision path and it does not
# depend on which pane happens to be focused.
FIXTURE = "\n".join(
    [
        '{"method":"pane.get","response":{"result":{"pane":{"pane_id":"A",'
        '"tokens":{"editor":"nvim"},"terminal_title_stripped":"f.rs - NVIM"}}}}',
        '{"method":"pane.send_keys","response":{"result":{}}}',
    ]
) + "\n"

HARNESS = r'''
import os, sys
scripts = sys.argv[1]
sys.path.insert(0, scripts)
import herdr_common as h

RESPONSES = {
    "pane.get": {"result": {"pane": {"pane_id": "A", "tokens": {"editor": "nvim"},
                                    "terminal_title_stripped": "f.rs - NVIM"}}},
    "pane.process_info": {"result": {"process_info": {"foreground_processes": []}}},
    "pane.send_keys": {"result": {}},
    "pane.focus_direction": {"result": {}},
    "pane.edges": {"result": {"edges": {"pane_id": "A", "left": False, "right": False,
                                        "up": False, "down": False,
                                        "layout": {"zoomed": False, "panes": []}}}},
    "pane.layout": {"result": {"layout": {"zoomed": False, "panes": []}}},
    "pane.current": {"result": {"pane": {"pane_id": "A"}}},
    "pane.zoom": {"result": {}},
    "pane.focus": {"result": {}},
}


def fake_rpc(sock_path, method, params, req_id="req"):
    return RESPONSES.get(method, {"result": {}})


h.rpc_call = fake_rpc
h.discover_socket_path = lambda: "stub"
os.environ["HERDR_PANE_ID"] = "A"
os.environ["HERDR_SPLITS_CONFIG"] = os.path.join(scripts, "bench-stub.conf")

path = os.path.join(scripts, sys.argv[2])
with open(path, "rb") as fh:
    code = compile(fh.read(), path, "exec")
mod = type(sys)(sys.argv[2])
mod.__file__ = path
mod.__dict__["__name__"] = sys.argv[2]
sys.modules[sys.argv[2]] = mod
exec(code, mod.__dict__)
for name in ("rpc_call", "discover_socket_path"):
    setattr(mod, name, getattr(h, name))
sys.argv = [sys.argv[2], sys.argv[3]]
sys.exit(mod.main())
'''


def percentile(values, q):
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, int(len(ordered) * q))]


def time_runs(make_cmd, rounds, env):
    # One warm-up run so first-touch page faults do not skew the minimum.
    subprocess.run(make_cmd(), env=env, stdout=subprocess.DEVNULL,
                   stderr=subprocess.DEVNULL)
    samples = []
    for _ in range(rounds):
        started = time.perf_counter()
        subprocess.run(make_cmd(), env=env, stdout=subprocess.DEVNULL,
                       stderr=subprocess.DEVNULL)
        samples.append((time.perf_counter() - started) * 1000.0)
    return samples


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("rounds", nargs="?", type=int, default=60)
    parser.add_argument("--scripts", default=os.environ.get("HERDR_SPLITS_SCRIPTS", ""))
    parser.add_argument("--bin", default=os.environ.get("HERDR_SPLITS_BIN", ""))
    parser.add_argument("--python", default=sys.executable)
    args = parser.parse_args()

    if not args.scripts:
        parser.error("set --scripts or HERDR_SPLITS_SCRIPTS to the plugin's scripts/ dir")
    if not args.bin:
        parser.error("set --bin or HERDR_SPLITS_BIN to scripts/bin/herdr-splits")

    harness = os.path.join(HERE, "harness.py")
    with open(harness, "w", encoding="utf-8") as fh:
        fh.write(HARNESS)
    fixture = os.path.join(HERE, "nav-nvim.jsonl")
    with open(fixture, "w", encoding="utf-8") as fh:
        fh.write(FIXTURE)

    env = dict(os.environ)
    for name in ("HERDR_SOCKET_PATH", "HERDR_SOCKET", "HERDR_PANE_ID"):
        env.pop(name, None)

    py_cmd = lambda: [args.python, "-S", harness, args.scripts, "herdr-nav.py", "left"]
    rs_cmd = lambda: [args.bin, "nav", "left", "--replay", fixture]

    old = time_runs(py_cmd, args.rounds, env)
    new = time_runs(rs_cmd, args.rounds, env)
    # Interleave a second pass to cancel drift, then merge.
    old += time_runs(py_cmd, args.rounds, env)
    new += time_runs(rs_cmd, args.rounds, env)

    print("%-10s %8s %8s %8s %8s" % ("variant", "min", "P25", "median", "P90"))
    for label, values in (("python", old), ("rust", new)):
        print("%-10s %8.1f %8.1f %8.1f %8.1f" % (
            label,
            percentile(values, 0.0),
            percentile(values, 0.25),
            statistics.median(values),
            percentile(values, 0.9),
        ))
    saved = percentile(old, 0.0) - percentile(new, 0.0)
    print("delta      %8.1f ms faster at the minimum (%.0f%%)" % (
        saved, saved / percentile(old, 0.0) * 100.0))
    return 0


if __name__ == "__main__":
    sys.exit(main())
