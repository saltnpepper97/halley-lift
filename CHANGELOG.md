# Changelog

## Unreleased

- Reconnect live node and cluster updates after API disconnects or event sequence
  gaps, replacing the cache from a fresh snapshot. Retry failed initial loads
  and refresh query results when subscriptions are unavailable.
- Serialize single-instance socket ownership through startup and shutdown, and
  only remove the socket owned by the exiting instance. Recover stale sockets
  after crashes without deleting replacement sockets or unrelated files.
- Keep compositor connection setup out of joined startup work and run action
  requests off the UI thread. Give handshake/action connections two-second
  read/write timeouts and ignore repeat activation while an action is pending.
- Keep painting at the compositor's granted size on small or scaled outputs,
  including when a larger dropdown cannot fit. Show fewer complete rows instead
  of squeezing their text, and wait for resize processing before painting new content.
- Clip backdrop blur to Lift's painted shape, including rounded corners and
  transparent dropdown gaps, using the optional Wayland background-effect protocol.

## [0.3.0](https://github.com/saltnpepper97/halley-lift/releases/tag/v0.3.0)

- Move Lift from the Halley workspace into its own repository and release cycle.
- Replace custom rasterization, font handling, and layout with Halley UI and Taffy.
- Keep direct Wayland shared-memory buffers, providers, cluster drafts, icon workers,
  configuration, pointer behavior, and launcher shortcuts.
- Add Unicode search editing, selection, and an optional desktop accessibility bridge.
- Use the published Halley API 0.4 and document standalone installation.

## [0.2.1](https://github.com/saltnpepper97/halley/releases/tag/halley-lift-v0.2.1)

- Restore and harden Freedesktop app-icon lookup and caching.
- Dismiss on outside clicks while allowing the underlying click through.
- Improve search-text, caret, and glyph alignment.
- Bundle a font-independent cluster selection marker.
- Accompany Halley 0.7.0 while retaining the public Halley API 0.3.

## [0.2.0](https://github.com/saltnpepper97/halley/releases/tag/halley-lift-v0.2.0)

- Port compositor queries, actions, cluster drafts, and live updates to the public
  Halley API 0.3 SDK; target Halley 0.6.
- Resolve generic system font families such as `sans-serif` correctly.
- Keep the first result selected while typing, and require real pointer motion
  before hover can change the selection.

## [0.1.0](https://github.com/saltnpepper97/halley/tree/halley-lift-v0.1.0/crates/halley-lift)

- Provide a self-contained Wayland shared-memory launcher for apps, running
  nodes, clusters, actions, configuration files, and terminal commands.
- Add provider search prefixes, keyboard and pointer navigation, and ranking
  from Halley's Field/Bearings pins.
- Stage apps and running nodes in cluster drafts, then hand creation to the
  compositor's Cluster Finalize popup.
- Bootstrap a documented Rune configuration without overwriting existing files;
  expose layout, colors, borders, search glyphs, and caret settings.
- Support PNG, JPEG, and SVG app icons and commands through the configured
  terminal and interactive shell.

The 0.1.0 through 0.2.1 entries are backfilled from the Halley repository's tagged
source and release notes. Those releases remain available from its
`halley-lift-*` tags.
