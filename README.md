# MindMap

A portable mind-mapping canvas for Linux. Each canvas starts as one circle. Branch out from that circle, link ideas, and open any idea as a map of its own.

## Build

Install Rust (1.88 or newer) and the usual X11 and OpenGL build libraries. On Linux Mint:

```bash
sudo apt install build-essential pkg-config libx11-dev libxcb1-dev libxkbcommon-dev libgl1-mesa-dev libxcursor-dev libxi-dev libxrandr-dev libxfixes-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
./build.sh
```

`./build.sh` writes a single program to `dist/mindmap`. Run that file:

```bash
./dist/mindmap
```

While developing:

```bash
cargo test
cargo run
```

## Where maps are stored

The program keeps an unencrypted folder named `mindmap-data` in the same directory as the executable.

```text
dist/mindmap
dist/mindmap-data/
  manifest.json
  canvases/<id>.json
```

Every change is written immediately. The JSON is plain text. Copy the program and that folder together to move your maps.

## Using the canvas

- **New canvas** starts a map. Its circle sits in the middle.
- **Ctrl+click** the circle or a rectangle to add a connected rectangle. A short Ctrl+drag that ends on empty space does the same.
- **Ctrl+drag** from one rectangle to another to connect them.
- **Click** a rectangle to select it. The panel shows when it was created and when it, or anything inside it, was last changed. From there you can edit, close, open, or delete it.
- **Close** greys out that rectangle and every rectangle branched from it. Closed ideas cannot be edited. Open the rectangle that was closed to work on it again.
- **Delete** removes that rectangle and every line touching it. Rectangles that branched from it stay on the map, detached, and are arranged underneath. A nested map that belonged to the deleted rectangle is kept as its own canvas in the sidebar.
- **Double-click** a rectangle or the centre circle to edit its text, unless it is closed. Ctrl+Enter finishes the edit, and so does a click outside the text. Escape cancels.
- **Shift+click** an idea to open a new canvas whose centre uses that idea's wording. Shift+click again to go deeper. **Back** returns to the parent canvas, centred on the rectangle you opened.
- **Arrow keys** pan. **Space** centres the circle. **+** and **−** zoom. **=** fits the whole map. The scroll wheel zooms too, and dragging empty canvas pans.
- A circle that has not been modified for **7 days** turns yellow, for **30 days** orange, and for **182 days** (about six months) red. Modification includes edits and additions inside the idea, including nested maps.
- **Needs attention** lists every yellow, orange, and red circle.

Ctrl+Z undoes the last change.
