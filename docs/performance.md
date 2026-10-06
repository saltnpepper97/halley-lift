# Software drawing comparison

A local release-build check on October 5, 2026 compared the previous Lift
renderer with the migrated renderer in separate launches against the same
nested Halley compositor. Both borrowed native BGRA Wayland buffers and used
identical configuration, query fixtures, and visible-result limits. Icons were
disabled to exclude asynchronous resolution. The populated fixtures had ten
local desktop entries, with eight visible rows. No application was activated.

| Fixture | Previous warm median | Halley UI warm median |
| --- | --- | --- |
| Empty search | 1.10 ms | 0.88 ms |
| App results | 9.58 ms | 7.11 ms |
| Cluster-mode search | 9.56 ms | 7.26 ms |

These are small local samples of the existing draw timer, excluding the first
frame (six or seven warm samples per fixture). They include host buffer setup,
layout, software drawing, and accessibility update. They do not measure every
input journey, icon workload, GPU driver, display scale, or startup font scan.
First populated draws were about 10 ms for both implementations; the new empty
first draw was about 2 ms versus about 1.3 ms previously.

The first migration was slower and was not released. Halley UI 0.1.2 skips
transparent chrome and glyph pixels, uses exact straight-edge distance outside
rounded corners, and reuses identical solid-fill blending results. The software
notice and Lift render fixture remain byte-identical before/after those
optimizations. This comparison does not claim byte-identical typography between
Lift's old font renderer and the shared text system.

To check your own session, run `HALLEY_LIFT_PERF=1 halley-lift`. Timings go to
stderr. Installation and a running process are separate from source validation.
