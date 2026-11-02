# Brickify

Turn an SVG or PNG into a 2D LEGO build: a flat mosaic (studs up) or an upright wall.
Exports LDraw (`.ldr`), a coloured 3D model (`.obj`) and a parts list (`.csv`).

## Run the desktop app

```sh
cargo run --release -- example.svg
```

## Run the web version

```sh
trunk serve --release        # then open http://127.0.0.1:8080
trunk build --release        # static site in dist/, host it anywhere
```

Every push to `main` publishes the web version to GitHub Pages
(`.github/workflows/pages.yml`). Turn it on once in Settings → Pages → Source: GitHub Actions.

## Command line (no window)

```sh
brickify IMAGE --width 48 [--layout flat|upright] [--parts plate|brick|hybrid] [--smooth] [--antialias] [--dither] [--no-backing] [--border STUDS] --export out.ldr
```

The export type comes from the extension: `.ldr`, `.obj` or `.csv`.

## How it works

1. The image is rendered (SVGs at 2048 px) and sampled over a grid, one cell per stud.
   Upright walls use cells 1.2× (bricks) or 0.4× (plates) as tall as they are wide.
2. Cells with less than the "min. coverage" of opaque pixels stay empty. Other cells get
   the nearest enabled LEGO colour (in CIE Lab space):
   - By default each cell takes its dominant colour, so edges stay crisp.
   - "Antialiasing" averages each cell instead, and blends edge cells towards the
     background fill or border, for the look of finer detail.
   - "Dithering" spreads colour error to neighbouring cells (Floyd-Steinberg), so mixes of
     nearby colours approximate gradients. It only applies in gradients, so solid areas
     stay solid. It makes many single-stud parts: upright walls with a border hold them, but
     a flat mosaic's backing plates can't hold them all.
3. Same-coloured cells are merged into the largest parts that fit. Upright walls avoid seams
   directly above the seams below, and can get a hidden back layer that bridges colour
   changes. Flat mosaics get an optional plate backing layer placed so its plates span the
   picture layer's seams. A repair pass then re-tiles any spot where parts don't connect.
4. An optional border outlines the picture. In an upright wall, parts grip the rows above
   and below, so the border holds loose parts and colour changes together without a back
   layer. In a flat mosaic it is only a frame: parts side by side don't grip each other.
5. Hybrid walls use plate-height rows grouped into bands of three. Where a column's three
   cells in a band share a colour, one brick replaces three plates; plates keep the detail
   elsewhere. That needs about 15-40% fewer pieces than plates for the same picture. Use a
   border to hold a hybrid wall together (the back layer isn't available in hybrid mode).
6. "Smooth top" swaps plates for tiles where nothing sits on them: the whole picture layer of a
   flat mosaic, or every uncovered plate of a wall (when a tile that size exists).
7. The sidebar reports whether every part is connected; loose parts are outlined in red.
8. Open the `.ldr` file in Studio, LeoCAD or LDView to see the real parts and build steps.

## Source layout

| File | Purpose |
|---|---|
| `src/source.rs` | SVG / PNG / JPEG loading |
| `src/palette.rs` | LDraw colours and colour matching |
| `src/mosaic.rs` | Grid sampling and part merging |
| `src/geometry.rs` | 3D triangles for the preview and OBJ |
| `src/export.rs` | LDraw, OBJ and CSV writers |
| `src/view3d.rs` | The 3D preview |
| `src/app.rs` | The user interface |
| `src/platform.rs` | Open/save on desktop and in the browser |
