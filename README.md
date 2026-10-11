# Halley Lift

Halley Lift is a standalone command palette for Halley. It is built from the `halley-lift` crate, installs as the `halley-lift` binary, and uses the public `halley-api` SDK for compositor queries, actions, cluster drafts, and live updates.

## Install

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

Lift searches everything by default. Prefixing the query with a provider name filters results without changing the search text into a badge.

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

In `cluster` searches, `Space` stages or unstages the selected app or running node. This is side-effect-free. Outside cluster searches, Space is normal search text.

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

Useful layout keys:

```rune
lift:
  placeholder "Search apps, nodes, clusters, actions..."
  width 760
  max-results 40
  visible-results 8
  icons true
  icon-size 28
  icon-theme "auto"
  icon-search-depth 5
  terminal "x-terminal-emulator -e"
  close-on-focus-loss false
  alt-number-jump true

  position:
    anchor "center" # center | top | top-left | top-right | bottom | bottom-left | bottom-right
    offset-x 0
    offset-y 0
  end

  rounding:
    panel 18
    dropdown 14
    search 12
    row 12
    badge 10
    draft 10
  end

  colors:
    panel "#151720ee"
    panel-border "#2b3248cc"
    dropdown "#151720ee"
    dropdown-border "#2b3248cc"
    search "#090b12d8"
    row-selected "#2e4575ea"
    divider "#2b324899"
    text "#f2f5ff"
    subtext "#9ea7bf"
    hint "#858fa8"
    accent "#8fb5ff"
    badge "#334875f2"
    danger "#eb9a8f"
    search-icon ""  # magnifier tint; empty = follow `hint`
    icon ""         # result-list icons; empty = follow `accent`
    alt-hint ""     # Alt+<n> jump labels; empty = follow `hint`
  end

  border:
    enabled true
    width 1          # thickness in px
    style "outline"  # "outline" wraps the whole app; "inset" borders only the results
  end

  search-icon:
    enabled true
    side "left"      # "left" or "right" of the search text
    size 22
  end

  cursor:
    enabled true
    width 2
    blink-ms 500
    stop-blink-after-ms 5000
  end

  ui:
    top-margin 96
    padding 20
    dropdown-gap 0
    dropdown-padding 10
    search-height 60
    row-height 64
    row-gap 6
    footer-height 0
    font "sans-serif"
    search-font-size 22
    title-font-size 17
    subtitle-font-size 13
    hint-font-size 12
  end
end
```

`max-results` controls how many results Lift computes. `visible-results` sets the
maximum number of rows shown at once. When the compositor grants less space,
Lift shows fewer complete rows; keyboard and wheel navigation scroll through the
full result set.

App icons are read from `.desktop` `Icon=` entries and sent directly to bounded resolver/decode workers as their rows become visible. Lift never walks the global icon tree or builds an icon index. Worker completion wakes the Wayland event loop immediately, so decoded PNG, JPEG, and SVG icons appear on the next compositor frame. Missing icons fall back to built-in glyphs. `icon-search-depth` remains accepted for config compatibility but is deprecated and ignored.

`terminal` is prepended to `.desktop` apps with `Terminal=true`, so terminal apps such as `micro` or `nvim` open in the configured terminal.

Mouse support includes hover selection, row click activation, and wheel navigation inside the Lift panel. Typing gives selection authority to the keyboard and resets the highlight to the first result even when the pointer is resting over another row; real pointer motion returns authority to hover selection. Empty general search shows only the rounded search bar; typing or entering a mode prefix expands a connected results body below it. Keyboard navigation supports held direction keys, Left/Right, PageUp/PageDown, Home/End, and Alt+1 through Alt+0 visible-row activation.


## UI and accessibility

Halley UI provides Taffy layout, text shaping, icons, components, and software
drawing. Lift keeps its native Wayland layer surface and renders into the borrowed
BGRA shared-memory buffer; it does not introduce Skia or a second frame-sized copy.
Existing `lift.rune` appearance settings and launcher shortcuts remain supported.

Search editing supports Unicode graphemes, pointer positioning, selection,
`Ctrl+A`, `Shift+Arrow`, `Ctrl+Arrow`, `Ctrl+Home`, `Ctrl+End`, and `Delete`.
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
