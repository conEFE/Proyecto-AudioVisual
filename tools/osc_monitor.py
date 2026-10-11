#!/usr/bin/env python3
"""Muestra los mensajes OSC que llegan (para comprobar que todo funciona sin TouchDesigner).

  python tools/osc_monitor.py            # escucha en el puerto 7000
  python tools/osc_monitor.py --filter drums
"""

import argparse
import sys
import time

from pythonosc.dispatcher import Dispatcher
from pythonosc.osc_server import BlockingOSCUDPServer


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--host", default="127.0.0.1")
    p.add_argument("--port", type=int, default=7000)
    p.add_argument("--filter", default="", help="sólo direcciones que contengan este texto")
    args = p.parse_args()

    latest = {}
    last_print = [0.0]

    def handler(addr, *values):
        if args.filter in addr:
            latest[addr] = values[0] if values else None
        now = time.monotonic()
        if now - last_print[0] > 0.25 and latest:
            last_print[0] = now
            sys.stdout.write("\033[H\033[J")
            for a in sorted(latest):
                sys.stdout.write(f"{a:<28} {latest[a]:10.3f}\n")
            sys.stdout.flush()

    d = Dispatcher()
    d.set_default_handler(handler)
    print(f"Escuchando OSC en {args.host}:{args.port} ...")
    BlockingOSCUDPServer((args.host, args.port), d).serve_forever()


if __name__ == "__main__":
    main()
