# Halley Lift

Halley Lift is a standalone command palette for Halley. It is built from the `halley-lift` crate, installs as the `halley-lift` binary, and uses the public `halley-api` SDK for compositor queries, actions, cluster drafts, and live updates.

Version 0.4.0 is in preparation. See the [changelog](CHANGELOG.md#unreleased)
for the planned release notes.

## Install

Install the released 0.3.0 version:

```sh
cargo install halley-lift --version 0.3.0 --locked
```

Or build this repository:

```sh
git clone https://github.com/saltnpepper97/halley-lift
cd halley-lift
cargo build --release --locked
./target/release/halley-lift
```

Lift needs a running Halley session, Wayland layer-shell support, a system font,
and the native build dependencies for Wayland, Fontconfig, and xkbcommon. On Arch,
install `base-devel`, `rust`, `wayland`, `fontconfig`, and `libxkbcommon`.
Halley 0.8 is the intended companion for this version. Older compositor versions
may not support newer actions such as **Show Halley basics**.

Running-node focus and panning are handled by Halley. Compositor builds predating
the [API retrieval fix](https://github.com/saltnpepper97/halley/pull/189) retain
their previous retrieval behavior even when Lift is upgraded. The fix does not
require a new public SDK or wire protocol.

On compositors supporting `ext-background-effect-v1`, Lift sends a blur region
matching its painted pixels, so blur follows rounded corners and excludes the
transparent gap between a separated search bar and dropdown. The region updates
with the buffer when the panel grows or shrinks. Halley's layer rules still
control whether blur is allowed; compositors without this extension retain their
existing blur behavior.

Lift is versioned independently of Halley. Its source and releases now live in
this repository; it is no longer built by Halley's workspace.

## Run

```bash
halley-lift
```

You can also seed an initial query:

```bash
halley-lift cluster release
```

Lift opens without waiting for the compositor API. Action requests run in the
background with two-second socket read/write timeouts, keeping search and Escape
responsive if the API stops replying. Repeated activation is ignored while an
action is pending; successful activation closes Lift as usual.

Action errors appear below the search box, including with `footer-height 0` or
an empty query. Editing the query or starting a new action clears the previous
message. The optional footer controls shortcut hints independently of errors.

Live node and cluster results recover from API disconnects and missed events by
reconnecting for a complete snapshot. Failed initial loads retry in the
background, with a one-second pause between attempts. Servers that reject
subscriptions use repeated node and cluster queries. Lift keeps the last
successful results while reconnecting and replaces them when fresh data arrives.

Launching Lift again closes the existing instance. Coordination uses
`$XDG_RUNTIME_DIR/halley/halley-lift.sock` and a `halley-lift.lock` file in the
same directory. The lock file stays for the runtime session; the operating system
releases its lock automatically when Lift exits or crashes.

## Launching From Halley

A freshly generated Halley config binds `Mod+D` to `halley-lift`, so Lift is the
default front door for launching applications and for retrieving work that is
already running. Halley launches it as an ordinary command line with the session
environment, so no arguments or environment variables are required and the whole
integration is one line in the `keybinds` section.

Halley keeps existing configurations unchanged. Keep exactly one launcher on
`Mod+D`. To use Lift, edit the binding yourself:

```rune
"$var.mod+d" "halley-lift"
```

To use Fuzzel, replace that line with:

```rune
"$var.mod+d" "fuzzel"
```

Halley owns that keybind and Lift owns its own window and appearance in
`lift.rune`, so the two configurations stay independent.

## Search Prefixes

Lift's general search uses the providers listed in `search.providers`. Prefixing the query with a provider name filters results without changing the search text into a badge.

Supported modes:

```text
app apps
cluster clusters
node nodes
action actions
config
term
```

Example:

```text
cluster release
```

searches clusters for `release` while leaving the full text visible in the search field.

`term` runs the typed command line in the configured `terminal` through your
interactive `$SHELL` (so aliases, pipes, and `&&` work), keeps a shell open afterward, and then
closes Lift:

```text
term journalctl -f | grep halley
```

## Cluster Drafts

In `cluster` searches, `Space` stages or unstages the selected app or running node,
including when the search is empty. This does not launch or focus the target.
Outside cluster searches, Space is normal search text.

After at least one item is staged in cluster mode, `Ctrl+Enter` or activating `Create cluster: <query>` materializes the draft:

```text
Cluster Draft: release · 3 selected
```

At that point Lift opens Halley's existing Cluster Finalize popup with a name hint and selected running node IDs. Staged apps are launched only during this handoff, and the compositor auto-selects matching newly appearing nodes while that finalize prompt is active.

Lift does not directly persist clusters. The finalize popup owns naming, confirmation, and final creation.

## Compositor Actions

The `action` provider exposes compositor-owned actions:

- **Reload Halley config** — reload the selected configuration immediately.
- **Show Halley basics** — reopen Halley's one-time basics card, which names the
  Field-first mental model and the five essential chords. The card is offered
  automatically once to a freshly generated configuration's first native
  session; this action always works, whether or not that card was already
  dismissed, so it doubles as the development path for inspecting the card in a
  nested `halley --winit` session.

## Pins

Lift does not keep its own favorites database. Field/Bearings-pinned nodes come from Halley and rank above normal matching nodes.

## Config

Config path:

```text
~/.config/halley/lift.rune
```

On first launch, if no config is present Lift writes a documented default
template (mirroring `examples/lift.rune`) to that path so you have a starting
point. Existing files are never overwritten.

Example config lives at `examples/lift.rune`.

`fuzzy false` uses case-insensitive contiguous matching instead of matching
scattered letters. Exact and substring matches retain their existing ranking.
`providers` chooses which providers contribute to general searches; an empty
list disables all general-search results. Provider order does not change ranking.
Known names are `apps`, `clusters`, `nodes`, `actions`, and `config`.

The `modes` flags independently enable prefixed searches. For example, omitting
`nodes` from `providers` removes running windows from general search while
`search.modes.nodes true` keeps `node Firefox` available. A disabled mode returns no
results. Cluster mode still offers apps and nodes to stage, regardless of the
general provider list. `term` remains available separately. The default general
provider list omits configuration files; the `config` prefix remains enabled.

`colors.panel` supplies the background when `colors.search` or
`colors.dropdown` is empty or invalid. Explicit search/dropdown colors override
it, including their alpha; Lift does not paint a second background beneath them.

The complete default configuration is:

```rune
@description "Halley Lift configuration"

behavior:
  close-on-focus-loss false
  alt-number-jump true
  terminal "x-terminal-emulator -e"
end

search:
  placeholder "Search apps, nodes, clusters, actions..."
  fuzzy true # false = contiguous matching only
  max-results 12
  visible-results 8
  show-section-labels true
  # General search only; [] disables it.
  providers ["apps" "clusters" "nodes" "actions"]

  # Enable prefixed searches independently of the general provider list.
  modes:
    apps true
    clusters true
    nodes true
    actions true
    config true
  end
end

position:
  anchor "center" # center | top | top-left | top-right | bottom | bottom-left | bottom-right
  offset-x 0
  offset-y 0
  top-margin 96
end

icons:
  enabled true
  size 28
  theme "auto"
end

layout:
  width 760

  search:
    height 60
    padding 20
    icon-gap 12 # Space between search glyph and text.
  end

  dropdown:
    gap 0
    padding 10
  end

  results:
    padding 20 # Horizontal inset of the results body.
    bottom-gap 8
    section-height 22
    footer-height 0
  end

  row:
    height 64
    gap 6
    padding 5
    content-gap 16 # Space between marker, icon, text and hint.
    text-gap 7 # Space between title and description.
    cluster-marker-size 18
  end

  draft:
    height 36
    padding 14
  end

  status:
    padding 3
  end
end

typography:
  font "sans-serif"
  search-font-size 22
  badge-font-size 15
  title-font-size 17
  subtitle-font-size 13
  hint-font-size 12
end

# Scroll overflowing text in the highlighted row.
text-scroll:
  enabled true
  delay-ms 900
  speed 30 # Pixels per second.
  pause-ms 1200
  transition-ms 180
end

colors:
  panel "#151720ee" # Fallback when search/dropdown is empty or invalid.
  panel-border "#2b3248cc"
  dropdown "#151720ee"
  dropdown-border "#2b3248cc"
  search "#090b12d8"
  row-selected "#2e4575ea"
  divider "#2b324899" # 1px connected-panel separator; #00000000 hides it.
  text "#f2f5ff"
  subtext "#9ea7bf"
  hint "#858fa8"
  accent "#8fb5ff"
  badge "#334875f2"
  danger "#eb9a8f"
  search-icon "" # Empty follows hint.
  icon "" # Empty follows accent.
  alt-hint "" # Empty follows hint.
end

rounding:
  panel 12
  dropdown 12
  search 12
  row 12
  badge 10
  draft 10
end

border:
  enabled true
  width 1
  style "outline" # outline or inset
end

search-icon:
  enabled true
  side "left"
  size 22
end

cursor:
  enabled true
  width 2
  blink-ms 500
  stop-blink-after-ms 5000
end
```

Existing `lift:` configuration paths remain supported, including gathered files.
When both a new path and its old equivalent are present, the new path wins.
For a gathered theme to override the new `colors:` block, update the theme to
use the top-level `colors:` block as well. Lift does not rewrite existing files.

`search.max-results` controls how many results Lift computes. `search.visible-results` sets the
maximum number of rows shown at once. When the compositor grants less space,
Lift shows fewer complete rows; keyboard and wheel navigation scroll through the
full result set.

Long result names and descriptions end with an ellipsis so icons and shortcut
hints stay aligned. Accessible result names retain the full title.

Selection markers reserve space only in cluster search. Normal results start
at `layout.results.padding + layout.row.padding` from the panel's left edge.
`layout.search.padding` controls the search inset. If results padding is omitted,
it inherits the search padding for compatibility with existing configurations.

`layout.row.content-gap` separates the marker, icon, text, and shortcut hint;
`layout.row.text-gap` separates the title and description. `layout.row.cluster-marker-size`
controls the cluster marker (8–32px). `layout.search.icon-gap`,
`layout.draft.padding`, `layout.results.bottom-gap`, and `layout.status.padding`
control their named spacing. All padding and gap settings accept zero. An omitted
`layout.results.bottom-gap` retains the previous `max(row gap, 8)` behavior.
These settings apply on the next Lift invocation.

The highlighted result reveals its full title and description inside their
existing text areas, whether selected with the keyboard or by mouse hover.
By default, Lift waits 900ms, fades in the full text, and scrolls back and forth
at 30 pixels per second with eased starts and stops and a 1200ms pause at each
end. Motion repeats while that row stays highlighted. Changing selection fades
the previous row back to the ellipsis; icons and shortcut hints remain fixed.
Text that fits does not scroll.
Set `text-scroll.enabled false` to disable this. Adjust `text-scroll.delay-ms`,
`text-scroll.speed`, `text-scroll.pause-ms`, and `text-scroll.transition-ms`
to tune it. These settings also apply to keyboard selection. Live title changes
retain scrolling progress for the same node.

Running-node titles follow the application's actual title updates, including
activity spinners. Lift prefers compositor subscriptions; if unavailable or
incompatible, it refreshes through the existing query connection every 100ms
and repaints only changed snapshots. Connection failures retain the last good
state and retry with the existing backoff. Selection follows the same target
when live updates reorder results.

`colors.divider` controls the 1px line between the search box and its connected
dropdown, including "No results" and action errors. Set it to `"#00000000"` to
hide the line. A positive `layout.dropdown.gap` separates the panels and omits the
divider.

App icons are read from `.desktop` `Icon=` entries and sent directly to bounded resolver/decode workers as their rows become visible. Lift never walks the global icon tree or builds an icon index. Worker completion wakes the Wayland event loop immediately, so decoded PNG, JPEG, and SVG icons appear on the next compositor frame. Missing icons fall back to built-in glyphs. `lift.icon-search-depth` remains accepted for config compatibility but is deprecated and ignored.

SVG icons are rasterized directly at `icons.size`, including SVGs authored at a
smaller size. When a theme supplies an undersized bitmap, Lift checks larger
sizes in the same theme lookup before scaling it. Larger bitmaps are reduced
with alpha-aware Lanczos filtering. If an app only supplies a small bitmap,
its available source artwork still limits sharpness.

App discovery searches `XDG_DATA_HOME/applications` first, falling back to
`~/.local/share/applications` when `XDG_DATA_HOME` is empty, unset, or relative.
It then searches `applications` beneath the absolute paths in `XDG_DATA_DIRS`,
in order; an empty or unset list defaults to `/usr/local/share:/usr/share`.
Relative paths are ignored. These follow the
[XDG directory rules](https://specifications.freedesktop.org/basedir/latest/).

The first readable desktop file for an ID takes precedence, including when
`Hidden=true`, `NoDisplay=true`, or an invalid command excludes it from results.
This lets user entries suppress system entries. Nested files use their relative
path with slashes changed to dashes, so `left/shared.desktop` and
`right/shared.desktop` remain separate apps. Lift keeps its existing convention
of omitting `.desktop` from app and cluster IDs. Discovery scans all folder
depths and skips directory symlink cycles.

Desktop app `Exec` values are parsed into arguments using the
[Desktop Entry rules](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html).
Double quotes preserve spaces and empty arguments; string and quoting escapes
are decoded before `%c` (the displayed app name), `%k` (the desktop-file path),
`%i` (`--icon` and the entry's icon), and `%%` (a literal percent) are expanded.
File/URL fields are omitted because Lift launches apps without a file selection.
Malformed commands and unknown field codes are excluded from app results.

Apps launch as an executable and argument list. Missing or inaccessible
executables leave Lift open with an error. This reports spawn failures; it does
not monitor applications for errors after they start. Cluster drafts pass the
same arguments to Halley as a safely quoted shell command.

For `.desktop` apps with `Terminal=true`, `terminal` supplies the executable and
its arguments, with shell-style quoting supported in this prefix. For example,
`kitty -e` opens terminal apps such as `micro` or `nvim` in Kitty. The `term`
search mode continues to run typed commands through your interactive shell.

Mouse support includes hover selection, row click activation, and wheel navigation inside the Lift panel. Typing gives selection authority to the keyboard and resets the highlight to the first result even when the pointer is resting over another row; real pointer motion returns authority to hover selection. Empty general search shows only the rounded search bar; typing or entering a mode prefix expands a connected results body below it. Keyboard navigation supports held direction keys, Left/Right, PageUp/PageDown, Home/End, and Alt+1 through Alt+0 visible-row activation.

Escape closes Lift when the key is released. Lift consumes the complete key
press and release before returning focus, preserving browser video fullscreen
in the window underneath. Holding Escape keeps Lift open until release.

Mouse and accessibility activation verify that a retained row still has the
action and title it was painted with. If live updates replace or reorder a
same-title result, its old row waits for repainting before accepting activation.

Alt-number shortcuts follow the hints on the painted visible rows. Alt+1 opens
the first displayed row after scrolling, and Alt+0 opens the tenth only when
it is visible. With eight rows displayed, Alt+9 and Alt+0 do nothing. Shortcuts
also ignore a row whose target changed before repainting.

## UI and accessibility

Halley UI provides Taffy layout, text shaping, icons, components, and software
drawing. Lift keeps its native Wayland layer surface and renders into the borrowed
BGRA shared-memory buffer; it does not introduce Skia or a second frame-sized copy.
Existing `lift.rune` appearance settings and launcher shortcuts remain supported.

Search editing supports Unicode graphemes, pointer positioning, selection, and
keyboard shortcuts that also work without dedicated Home/End keys:

| Shortcut | Action |
| --- | --- |
| `Ctrl+A` / `Ctrl+E` | Move to the beginning / end of the input |
| `Ctrl+B` / `Ctrl+F` | Move one character left / right |
| `Ctrl+U` / `Ctrl+K` | Delete everything before / after the cursor |
| `Ctrl+W` or `Ctrl+Backspace` | Delete the previous word |
| `Ctrl+Delete` | Delete the next word |
| `Ctrl+Arrow` | Move by word |
| `Shift+Arrow` | Extend the selection |
| `Ctrl+Home` / `Ctrl+End` | Move to the beginning / end of the input |
| `Backspace` / `Delete` | Delete the previous / next character or selected text |

Shift also extends selection with the cursor movement shortcuts, including
`Ctrl+Shift+A` / `Ctrl+Shift+E`. `Ctrl+A` moves the cursor rather than selecting all.
To clear the whole input from any cursor position, use `Ctrl+E` then `Ctrl+U`.
Unmodified arrows and Home/End retain result navigation; Tab retains the Actions
shortcut. Lift does not yet connect Wayland IME composition or clipboard shortcuts.

The Linux accessibility bridge exposes the search field, visible result buttons,
labels, focus, and activation through AT-SPI when desktop accessibility is enabled.
Full screen-reader interaction remains a separate manual validation step.

## Development

```sh
cargo fmt --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

`HALLEY_LIFT_PERF=1 halley-lift` reports startup and frame timings to stderr.

A bounded before/after native drawing comparison is recorded in [the performance notes](docs/performance.md).
