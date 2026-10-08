# MegaMail research

These are source reviews of pinned upstream revisions. Implementation facts
are linked to files and lines; design rationale is labeled as interpretation.
Performance budgets are targets until measured in MegaMail.

| Report | What it resolves |
| --- | --- |
| [Hylki](hylki.md) | What is reusable, real mail-worker boundaries, GTK coupling and extraction cost |
| [Zeron design system](zeron.md) | Actual tokens, components, geometry, state vocabulary and source map |
| [Zeron hierarchy](zeron-hierarchy.md) | Why it reads well; information priority, density and adaptation to email |
| [Zeron materials](zeron-materials.md) | Glass, opacity, blur, edge fades, platform behavior and frame scheduling |
| [Visual atlas](design-atlas.html) | Portable illustrated anatomy of the design language; a research diagram, not an app screenshot |
| [GPUI Kit](gpui-kit.md) | Official crate/API recipe, platform requirements, primitives and renderer constraints |
| [Performance and security](performance-security.md) | Existing bottlenecks, mail integrity/privacy requirements and measurable acceptance gates |
| [Account onboarding](account-onboarding.md) | Thunderbird ISPDB discovery versus provider OAuth, existing Hylki setup and MegaMail-owned sign-in |

The [architecture decision](../MEGAMAIL_ARCHITECTURE.md) turns these findings
into an implementation sequence. The root [DESIGN.md](../../DESIGN.md) is the
native UI contract; source research alone does not verify its implementation.
