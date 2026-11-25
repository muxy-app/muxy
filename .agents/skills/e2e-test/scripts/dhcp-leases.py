#!/usr/bin/env python3
"""Inspect and prune the host DHCP lease table that SwiftVM's shared network uses.

macOS's bootpd hands out guest addresses on the vmnet bridges (bridge100, ...)
and records every lease in /var/db/dhcpd_leases. It doesn't hand an expired
lease to a new MAC address, and every new e2e VM has a new MAC, so the /24
fills up after about 250 VMs. A full pool shows up later as a guest with no
IPv4 address and a build that can't resolve github.com.

  dhcp-leases.py check [needed]   Print each vmnet subnet's use; exit 1 if one has fewer than `needed` free
  dhcp-leases.py prune <out>      Write the lease table without expired leases to <out>
"""
import re
import subprocess
import sys
import time

LEASES = "/var/db/dhcpd_leases"
POOL_SIZE = 253  # .2 to .254; .1 is the host


def leases():
    try:
        text = open(LEASES).read()
    except FileNotFoundError:
        return []
    out = []
    for block in re.findall(r"\{.*?\}", text, re.S):
        fields = dict(
            line.strip().split("=", 1) for line in block.strip("{}").splitlines() if "=" in line
        )
        expires = int(fields.get("lease", "0x0"), 16)
        out.append((block, fields.get("ip_address", ""), expires))
    return out


def vmnet_subnets():
    """Subnets of the host's vmnet bridges (bridge100 and up), such as 192.168.234."""
    text = subprocess.run(["ifconfig"], capture_output=True, text=True).stdout
    subnets, bridge = [], None
    for line in text.splitlines():
        if not line.startswith(("\t", " ")):
            bridge = line.split(":", 1)[0]
        m = re.search(r"inet (\d+\.\d+\.\d+)\.1 ", line)
        if m and bridge and re.fullmatch(r"bridge1\d\d", bridge):
            subnets.append(m.group(1))
    return subnets


def check(needed):
    now = time.time()
    all_leases = leases()
    full = False
    subnets = vmnet_subnets()
    if not subnets:
        print("host DHCP pool: no vmnet bridge yet (it appears when a VM starts)")
        return 0
    for subnet in subnets:
        held = [l for l in all_leases if l[1].startswith(subnet + ".")]
        expired = sum(1 for l in held if l[2] < now)
        free = POOL_SIZE - len(held)
        print(f"host DHCP pool {subnet}.0/24: {len(held)} leases ({expired} expired), {free} free")
        full |= free < needed
    return 1 if full else 0


def prune(out):
    now = time.time()
    all_leases = leases()
    keep = [l[0] for l in all_leases if l[2] >= now]
    with open(out, "w") as f:
        f.write("".join(block + "\n" for block in keep))
    print(f"{len(all_leases)} leases, keeping {len(keep)} unexpired")
    return 0


if __name__ == "__main__":
    if len(sys.argv) >= 2 and sys.argv[1] == "check":
        sys.exit(check(int(sys.argv[2]) if len(sys.argv) > 2 else 3))
    if len(sys.argv) == 3 and sys.argv[1] == "prune":
        sys.exit(prune(sys.argv[2]))
    print(__doc__, file=sys.stderr)
    sys.exit(2)
