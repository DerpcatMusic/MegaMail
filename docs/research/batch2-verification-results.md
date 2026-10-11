# Batch 2 verification: first grouped window and correction batch

Source receipt: /tmp/megamail-batch2-window-20261010/freeze.json (574 source/embedded inputs). Retained logs/results: /tmp/megamail-batch2-window-20261010/logs/.

## First grouped window — executed

- Source/launch-free/bridge/packaging/kernel preflights passed with their stated limited proofs.
- Core Cargo compilation failed: four exhaustive dispatches (Graph, JMAP, POP3, demo) omitted additive LoadBodyAtEpoch; provider SMTP transport builder could not infer its executor. Five distinct errors, duplicated for library and library-test compilation. No batch-2 core Rust test pass follows.
- Native tests/build and license refresh were skipped because the core gate failed. Isolated SYN fixture was not run.
- Theme compiled and 25 unit tests passed (doc-suite details retained in theme-tests.log).
- C production helper/extension and separate test-only no-CSP mutation compiled successfully. Actual compiled policy self-test executed 35 assertions; production policy fixture checks passed. This is NOT engine egress, rendering, clipboard, accessibility, or helper-release acceptance. tests.py's initial compiled:false line refers to its source/header audit, not the subsequently executed compiled policy seam.
- No native/real-TB/provider launch, login, private profile/keyring operation, real mail send/mutation, installation or default-handler change occurred. Core tests did not execute because compilation failed. The theme/policy tests use synthetic local data.

## Bounded correction batch — prepared

Parent adds explicit fail-closed arms for LoadBodyAtEpoch to Graph/JMAP/POP3/demo through reject_unsupported_epoch_body: one BodyEpochStale with exact original request ID/path/UID/expected generation, actual_generation=None and fixed capability refusal. No I/O, fake UIDVALIDITY, legacy-body downgrade or metadata success. Ordinary body requests remain unchanged. Two real helper-seam Rust regressions are authored, not yet run, for exact correlated refusal (including zero/max generation) and ordinary request non-consumption; these do not execute complete provider loops.

ProviderHttp's outgoing probe now explicitly types AsyncSmtpTransport<Tokio1Executor>. Locked lettre0.11.23 confirms builder build<E> returns that transport. Timeout/TLS/credential/endpoint/cancellation semantics are unchanged.

All source editing owners remain released; these parent integration corrections invalidate the first receipt. Syntax/whitespace checks passed. New worker SHA-256: b08c3f70735bf233af55b5229d024e81192323ccc99d46ef92ec488ee4b99c56. Preserve original failures/logs, record a fresh receipt, then execute the bounded grouped follow-up. No per-patch compiler loop was run.

## Correction window 1 — executed

Core compiled and passed: 574 library tests (19 ignored), four callback contracts, 21 provider contracts (one ignored). The exact ignored SYN-blackhole production-adapter contract separately passed in a new rootless loopback-only namespace after calibration (1.65 seconds; 21 others filtered out). That is 599 ordinary core/public passes plus one separately isolated connect pass, not real provider acceptance.

Native compilation then exposed four production errors and additional test errors: recovery Send request IDs narrowed to u32; OriginalBody lacked Debug; fictional AccountConfig fixtures assumed an unavailable Default; one test omitted WorkerMailboxEvent import; a Drop-owning Session test used illegal struct-update movement. Native Rust tests/build and license refresh remained not_run. Theme and C policy repeated green; no GUI launch.

Parent prepared all known native errors together: retained recovery tuple now keeps full u64 request IDs without truncation (new max-ID test); OriginalBody custom Debug prints only size/completeness/eligibility, never raw HTML (new redaction test); fixtures use the existing AccountConfig::new constructor rather than inventing a production Default; test enum import corrected; Session fixture uses default then field assignment. No provider/renderer gate is enabled and no drop/cancellation/privacy contract is weakened. New receipt required before the next bounded verification window.

## Correction window 2 — executed

Core/public contracts and isolated connect fixture repeated green. Native production compilation cleared its four errors; the test target exposed only a missing qualification in the newly added max-request-ID test. Parent corrected Capture to super::Capture (test-only name resolution, no runtime change). Native tests/build/license remained pending, not passed. A new receipt is required before that bounded follow-up; no sources were edited while Cargo ran.

## Correction window 3 — executed; two red-capable native regressions

All native compilation now succeeds. Native suite executed: 264 passed, two failed (266 total). Production ordinary executable artifact exists, but explicit native-build/license gates remained not_run because the suite failed. Both failures were reproduced directly from the existing compiled test binary with exact test filters, no compiler invocation, in under a second.

The failed-import regression demonstrates missing configured identity metadata: a source-qualified NeedsLogin outcome supplied one@example.test, but its durable dormant row retained blank email and therefore unnecessarily blocked direct admission. Parent now fills only an empty TB row address from a nonblank, durable profile+source-ID-qualified failure. It does not install a sender, set Ready, overwrite a healthy known address or attach to a direct owner. The existing regression is strengthened with exact retained address/no sender assertions; a new blank-address regression preserves fail-closed admission and healthy sibling usability. This is identity metadata, not authentication evidence.

The LRU regression expected 31 bytes before original-source retention; the actual 44 includes the additional 13-byte original HTML. Parent updates only that expectation to count original.retained_bytes, adds an exact source length assertion, and preserves entry/memory/oversize eviction checks. Production byte accounting/cap are not weakened.

Prior corrective tests (full-u64 retirement, redacted original Debug) compiled and passed in the 264. All new source/whitespace checks pass; no compiler ran per patch. A bounded read-only acceptance review precedes the next new receipt. Actual UI/provider/engine acceptance remains separate.

## Correction window 4 — compiled batch green

Private receipt freeze-correction4.json covered 574 inputs. Every grouped gate passed: 599 ordinary core/public contracts (574 library +4 callback +21 provider), 267 native tests, 25 theme tests, normal native build, production helper/extension and no-CSP mutation build, 35 compiled C policy assertions/production fixture policy checks, license refresh and calibrated rootless loopback-only SYN-blackhole production adapter. Normal ignored tests stayed ignored. The native isolated child test emits an additional one-test result within the parent suite; do not count it twice. Structured results explicitly keep release_accepted=false.

Actual helper engine baseline FAILED before READY: WebKit inner bubblewrap needs /sys/{block,bus,class,dev,devices}; outer sandbox provided none. R6-diagnosis captured ENOENT, bwrap exit1, credential-channel EOF, UI abort134. An external minimum-privilege empty read-only sys tree retained all other containment/fd3 checks and yielded production READY+7 token-correlated heartbeats using unchanged compiled helper. This is experiment/readiness evidence, NOT current source-helper acceptance or request/egress/pixel/A11y proof.

Actual native QA attempt BLOCKED before app or Xvfb launch: isolated /qa-run.py module import indexed parents[1], raising IndexError. No observations/screenshots/native.log exist; this is a harness defect, not a native render failure.

Parent groups fixes: C helper gets PRIVATE tmpfs /sys with five EMPTY directories and read-only remount, no host sysfs/device access; QA uses parent.parent preserving host ROOT and permitting root-level import; real launch-free import tests at both paths (2/2), additional helper source assertion (15 checks/63 symbols), driver contracts 10/10 pass. Separate real rootless kernel smoke proves exact proposed tmpfs recipe empty/read-only (EROFS on fictional write), not engine proof. Sources not compiled per patch; R7 review and new receipt precede next serial window, then actual engine/GUI retries. Prior logs preserved.

## Correction window 5 — latest compiled receipt and actual runtime observations

R7 read-only Standards/Spec accepted the private empty read-only hierarchy and import-path corrections; no new hard breach/P2. Separate freeze-correction5.json covers575 inputs. The subsequent entire serial grouped window passed again:599 ordinary core/public,267native,25theme, normalnativebuild, real Cproduction+noCSPvariant build/policy35assertions, license inventory and calibrated isolated SYN production-adapter gate. This is current compiled green—not merely source disposition.

Actual currentcompiled productionhelper authoredfixture engine_probe returned0: real helper IPC READY+token-correlated heartbeat (helper-authored-engine-correction5.log). Containment/inner WebKit remainenabled; no hostsysfs ordevices exposed. Actualegress/pixel/input/A11y/nativehostcancellation remainunaccepted.

Actual nativeQA returned0 in source-reviewed owned net/PID/IPC namespace, private emptyHOME/XDG andXvfb. native-qa-observations-correction5 contains13window-only PNG captures plusowningPID/window/geometry/time receipt. Manual review: populatedfictionalRTL body visible,1060x640 longsubject/sender/recipient metadata wraps without visiblehorizontalspill, bodybelowfold requiresreading. Originalformatting action visiblydisabled, consistentwithreleasegate. Calendarunknown warning/idlequeued-linkcopy createobservable UI noise; notfactored into functional pass.

Manual inspection finds select-next andcompose-request captures stillsame selectedmailbox ratherthanprovingnavigation/composer. QA intentionallylabels theseinputs/observations not_run; harnessXSendEvent vsGPUIXI2/XTest is a hypothesis, notyetconfirmedappbug. A read-onlyactualFocusReader/SelectAll/Copy probe willtestnativeconsumer +correct delivery in ownedprivate display. Separateread-onlyhostile/parser/noCSP-descendanttrace task examines actualrequest attempts/egress/teardown. Neither mayedit source/build. Originalgate remainsclosed, norealprovider/profile/keyring/mail/installation operations.

## Actual reader keyboard/clipboard probe, round 1 — not accepted

Read-only actual-runtime task released batch2-reader-clipboard-runtime.md. Pinned production demo and body observation passed; source575/binary hashes unchanged and owned app/display/sandbox processes reaped. XTEST with exact owned window focus did not visibly change selected Leila row after Down/Up; cause remains unresolved. Actual FocusReader/SelectAll dispatch was not witnessed. Copy readback was BLOCKED because neither xclip nor xsel is installed; no body oracle comparison occurred. This is not a native Copy pass or a proven reader-handler defect.

A new bounded read-only round uses an owned-display Xlib clipboard consumer (no installs) and a narrowly authorized fictional search-field input control to distinguish initial internal-focus, input translation and reader-only failure. It preserves isolation and frozen source. Helper hostile/descendant tracing task remains separate. No new compilation/source changes.

## Actual standalone hostile-engine/request/cleanup gates — scoped pass

Read-only task released batch2-helper-engine-runtime.md; current production helper a1412050…/test-only no-CSP4528cf4c… +extension1c21b3d8… matched frozen helper fixtures/15 receipt-covered files. Independent bounded read-only evidence review now checks observer coverage and reporting. No code changes or compilation.

Actual library traces order hook installation before acknowledgement and document loading. ProductionCSP authored/hostile/parser fixtures reach readiness; no remote requests reach hook in production (earlier parser/CSP prevention). Standalone NO-CSP mutation exercisesactualextension:5hostile +1parser-confusion requests denied; actualcallback cancels; authoredcontrol document/CID allowed. This is hook independence for six observed requests—not allCSS/parser/redirect/file paths.

Separate descendant syscall traces recordzero AF_INET/6 sockets/address operations for testedhelpers, distinctfromsix DOM requests. A separateOWNprivate-loopback IPv4/6TCP/UDPcontrol confirmscapture ofInternet-familyattempts/sends. InitialEPERM setup isblocked/excluded,notpass. No host firewall/listener/provider operation.

Ownedwindowclose emitsclosed/exit0; observer-ownedcancellation andclose leavezero live descendantsbeforeforcedcleanup. Cancellation'sfinalintervallosestracercoverage,doesNOTprove nativeReaderHost cancellation. MissingextensionrefusesbeforeWebKitexec; pinnedWebProcesskill emitsweb-process-terminated andsubsequentownedwindowclose exits2 withoutsurvivors. Own processes/socket/lock reaped. Artifact summary hash b9831cd6af25de0cb38f3e6a0e30c89fb01a4c33d4337824295151a4b7b4f293 in/tmp/megamail-engine-runtime.xDkLq2.

Redirect/image-set/realfile/user-click/hung/native lifecycle/pixels/CIDtransport/A11y/inline remainunaccepted;defaultgateCLOSED. Newreadonlyactualstyledfixturepixel/resizeprobe complementsrequesttracing,notfullmailfidelity.

## Parent input control and actual Copy — startup red, post-focus green

The first child rounds removed missing clipboard tools with actual namespace-private Xlib, but manual review gates blocked input control. Parentphase3 verifiedfreshSearchandbody captures, typedfixedfictionalsentinel mm-clip-probe-260 in knownownedSearch box300,108, observed0results, clearedandselectedPriya withCtrlAltL/Down. Lastbodybelowfold; reviewgateexpired. Notclassifiedasappcopyfailure; latecontrol-only directoryremoved.

Parentphase4 keptallbinary/digest/netPIDIPC/ownedPID/XID/window/clipboard safeguards but removedmanualreviewwaits; capturesnot_runpendingmanualreview, fixed8ssettlepauseNOTreadinessorlatencyproof. Actualreadbackoracles determinefunctionalstatus. StartupCLIPBOARDowner0 failsagain. AfterallowedSearchclick/type/clear,actualCtrlAltR/A/C onPriya producesUTF8_STRING185bytesand2bodyonlyoraclematches;UpreturnRTLproduces1021bytesand2matchingbodyoracles. ThisprovesrealretainedTextViewCopyandselectionchangeAFTERinternalinputfocus; rulesoutblanketXTESTfailureinthisconfig. Startupcontextstillbroken, notaccepted. No source/mocks/clipboardseed;ownapp+Xvfb+parenttimeoutreaped,575receiptunchanged. Tightcommand/artifacts inbatch2-reader-clipboard-parent-control.md.

R8 independentlyverified70engineartifacts/all15helper-sourcefixtures andacceptsnamedrequest/noegress/standaloneteardown scenarios, no blanketsecurity/provider/inlineclaims.

Actualhelperpixels900x700 and500x700: preservedtabletwoequalpanels, exactbackground/foregroundcolors/headings/fakecopy, decodedCID32x32image=1024pixelsbothcaptures. Request430clamped455 becauseGTKmin455; ≤440stackingCSSretainedbut visualactivationBLOCKED. OwnershipNSpid/pidfd/starttime/ownXvfb/closedtoken/exit0+noleftoverdescendantssupported. Fixturesunmodified,all575inputsunchangedthroughtheseprobes;no compiled-sourceupdates.

Batch3threeexclusiveownersnowpreparesource,no builds: nativeMaininitialfocus+scopedcontacts/literalcomposecontrols,helperMainadaptive narrowtoolbar,TBAdapterboundedpagereadlane.P1oldrules/parsedsearch/bulkstillunwired;sourcebatchnotacceptedwithoutnewreviews/receipt/groupverify. DefaultoriginalHTMLgateCLOSED,no realproviders/profiles/keyring/mail/install/physicalperf/A11y.

Release remains unaccepted. Actual native UI/lease/focus, helper engine/descendant requests, real providers, original inline/CID, shutdown and performance remain separate gates.
