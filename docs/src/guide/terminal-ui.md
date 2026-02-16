# Terminal UI

Lambdaform includes an optional terminal dashboard powered by [ratatui](https://github.com/ratatui/ratatui).

## Enabling the TUI

```bash
lambdaform start --tui
```

> **Note:** The TUI feature requires the `tui` cargo feature flag (enabled by default in release builds).

## Dashboard Layout

The terminal UI shows:

- **Server info** — port, discovered functions, routes
- **Live request log** — color-coded by HTTP method and status:
  - 🟢 `2xx` responses in green
  - 🟡 `4xx` responses in yellow
  - 🔴 `5xx` responses in red
  - Methods color-coded: GET (cyan), POST (green), PUT (yellow), DELETE (red)
- **Timing** — response time for each request
- **Function** — which Lambda handled the request

## Keyboard Controls

| Key | Action |
|-----|--------|
| `q` / `Ctrl-C` | Quit |
| `↑` / `↓` | Scroll request log |
| `Home` / `End` | Jump to top/bottom |

## When to Use

The TUI is great for:
- **Demos** — visual feedback during presentations
- **Debugging** — see request flow in real time
- **Development** — monitor which functions handle which routes

For CI/CD or scripting, use `--json-log` instead.
