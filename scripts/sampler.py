#!/usr/bin/env python3
"""MinkaMon system sampler.

Long-running process: emits one JSON object per line on stdout.

  every tick (1s):  cpu, mem, gpu, temps, net
  every 2 ticks:    procs (full process table)
  every 5 ticks:    conns (established TCP, geolocated offline)

GPU notes for this machine (and portability):
  - Intel xe: busyness from /proc/*/fdinfo drm-cycles-*/drm-total-cycles-*
    deltas summed across DRM clients (the nvtop method); frequency from
    /sys/class/drm/cardN/device/tile0/gt0/freq0/act_freq.
  - NVIDIA proprietary: nvidia-smi — but ONLY while the card's PCI
    runtime_status is "active". Polling nvidia-smi wakes a runtime-suspended
    Optimus dGPU and keeps it awake, wrecking battery; a suspended card is
    reported as {"asleep": true} instead.

IP geolocation is fully offline: bundled iptoasn ipv4 table (PDDL-1.0)
mapped to country centroids. Nothing leaves the machine.
"""

import bisect
import glob
import gzip
import json
import os
import socket
import struct
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ASSETS = os.path.join(HERE, "..", "assets")

TICK_SECONDS = 1.0
PROCS_EVERY = 2
CONNS_EVERY = 5

# Country centroids (approximate, for globe plotting): ISO 3166-1 alpha-2.
CENTROIDS = {
    "AD": (42.5, 1.5), "AE": (24.0, 54.0), "AF": (33.9, 67.7), "AG": (17.1, -61.8),
    "AL": (41.2, 20.2), "AM": (40.1, 45.0), "AO": (-11.2, 17.9), "AR": (-38.4, -63.6),
    "AT": (47.5, 14.6), "AU": (-25.3, 133.8), "AZ": (40.1, 47.6), "BA": (43.9, 17.7),
    "BB": (13.2, -59.5), "BD": (23.7, 90.4), "BE": (50.5, 4.5), "BF": (12.2, -1.6),
    "BG": (42.7, 25.5), "BH": (26.0, 50.5), "BI": (-3.4, 29.9), "BJ": (9.3, 2.3),
    "BN": (4.5, 114.7), "BO": (-16.3, -63.6), "BR": (-14.2, -51.9), "BS": (25.0, -77.4),
    "BT": (27.5, 90.4), "BW": (-22.3, 24.7), "BY": (53.7, 27.9), "BZ": (17.2, -88.5),
    "CA": (56.1, -106.3), "CD": (-4.0, 21.8), "CF": (6.6, 20.9), "CG": (-0.2, 15.8),
    "CH": (46.8, 8.2), "CI": (7.5, -5.5), "CL": (-35.7, -71.5), "CM": (7.4, 12.4),
    "CN": (35.9, 104.2), "CO": (4.6, -74.3), "CR": (9.7, -83.8), "CU": (21.5, -77.8),
    "CV": (16.0, -24.0), "CY": (35.1, 33.4), "CZ": (49.8, 15.5), "DE": (51.2, 10.5),
    "DJ": (11.8, 42.6), "DK": (56.3, 9.5), "DM": (15.4, -61.4), "DO": (18.7, -70.2),
    "DZ": (28.0, 1.7), "EC": (-1.8, -78.2), "EE": (58.6, 25.0), "EG": (26.8, 30.8),
    "ER": (15.2, 39.8), "ES": (40.5, -3.7), "ET": (9.1, 40.5), "FI": (61.9, 25.7),
    "FJ": (-17.7, 178.1), "FM": (7.4, 150.6), "FR": (46.2, 2.2), "GA": (-0.8, 11.6),
    "GB": (55.4, -3.4), "GD": (12.1, -61.7), "GE": (42.3, 43.4), "GH": (7.9, -1.0),
    "GM": (13.4, -15.3), "GN": (9.9, -9.7), "GQ": (1.7, 10.3), "GR": (39.1, 21.8),
    "GT": (15.8, -90.2), "GW": (11.8, -15.2), "GY": (4.9, -58.9), "HK": (22.3, 114.2),
    "HN": (15.2, -86.2), "HR": (45.1, 15.2), "HT": (18.97, -72.3), "HU": (47.2, 19.5),
    "ID": (-0.8, 113.9), "IE": (53.4, -8.2), "IL": (31.0, 34.9), "IN": (20.6, 79.0),
    "IQ": (33.2, 43.7), "IR": (32.4, 53.7), "IS": (64.96, -19.0), "IT": (41.9, 12.6),
    "JM": (18.1, -77.3), "JO": (30.6, 36.2), "JP": (36.2, 138.3), "KE": (-0.02, 37.9),
    "KG": (41.2, 74.8), "KH": (12.6, 105.0), "KI": (1.9, -157.4), "KM": (-11.6, 43.9),
    "KN": (17.4, -62.8), "KP": (40.3, 127.5), "KR": (35.9, 127.8), "KW": (29.3, 47.5),
    "KZ": (48.0, 66.9), "LA": (19.9, 102.5), "LB": (33.9, 35.9), "LC": (13.9, -61.0),
    "LI": (47.2, 9.6), "LK": (7.9, 80.8), "LR": (6.4, -9.4), "LS": (-29.6, 28.2),
    "LT": (55.2, 23.9), "LU": (49.8, 6.1), "LV": (56.9, 24.6), "LY": (26.3, 17.2),
    "MA": (31.8, -7.1), "MC": (43.75, 7.4), "MD": (47.4, 28.4), "ME": (42.7, 19.4),
    "MG": (-18.8, 46.9), "MH": (7.1, 171.2), "MK": (41.6, 21.7), "ML": (17.6, -4.0),
    "MM": (21.9, 95.96), "MN": (46.9, 103.8), "MO": (22.2, 113.5), "MR": (21.0, -10.9),
    "MT": (35.9, 14.4), "MU": (-20.3, 57.6), "MV": (3.2, 73.2), "MW": (-13.3, 34.3),
    "MX": (23.6, -102.6), "MY": (4.2, 101.98), "MZ": (-18.7, 35.5), "NA": (-22.96, 18.5),
    "NE": (17.6, 8.1), "NG": (9.1, 8.7), "NI": (12.9, -85.2), "NL": (52.1, 5.3),
    "NO": (60.5, 8.5), "NP": (28.4, 84.1), "NR": (-0.5, 166.9), "NZ": (-40.9, 174.9),
    "OM": (21.5, 55.9), "PA": (8.5, -80.8), "PE": (-9.2, -75.0), "PG": (-6.3, 143.96),
    "PH": (12.9, 121.8), "PK": (30.4, 69.3), "PL": (51.9, 19.1), "PS": (31.9, 35.2),
    "PT": (39.4, -8.2), "PW": (7.5, 134.6), "PY": (-23.4, -58.4), "QA": (25.4, 51.2),
    "RO": (45.9, 25.0), "RS": (44.0, 21.0), "RU": (61.5, 105.3), "RW": (-1.9, 29.9),
    "SA": (23.9, 45.1), "SB": (-9.6, 160.2), "SC": (-4.7, 55.5), "SD": (12.9, 30.2),
    "SE": (60.1, 18.6), "SG": (1.35, 103.8), "SI": (46.2, 15.0), "SK": (48.7, 19.7),
    "SL": (8.5, -11.8), "SM": (43.9, 12.5), "SN": (14.5, -14.5), "SO": (5.2, 46.2),
    "SR": (3.9, -56.0), "SS": (6.9, 31.3), "ST": (0.2, 6.6), "SV": (13.8, -88.9),
    "SY": (34.8, 39.0), "SZ": (-26.5, 31.5), "TD": (15.5, 18.7), "TG": (8.6, 0.8),
    "TH": (15.9, 101.0), "TJ": (38.9, 71.3), "TL": (-8.9, 125.7), "TM": (38.97, 59.6),
    "TN": (33.9, 9.5), "TO": (-21.2, -175.2), "TR": (39.0, 35.2), "TT": (10.7, -61.2),
    "TV": (-7.1, 177.6), "TW": (23.7, 121.0), "TZ": (-6.4, 34.9), "UA": (48.4, 31.2),
    "UG": (1.4, 32.3), "US": (37.1, -95.7), "UY": (-32.5, -55.8), "UZ": (41.4, 64.6),
    "VA": (41.9, 12.45), "VC": (12.98, -61.3), "VE": (6.4, -66.6), "VN": (14.1, 108.3),
    "VU": (-15.4, 166.96), "WS": (-13.8, -172.1), "YE": (15.6, 48.5), "ZA": (-30.6, 22.9),
    "ZM": (-13.1, 27.8), "ZW": (-19.0, 29.2),
}


def load_ip_table():
    starts, countries = [], []
    path = os.path.join(ASSETS, "ip2country.csv.gz")
    try:
        with gzip.open(path, "rt") as f:
            for line in f:
                start, country = line.rstrip("\n").split(",", 1)
                starts.append(int(start))
                countries.append(country)
    except OSError:
        pass
    return starts, countries


IP_STARTS, IP_COUNTRIES = load_ip_table()


def country_for_ip(ip: str):
    if not IP_STARTS:
        return None
    try:
        n = struct.unpack("!I", socket.inet_aton(ip))[0]
    except OSError:
        return None
    i = bisect.bisect_right(IP_STARTS, n) - 1
    if i < 0:
        return None
    country = IP_COUNTRIES[i]
    return None if country in ("-", "None", "") else country


# --- CPU ---

def read_cpu_times():
    out = {}
    with open("/proc/stat") as f:
        for line in f:
            if not line.startswith("cpu"):
                break
            parts = line.split()
            vals = [int(v) for v in parts[1:]]
            idle = vals[3] + (vals[4] if len(vals) > 4 else 0)
            out[parts[0]] = (sum(vals), idle)
    return out


def cpu_percentages(prev, cur):
    result = {"total": 0.0, "cores": []}
    cores = []
    for key, (total, idle) in cur.items():
        ptotal, pidle = prev.get(key, (total, idle))
        dt, di = total - ptotal, idle - pidle
        busy = 0.0 if dt <= 0 else max(0.0, min(100.0, 100.0 * (dt - di) / dt))
        if key == "cpu":
            result["total"] = round(busy, 1)
        else:
            cores.append((int(key[3:]), round(busy, 1)))
    cores.sort()
    result["cores"] = [v for _, v in cores]
    return result


# --- memory ---

def read_mem():
    fields = {}
    with open("/proc/meminfo") as f:
        for line in f:
            key, rest = line.split(":", 1)
            fields[key] = int(rest.split()[0])  # kB
    total = fields.get("MemTotal", 0)
    available = fields.get("MemAvailable", 0)
    swap_total = fields.get("SwapTotal", 0)
    swap_free = fields.get("SwapFree", 0)
    return {
        "totalKb": total,
        "usedKb": total - available,
        "availableKb": available,
        "cacheKb": fields.get("Cached", 0) + fields.get("Buffers", 0),
        "swapTotalKb": swap_total,
        "swapUsedKb": swap_total - swap_free,
    }


# --- Intel xe via fdinfo ---

def read_xe_clients():
    """client-id -> {engine-class: (cycles, total_cycles)}"""
    clients = {}
    for fdinfo in glob.iglob("/proc/[0-9]*/fdinfo/*"):
        try:
            with open(fdinfo) as f:
                text = f.read(4096)
        except OSError:
            continue
        if "drm-driver:\txe" not in text and "drm-driver: xe" not in text:
            continue
        client = None
        cycles, totals = {}, {}
        for line in text.splitlines():
            if line.startswith("drm-client-id:"):
                client = line.split(":", 1)[1].strip()
            elif line.startswith("drm-cycles-"):
                key, val = line.split(":", 1)
                cycles[key[len("drm-cycles-"):]] = int(val.strip())
            elif line.startswith("drm-total-cycles-"):
                key, val = line.split(":", 1)
                totals[key[len("drm-total-cycles-"):]] = int(val.strip())
        if client is None:
            continue
        entry = clients.setdefault(client, {})
        for engine, cyc in cycles.items():
            entry[engine] = (cyc, totals.get(engine, 0))
    return clients


def xe_busy(prev, cur):
    """Per-engine busy%: sum client cycle deltas / total-cycle delta."""
    engines = {}
    for client, cur_engines in cur.items():
        prev_engines = prev.get(client, {})
        for engine, (cyc, total) in cur_engines.items():
            pcyc, ptotal = prev_engines.get(engine, (cyc, total))
            dcyc, dtotal = cyc - pcyc, total - ptotal
            if dtotal <= 0:
                continue
            busy, seen = engines.get(engine, (0, 0))
            engines[engine] = (busy + dcyc, max(seen, dtotal))
    return {
        engine: round(min(100.0, 100.0 * dcyc / dtotal), 1)
        for engine, (dcyc, dtotal) in engines.items()
        if dtotal > 0
    }


def xe_freq():
    for path in glob.glob("/sys/class/drm/card*/device/tile0/gt*/freq0/act_freq"):
        try:
            with open(path) as f:
                return int(f.read().strip())
        except (OSError, ValueError):
            continue
    return None


# --- NVIDIA (Optimus-safe) ---

def find_nvidia_pci():
    for dev in glob.glob("/sys/bus/pci/devices/*/vendor"):
        try:
            with open(dev) as f:
                if f.read().strip() != "0x10de":
                    continue
            base = os.path.dirname(dev)
            with open(os.path.join(base, "class")) as f:
                if not f.read().startswith("0x03"):
                    continue
            return base
        except OSError:
            continue
    return None


NVIDIA_PCI = find_nvidia_pci()


def read_nvidia():
    if NVIDIA_PCI is None:
        return None
    try:
        with open(os.path.join(NVIDIA_PCI, "power", "runtime_status")) as f:
            status = f.read().strip()
    except OSError:
        status = "unknown"
    if status == "suspended":
        return {"asleep": True}
    try:
        out = subprocess.run(
            ["nvidia-smi",
             "--query-gpu=utilization.gpu,temperature.gpu,memory.used,memory.total,power.draw",
             "--format=csv,noheader,nounits"],
            capture_output=True, text=True, timeout=3,
        )
        if out.returncode != 0:
            return None
        util, temp, mem_used, mem_total, power = [
            v.strip() for v in out.stdout.strip().split(",")]
        return {
            "asleep": False,
            "utilPct": float(util),
            "tempC": float(temp),
            "memUsedMb": float(mem_used),
            "memTotalMb": float(mem_total),
            "powerW": None if "N/A" in power else float(power),
        }
    except (subprocess.TimeoutExpired, OSError, ValueError):
        return None


# --- temperatures ---

def read_temps():
    temps = []
    for hwmon in sorted(glob.glob("/sys/class/hwmon/hwmon*")):
        try:
            with open(os.path.join(hwmon, "name")) as f:
                chip = f.read().strip()
        except OSError:
            continue
        for input_path in sorted(glob.glob(os.path.join(hwmon, "temp*_input"))):
            try:
                with open(input_path) as f:
                    milli = int(f.read().strip())
            except (OSError, ValueError):
                continue
            label_path = input_path.replace("_input", "_label")
            label = ""
            try:
                with open(label_path) as f:
                    label = f.read().strip()
            except OSError:
                pass
            sensor = os.path.basename(input_path)[: -len("_input")]
            temps.append({
                "chip": chip,
                "label": label or sensor,
                "c": round(milli / 1000.0, 1),
            })
    return temps


# --- network ---

def read_net():
    ifaces = {}
    with open("/proc/net/dev") as f:
        for line in f.readlines()[2:]:
            name, rest = line.split(":", 1)
            name = name.strip()
            if name == "lo":
                continue
            vals = rest.split()
            ifaces[name] = (int(vals[0]), int(vals[8]))  # rx, tx bytes
    return ifaces


def net_rates(prev, cur, dt):
    per_iface = {}
    rx_total = tx_total = 0
    for name, (rx, tx) in cur.items():
        prx, ptx = prev.get(name, (rx, tx))
        drx, dtx = max(0, rx - prx) / dt, max(0, tx - ptx) / dt
        if drx > 0 or dtx > 0 or name.startswith(("wl", "en", "eth")):
            per_iface[name] = {"downBps": round(drx), "upBps": round(dtx)}
        rx_total += drx
        tx_total += dtx
    return {"downBps": round(rx_total), "upBps": round(tx_total), "ifaces": per_iface}


# --- connections ---

def hex_to_ipv4(h):
    return socket.inet_ntoa(struct.pack("<I", int(h, 16)))


def read_connections():
    seen = {}
    try:
        with open("/proc/net/tcp") as f:
            for line in f.readlines()[1:]:
                parts = line.split()
                if parts[3] != "01":  # ESTABLISHED
                    continue
                ip_hex, port_hex = parts[2].split(":")
                ip = hex_to_ipv4(ip_hex)
                first = int(ip.split(".", 1)[0])
                if ip.startswith(("127.", "10.", "192.168.", "169.254.")) or \
                        (first == 172 and 16 <= int(ip.split(".")[1]) <= 31) or first == 100:
                    continue
                key = ip
                if key not in seen:
                    seen[key] = {"ip": ip, "port": int(port_hex, 16)}
    except OSError:
        pass
    conns = []
    for entry in list(seen.values())[:64]:
        country = country_for_ip(entry["ip"])
        entry["country"] = country
        if country in CENTROIDS:
            entry["lat"], entry["lon"] = CENTROIDS[country]
        conns.append(entry)
    return conns


# --- processes ---

def read_procs():
    procs = {}
    for stat_path in glob.iglob("/proc/[0-9]*/stat"):
        try:
            with open(stat_path) as f:
                data = f.read()
        except OSError:
            continue
        pid_str, rest = data.split(" ", 1)
        lparen, rparen = rest.find("("), rest.rfind(")")
        comm = rest[lparen + 1:rparen]
        fields = rest[rparen + 2:].split()
        # fields[11]=utime fields[12]=stime (0-based after comm/state removal:
        # state is fields[0], utime is fields[11], stime fields[12])
        try:
            cpu_ticks = int(fields[11]) + int(fields[12])
            rss_pages = int(fields[21])
            state = fields[0]
        except (IndexError, ValueError):
            continue
        procs[int(pid_str)] = {
            "comm": comm,
            "state": state,
            "ticks": cpu_ticks,
            "rssKb": rss_pages * PAGE_KB,
        }
    return procs


PAGE_KB = os.sysconf("SC_PAGE_SIZE") // 1024
CLOCK_TICKS = os.sysconf("SC_CLK_TCK")
NUM_CPUS = os.cpu_count() or 1


def proc_deltas(prev, cur, dt):
    out = []
    for pid, info in cur.items():
        pticks = prev.get(pid, {}).get("ticks", info["ticks"])
        cpu_pct = 100.0 * (info["ticks"] - pticks) / CLOCK_TICKS / dt
        out.append({
            "pid": pid,
            "comm": info["comm"],
            "state": info["state"],
            "cpuPct": round(min(cpu_pct, 100.0 * NUM_CPUS), 1),
            "rssKb": info["rssKb"],
        })
    out.sort(key=lambda p: p["cpuPct"], reverse=True)
    return out


def main():
    prev_cpu = read_cpu_times()
    prev_xe = read_xe_clients()
    prev_net = read_net()
    prev_procs = read_procs()
    prev_time = time.monotonic()
    prev_procs_time = prev_time
    nvidia = read_nvidia()

    print(json.dumps({
        "meta": {
            "cores": len(prev_cpu) - 1,
            "hasNvidia": NVIDIA_PCI is not None,
            "geoRanges": len(IP_STARTS),
        }
    }), flush=True)

    tick = 0
    while True:
        time.sleep(TICK_SECONDS)
        tick += 1
        now = time.monotonic()
        dt = max(now - prev_time, 1e-3)

        cur_cpu = read_cpu_times()
        cur_xe = read_xe_clients()
        cur_net = read_net()

        if tick % 2 == 0:
            nvidia = read_nvidia()

        sample = {
            "cpu": cpu_percentages(prev_cpu, cur_cpu),
            "mem": read_mem(),
            "gpu": {
                "xe": {"engines": xe_busy(prev_xe, cur_xe), "freqMhz": xe_freq()},
                "nvidia": nvidia,
            },
            "temps": read_temps(),
            "net": net_rates(prev_net, cur_net, dt),
        }
        if tick % PROCS_EVERY == 0:
            cur_procs = read_procs()
            sample["procs"] = proc_deltas(
                prev_procs, 
                cur_procs,
                max(now - prev_procs_time, 1e-3)
                )
            prev_procs = cur_procs
            prev_procs_time = now
        if tick % CONNS_EVERY == 1 or tick == 1:
            sample["conns"] = read_connections()

        print(json.dumps(sample), flush=True)
        prev_cpu, prev_xe, prev_net, prev_time = cur_cpu, cur_xe, cur_net, now


if __name__ == "__main__":
    try:
        main()
    except (KeyboardInterrupt, BrokenPipeError):
        pass