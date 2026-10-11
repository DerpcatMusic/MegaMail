# Batch 2: parent-owned serial verification window

No compilation has occurred for this batch. Run only after every source-editing owner and corrective review has released. The script defaults to printing its plan; source receipt creation is separate from execution.

```sh
python3 tools/verify-megamail-batch2.py
# AFTER whole-batch release and source review; choose a NEW private receipt path:
python3 tools/verify-megamail-batch2.py --record-freeze /absolute/private/batch2-freeze.json
python3 tools/verify-megamail-batch2.py --execute --freeze-state /absolute/private/batch2-freeze.json --connect-blackhole
```

The driver checks baseline and hashes tracked/untracked source inputs. It rechecks before each step and stops on source change. Generated license artifacts and helper build outputs are excluded, not application sources. Explicit reviewed asset namespaces also include native fonts, native/core fictional fixtures, bridge extension manifests/schema, and vendor dock fixtures; arbitrary runtime JSON/EML is not indiscriminately read. No target-directory/compiler-wrapper override, installer, default-handler association, private Thunderbird probe, real login or mail sending is invoked. Native/core/theme compilation is serial with two compiler jobs per Cargo invocation; C production and no-CSP mutation variants build in the same grouped window. Normal tests keep private/external/benchmark cases ignored.

Logs and a results.json remain in an owned private directory. Commands have bounded timeouts and own process groups. Timeout cleanup targets only the unreaped group; it never uses global pkill. Dependent native gates are not run after a core failure. Exact synthetic connect acceptance runs only with explicit opt-in, the actual compiled provider contract binary and a fresh rootless loopback-only namespace. Host firewall rules are never changed. Failed namespace/calibration is not a transport pass.

This driver does not declare release acceptance. C policy tests are not engine egress proof; helper readiness is not fidelity or accessibility. Separate post-build gates are required: actual descendant request tracing, missing extension/crash/teardown checks, isolated native GUI observations and manual review, and legacy GTK executable checking if WebKitGTK6 development files become available. Real providers, owned registration deployment, inline GPUI, screen-reader/clipboard integration and physical-display latency remain separate.

Native GUI harness: tools/megamail-native-batch2-qa.py and batch2-native-qa-plan.md. It defaults to no launch and requires a matching manually reviewed final-source receipt, trusted built binary, private HOME/XDG, owned Xvfb and network/PID isolation. Demo deliberately has no normal mailto activation or provider/recovery service; those unavailable journeys are not passed by screenshots. All captured content must be fictional.

Executed preflight only: bridge JS fixtures, six actual-generator packaging tests, three synthetic kernel/static tests, fourteen helper source checks/63 installed header symbols, native QA dry-run, verification plan output and whitespace/syntax checks. These are not Rust compilation, native runtime, engine safety, real Thunderbird or provider acceptance. Ten launch-free verification-driver contracts additionally cover receipt refusal/private creation, source-change refusal, prerequisite skipping, owned mocked process-group signaling and embedded-asset hashing. Signals/compiler commands are mocked; these are not kernel cleanup or compilation proof.
