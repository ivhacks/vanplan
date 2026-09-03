# vanplan

Live at https://vanplan.lol

Blank-canvas DAG editor. Cards with directed dependencies; layout is computed.

## Run

Needs Rust (`rustup`) with `wasm32-unknown-unknown` and [Trunk](https://trunkrs.dev/).

```bash
cd /workspace/vanplan && trunk serve
```

Then open the URL Trunk prints (usually `http://127.0.0.1:8080`).

## Use

- Double-click empty canvas: add a card, type
- Click the small swatch on a card: cycle color
- Drag from either circle onto another card: left means this task depends on the other; right means this task is a prereq for the other. Cycles are rejected.
- Click a wire: delete it
- Hover a card, click × — or Delete/Backspace when the card is selected (not while typing)
- Ctrl/Cmd-S: save `.vanplan` YAML
- Ctrl/Cmd-O: open `.vanplan` YAML

No dragging. Cards auto-arrange left-to-right (prerequisites left, dependents right).

## Session URL

The document *is* the URL hash: `#vp1.<base64url(gzip(yaml))>`. Bookmark, paste, or share that address; back/forward loads it. Agents mint links without opening the page:

```bash
./scripts/vanplan-url encode plan.vanplan          # prints https://vanplan.lol/#vp1....
./scripts/vanplan-url decode 'https://vanplan.lol/#vp1....'
```

Ctrl/Cmd-S and Ctrl/Cmd-O still save and open YAML files.
