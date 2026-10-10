# Worst-case stack depth of the SmallTV Ultra firmware (`make ultra-stack`).
#
# The Arduino loop runs on a 4 KB stack carved out of the SDK's system stack
# (see platformio.ini), shared by the web server, the HTTP client and the
# Rust core, and interrupts push their frames on top. Run this after changing
# anything on those paths and keep the deepest path well under ~3 KB.
#
# Input: `objdump -d` (code) and `objdump -s` (literal pools) of firmware.elf.
# A function's frame is its prologue's SP decrement (call0 ABI). Edges are
# direct call0s and l32r-loaded addresses that the function calls with callx0.
# Not followed: dynamic dispatch (vtables, the page sink callback into the
# web server: add ~800 B for lwIP there). Recursion counts once. Crash paths
# (panic, abort, restart) are leaves: the device is restarting anyway.
#
#   python3 stack_depth.py [--max BYTES] code.txt literals.txt ROOT...
#
# With --max, exits 1 when any root's deepest path is larger (CI uses it).

import re, sys
args = sys.argv[1:]
limit = None
if args[:1] == ["--max"]:
    limit, args = int(args[1]), args[2:]
dis, hexdump = args[0], args[1]
roots = args[2:]
sys.setrecursionlimit(1000000)

words = {}
for line in open(hexdump):
    m = re.match(r"^ ([0-9a-f]{8}) ((?:[0-9a-f]{8} ?){1,4})", line)
    if not m: continue
    base = int(m.group(1), 16)
    for i, w in enumerate(m.group(2).split()):
        words[base + 4 * i] = int.from_bytes(bytes.fromhex(w).ljust(4, b"\0"), "little")

def num(s):
    v = int(s, 16) if s.startswith(("0x", "-0x")) else int(s)
    return v - (1 << 32) if v >= 1 << 31 else v

funcs, starts, cur = {}, {}, None
for line in open(dis):
    m = re.match(r"^([0-9a-f]+) <(.*)>:$", line)
    if m:
        cur = m.group(2); starts[int(m.group(1), 16)] = cur; funcs[cur] = []
        continue
    if cur and "\t" in line:
        funcs[cur].append(line.split("\t", 1)[1].strip())

def frame(ins):
    regs = {}
    for i in ins[:12]:
        m = re.match(r"addi(?:\.n)?\s+a(\d+), a1, (-?(?:0x)?[0-9a-f]+)$", i)
        if m and num(m.group(2)) < 0: return -num(m.group(2))
        m = re.match(r"movi(?:\.n)?\s+a(\d+), (-?(?:0x)?[0-9a-f]+)$", i)
        if m: regs[m.group(1)] = num(m.group(2)); continue
        m = re.match(r"(?:add|sub)(?:\.n)?\s+a(\d+), a1, a(\d+)", i)
        if m and m.group(2) in regs: return abs(regs[m.group(2)])
    return 0

fsize, edges = {}, {}
for name, ins in funcs.items():
    fsize[name] = frame(ins)
    out, loaded = set(), {}
    for i in ins:
        m = re.match(r"call0\s+([0-9a-f]+)", i)
        if m and int(m.group(1), 16) in starts: out.add(starts[int(m.group(1), 16)])
        m = re.match(r"l32r\s+a(\d+), ([0-9a-f]+)", i)
        if m:
            v = words.get(int(m.group(2), 16))
            if v in starts: loaded.setdefault(m.group(1), set()).add(starts[v])
        m = re.match(r"callx0\s+a(\d+)", i)
        if m: out |= loaded.get(m.group(1), set())
    out.discard(name)
    STOP = re.compile(r"jc_panic|handle_alloc_error|panic_nounwind|alloc_error_handler|core9panicking|panic_fmt|__wrap_system_restart|EspClass::restart|abort|__assert_func|__stack_chk_fail|panic\(\)")
    edges[name] = {o for o in out if not STOP.search(o)}

# Tarjan SCC (iterative)
index, low, onstack, stack, comp, idx = {}, {}, set(), [], {}, [0]
def strong(v0):
    work = [(v0, iter(edges[v0]))]
    index[v0] = low[v0] = idx[0]; idx[0] += 1; stack.append(v0); onstack.add(v0)
    while work:
        v, it = work[-1]
        nxt = next(it, None)
        if nxt is not None:
            if nxt not in index:
                index[nxt] = low[nxt] = idx[0]; idx[0] += 1; stack.append(nxt); onstack.add(nxt)
                work.append((nxt, iter(edges[nxt])))
            elif nxt in onstack:
                low[v] = min(low[v], index[nxt])
        else:
            work.pop()
            if work: low[work[-1][0]] = min(low[work[-1][0]], low[v])
            if low[v] == index[v]:
                c = []
                while True:
                    w = stack.pop(); onstack.discard(w); c.append(w)
                    if w == v: break
                for w in c: comp[w] = v
for v in funcs:
    if v not in index: strong(v)

members = {}
for v, c in comp.items(): members.setdefault(c, []).append(v)
cframe = {c: max(fsize[m] for m in ms) for c, ms in members.items()}
cedges = {c: set() for c in members}
for v, outs in edges.items():
    for w in outs:
        if comp[w] != comp[v]: cedges[comp[v]].add(comp[w])
memo = {}
def depth(c):
    if c in memo: return memo[c]
    memo[c] = (cframe[c], (c,))  # provisional (DAG: no cycles left)
    best = (0, ())
    for d in cedges[c]:
        r = depth(d)
        if r[0] > best[0]: best = r
    memo[c] = (cframe[c] + best[0], (c,) + best[1])
    return memo[c]

over = []
for r in roots:
    n = next((f for f in funcs if f == r), None)
    if not n:
        print("no", r)
        over.append(r)
        continue
    d, path = depth(comp[n])
    print(f"{r}: worst {d} B")
    for p in path[:16]:
        rec = " (recursive)" if len(members[p]) > 1 else ""
        print(f"   {cframe[p]:5d}  {p[:95]}{rec}")
    if limit is not None and d > limit:
        over.append(f"{r} ({d} B)")
if limit is not None:
    if over:
        print(f"FAIL: over the {limit} B stack budget: {', '.join(over)}")
        sys.exit(1)
    print(f"ok: every path within {limit} B")
