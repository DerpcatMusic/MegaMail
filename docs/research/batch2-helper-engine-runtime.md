# Batch 2: compiled helper engine runtime

The original-formatting release gate remains CLOSED. These results cover the standalone C helper, not native ReaderHost, inline HTML, selection, clipboard, pixels, or accessibility.

On 2026-10-10, this run used Correction 5's compiled production and test-only no-CSP packages. No compiler, make, Cargo command, source edit, or test-suite rerun occurred. AGENTS.md and the requested helper files were read fully. Graft returned no matching nodes. Temporary launchers, fictional inputs, and traces remain outside the repository.

## Receipt and containment

The parent receipt is `/tmp/megamail-batch2-window-20261010/freeze-correction5.json`, with 575 inputs. All 15 receipt-covered helper/fixture files still match their frozen hashes. README is outside that receipt and has a separate recorded hash. The parent's 35 compiled policy assertions and authored readiness probe are existing evidence, not results of this run.

The earlier diagnosis records helper `6887e82b…` aborting before WebProcess execution because `/sys/block` was absent. Current helper `a1412050…` executes both WebKit processes. Its unchanged compiled launcher creates five empty private sys directories and remounts `/sys` read-only. Traces record the genuine namespace FD and different namespace identities. No external sys fix or host sysfs mount was used.

Every helper launch retained its own user/network/PID/IPC/UTS namespaces, capability drop, read-only package/runtime, ephemeral home, and enabled inner WebKit sandbox. A fresh fictional Xvfb `:35827` supplied only its local socket. No host profile, bus, Xauthority, GPU grant, provider, or credential was used. Each phase stayed below 40 seconds, including cleanup.

## Actual request observations

Ltrace, a library-call observer, followed the helper and descendants. An extra observer-only namespace exposed the genuine `/app` ELF for tracing. The helper still ran its complete compiled sandbox launcher. Separate strace runs followed descendant system calls from startup through normal window teardown. Cancellation stops the observer, so its final interval lacks syscall coverage.

Every library trace orders hook installation before acknowledgement, then untrusted document loading. Hostile/parser traces show those operations at lines 40, 41, and 43. Each document capability returns allowed. The authored baseline also allows its CID `/part/0` resource.

With production CSP, hostile and parser fixtures reach readiness and matching heartbeats. Neither generates a remote request at the extension hook. This is consistent with earlier parser/CSP blocking, not proof that the extension rejected remote requests.

With test-only no-CSP, the actual extension rejects five hostile requests: import, background, remapped file cursor, escaped CSS URL, and font. It rejects the parser fixture's `css-confusion` request. `mm_request_allowed` returns zero. The compiled cancellation callback returns its inverse. This demonstrates hook independence for these six requests only. No native process received the mutation policy.

Every helper syscall trace, including the clean authored baseline, contains zero AF_INET/AF_INET6 socket/address operations. Observed DNS/Internet connection/send attempts and successful Internet kernel egress are zero. AF_UNIX display/IPC traffic is separate. Zero host-loopback hits were not used as evidence.

A separate private-network control successfully exchanged fictional IPv4/IPv6 TCP and UDP bytes. Strace captured both address families, connection attempts, and sends. The control raised only its private loopback interface, then cleared capabilities. Its first setup failed with EPERM and remains BLOCKED evidence, not a pass.

## Teardown and limits

Owned window close produces `closed` and exit zero. Observer-owned process-group cancellation produces no graceful close event. Both leave zero live descendants before forced cleanup. Post-cancellation identity checks establish teardown, not uninterrupted syscall observation. PIDfds and start times pin process identities. Cleanup signals only owned processes, reaps adopted children, and removes the display socket/lock.

A missing-extension package copy returns `extension-unavailable`, exit two, no readiness, and no WebKit process execution. Killing the pinned WebProcess produces `web-process-terminated`. Subsequent window close exits two without remaining descendants.

Inputs copy repository fixtures, replacing loopback/file destinations with `synthetic.example.test` URLs. Original and adapted hashes are recorded separately. Redirects, image-set dispatch, real file requests, user-click handoff, hung-process behavior, and native lifecycle remain unaccepted. Initial observer/window/crash harness setup failures are excluded. These observations are not a blanket security proof.

## Artifacts

`/tmp/megamail-engine-runtime.xDkLq2/` contains `runtime.py`, `summary.json`, `artifact-sha256.json`, paired `ltrace-*`/`strace-*` traces, per-phase receipts, and control evidence. The summary records fixture hashes, exact hook calls, process identities, namespaces, and source comparisons.

```text
SHA-256 production helper:
a14120503370abe86b962d66ff88bc48a18fc32b8a59ed234f640d0b54d18db0
SHA-256 no-CSP helper:
4528cf4c3bea7174c85406e03a911d81a559ccf205536ac8b8eb665b812d2f8e
SHA-256 both extensions:
1c21b3d81abdfac75361f87fe3df5825eba32f7dfd254d996a3897fbfbff411f
SHA-256 summary.json:
b9831cd6af25de0cb38f3e6a0e30c89fb01a4c33d4337824295151a4b7b4f293
```
