# Batch 2 correction: provider HTTP remaining-budget cap

## Scope and release

Baseline: `d4dbd2bba14f474a33870269cc15bd98376ff360`. All changes remain uncommitted. This correction owns only:

- `crates/mail-core/src/provider_auth.rs`
- `crates/mail-core/tests/provider_auth_contract.rs`
- this new report

These files are released for parent integration and the subsequent frozen review. No manifests, module declarations, config, native integration, worker, renderer, recovery, search, or previous reports were edited. The separate recovery-directory-sync P2 is not this owner's task.

Read AGENTS, Rust guidance, architecture/design, provider foundation and provider handoff, prior review responses and batch ownership/build gates. Queried Graft first; its empty graph supplied no source nodes. Inspected the exact locked dependency source instead of relying on current/latest library behavior. Missing detailed rule files in the installed rust-skills package were not fabricated; its SKILL and the relevant installed Apollo testing/error chapters were available.

## Confirmed source defect and bounded correction

The frozen security R1 P2 is confirmed: ureq 2.12.1 defaults to a 30-second connect timeout, and that timeout takes precedence over the overall request deadline. Previously, `ProviderHttp::agent` set only the overall `timeout.min(HTTP_TIMEOUT)`. A remaining budget of two seconds therefore did not bound a dropped-SYN connect to two seconds.

The production builder now computes one `budget = timeout.min(HTTP_TIMEOUT)` and sets:

- `timeout_connect(budget)`
- `timeout_read(budget)`
- `timeout_write(budget)`
- `timeout(budget)`

`HTTP_TIMEOUT` remains 20 seconds. Both production `post_form` and `get_bearer` still use this single builder and the same bounded response reader. The overall deadline takes precedence over individual read/write settings in the locked version, so explicitly matching the read/write caps does not give each read a fresh overall budget. The response body reader retains ureq's deadline wrapper.

No public API/signature or typed error mapping changed. Transport errors still return `TemporarilyUnavailable`; this must not be presented as revoked consent. Redirects remain disabled. Catalogue token/identity/Graph-probe endpoints, default TLS validation, resource/protocol separation, response-size caps, cancellation/staleness checks, and checks before probe/save remain unchanged. No production endpoint override, resolver, proxy, invalid-certificate option, or new UI-thread work was introduced. No mail body or decrypted-body cache is involved. GTK compatibility is unchanged at the source-interface level, not proven by a GTK build.

### Exact locked primary-source proof

Both root `Cargo.lock` and native `apps/megamail/Cargo.lock` pin **ureq 2.12.1** with checksum:

`02d1a66277ed75f640d608235660df48c8e3c19f3b4edb6a263315626cc3c01d`

The cached `.crate` archive's SHA-256 matched this checksum. Python byte comparisons also confirmed that the inspected extracted `src/agent.rs`, `src/stream.rs`, and `src/request.rs` exactly match the files inside that archive. These are read-only dependency/source checks, not dependency resolution or compilation.

Primary source references for that version:

- [agent.rs](https://docs.rs/crate/ureq/2.12.1/source/src/agent.rs), lines 255–258: default connect timeout is 30 seconds; read/write defaults are unset. Lines 404–424 document and implement connect precedence; lines 427–490 document read/write versus overall precedence and implement the setters.
- [stream.rs](https://docs.rs/crate/ureq/2.12.1/source/src/stream.rs), lines 352–357: chooses the configured connect deadline instead of `unit.deadline`. Lines 384–410 apply its remaining time to `TcpStream::connect_timeout`; lines 433–442 apply the overall deadline to socket reads/writes. Lines 54–93 show the response-reader deadline wrapper.
- [request.rs](https://docs.rs/crate/ureq/2.12.1/source/src/request.rs), lines 122–147: derives the overall deadline from the request/agent timeout and passes it into the request unit.

## Authored regressions: actual production transport, unexecuted

Four additional Rust test declarations are authored in `provider_auth_contract.rs` (22 total, previously 18). None was compiled or executed here.

1. **Token response-header stall:** a bound random `127.0.0.1` listener accepts TCP but sends no response. The real Transaction and strict callback use an 800 ms attempt budget. A test-only `Http` adapter asserts the unchanged Google catalogue token endpoint, substitutes a numeric loopback fixture URL, then calls the actual `ProviderHttp.post_form` with the actual remaining budget and form. It verifies one token POST, zero identity GETs, zero probes and zero synthetic store calls.
2. **Token response-body stall:** the same production path receives a valid HTTP 200 header with a declared 100-byte body, then only one byte. This exercises `ProviderHttp::response` / ureq's `into_reader`, not only request-header timeout. It requires the same no-later-operation counts.
3. **Bearer body-read stall:** the actual `ProviderHttp.get_bearer` reads a partial response from numeric loopback with a 250 ms budget. Only a synthetic bearer value is supplied. This verifies the other production HTTP method uses the bounded reader.
4. **Connect SYN blackhole:** an explicitly ignored Linux test requires an isolated loopback network namespace and a SYN-drop rule for `127.0.0.1:49091`. It verifies the gate marker and that `/proc/net/dev` lists only `lo`. A 150 ms `TcpStream::connect_timeout` calibration must fail with `TimedOut`, not RST/refusal or a successful handshake. It then runs the real Transaction → loopback `Http` substitution → **ProviderHttp/ureq connect** path with 500 ms and 1,000 ms attempt budgets, requiring no subsequent identity/probe/store operation. It never returns a fake HTTP success.

Each transport assertion requires elapsed time at least half the passed budget, so immediate refusal/error is not accepted as a stall. Completion must be within the passed budget plus one second of scheduling tolerance. The tolerance is a test-harness allowance, not a larger production timeout contract. The elapsed interval covers the actual production POST, while the budget originates from the attempt after callback processing. Header/body fixtures separately require confirmed TCP acceptance. They have bounded accept/write/hold time and a stop-channel/Drop join for cleanup on success or assertion failure.

**The first three tests cannot establish connect-timeout coverage.** An accepted connection stalled on headers/body does not reproduce silently dropped SYNs. Only the gated fourth test targets the reported connect defect. Its expected negative control is the previous builder (overall timeout only): connect may block about 30 seconds, exceeding both short budgets. That negative control was not executed and no red/green result is claimed.

All fixture endpoint substitutions exist only in the integration-test file. No real provider/public network is used or permitted. The probe and store remain synthetic counters because the required assertion is that neither stage is reached; this does not test real probes/keyring/config persistence.

## Parent-owned grouped verification gate

No Cargo, rustc, check/build/clippy, helper compilation or Rust acceptance run occurred. Preserve the single grouped window after all owners freeze. Parent must compile and run the core contracts serially with the rest of the batch; normal execution will leave the SYN-blackhole test ignored.

A capability-only probe, `unshare --user --map-root-user --net true`, exited 0 here. It created no sockets, set no firewall rules, and ran no Rust. This establishes namespace creation only. It does **not** establish `nft` permission, loopback SYN dropping, deterministic timing, or regression success. The blackhole regression remains an explicit acceptance gate until the full isolated fixture runs.

During that grouped window, after compiling the integration test, set `AUTH_TEST_BIN` to its actual absolute executable path. Then run the already-built binary inside a fresh rootless namespace. The following is a prepared execution recipe, **not an executed command**:

```sh
# AUTH_TEST_BIN must be the parent-built provider_auth_contract test executable.
# Run as the normal user, without sudo. Never install these rules on the host.
export AUTH_TEST_BIN
unshare --user --map-root-user --net sh -eu -c '
  ip link set lo up
  nft -f - <<"RULES"
table ip megamail_provider_budget {
  chain output {
    type filter hook output priority 0; policy accept;
    oifname "lo" ip daddr 127.0.0.1 tcp dport 49091 tcp flags & syn == syn drop
  }
}
RULES
  MEGAMAIL_PROVIDER_BLACKHOLE=isolated-loopback timeout 10s "$AUTH_TEST_BIN" \
    production_transport_budget::production_connect_blackhole_obeys_remaining_authorization_budget \
    --exact --ignored --test-threads=1
'
```

The rule is scoped to the new user/network namespace and one loopback destination port. Callback traffic uses another random loopback port. The namespace and its rules disappear when its last process exits. The outer watchdog bounds a regression to the old 30-second default; watchdog expiry is failure, never success. Use the already-built binary, not Cargo inside the isolated namespace.

If namespace setup, local nft capability or calibration is unavailable, record **connect regression blocked/unexecuted**. Do not substitute a reachable host, reserved/public IP, fake HTTP, nondeterministic backlog saturation, host-global firewall changes, or root/sudo. Header/body-stall success alone does not close this gate.

## Executed static checks and remaining limits

Executed successfully:

- Direct `rustfmt --edition 2021` on the two owned Rust files, then direct `rustfmt --edition 2021 --check` on those same files.
- Locked archive checksum and three primary-source byte comparisons.
- Python source assertions for the single min-capped budget and all four timeout setters; redirects disabled; catalogue identities retained; existing post-operation checks present; 22 test declarations; actual production POST delegation; SYN calibration and explicit ignore gate; header/body/bearer fixture declarations; owned-file whitespace.
- Scoped `git diff --check` plus an explicit whitespace scan because both Rust files were already untracked in this batch.

These checks establish formatting and source facts only. No real provider/browser/keyring/private profile/mail/installer operations, helper execution, manifests or commits were involved. No new performance, login, network-test, TLS-security or GTK acceptance claim is made.

Residuals preserved from the original handoff: blocking OS DNS cannot be interrupted by ureq's available APIs; cancellation does not interrupt an in-flight blocking HTTP call; late results remain checked before advancing. A DNS delay can still exceed the attempt budget, and the configured connect duration is relative to connect setup rather than an interruptible attempt-wide clock. Thread scheduling and OS timeout rounding also require runtime acceptance. Credential-store finalization remains outside forcible timeout interruption. Owned registrations, native serialized roster/ID integration, committed-token rotation coordination, real TLS/protocol acceptance and the parent's remaining integration obligations are unchanged. The separate post-rename recovery P2 remains assigned to its separate owner.
