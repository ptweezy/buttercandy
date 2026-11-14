# Third-party material

buttercandy is Copyright (C) 2026 Parker Loflin and licensed under the GNU
Affero General Public License, version 3 (see [LICENSE](LICENSE)). It also
contains a few short pieces adapted from other people's published work,
follows some published methods, and is built on libraries with licences of
their own.

## Adapted code

- **Hash functions** (`hash21`, `hash22` in
  `src/gfx/shaders/preset_header.wgsl`, and the hashes in `post.wgsl` and
  `presets/gumdrop.wgsl`) are adapted from "Hash without Sine" by David
  Hoskins, <https://www.shadertoy.com/view/4djSRW>. Copyright (c) 2014 David
  Hoskins. MIT License (below).
- **Whole-number hash** (`ih` in `src/gfx/shaders/presets/sugarplum.wgsl`,
  repeated in `src/gfx/sugarplum.rs`) is the PCG output permutation used as
  a hash, as in "Hash Functions for GPU Rendering" by Mark Jarzynski and Marc
  Olano, Journal of Computer Graphics Techniques 9(3), 2020, with the
  constants of the PCG random number generators,
  <https://www.pcg-random.org>. Copyright 2014-2019 Melissa O'Neill and the
  PCG Project contributors, who offer it under the Apache License 2.0 or the
  MIT License (below).
- **Five-pointed star distance** (`sd_star5` in
  `src/gfx/shaders/presets/sprinkles.wgsl`) is adapted from the 2D distance
  functions by Inigo Quilez,
  <https://iquilezles.org/articles/distfunctions2d/>. Copyright (c) Inigo
  Quilez. MIT License (below).
- **Tone mapping** (`aces` in `src/gfx/shaders/post.wgsl`) is the ACES filmic
  curve fit by Krzysztof Narkowicz,
  <https://knarkowicz.wordpress.com/2016/01/06/aces-filmic-tone-mapping-curve/>,
  which its author offers under public domain CC0 or the MIT License.

### MIT License

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## Methods

The fractal presets copy no one's code, but they rest on methods that other
people worked out and published:

- **Deep zooms of the Mandelbrot set** (`src/gfx/jawbreaker.rs` and
  `src/gfx/shaders/presets/jawbreaker.wgsl`). Iterating each pixel's
  difference from one reference orbit, and skipping the first steps with a
  series, are from K. I. Martin, "Superfractalthing Maths" (2013). Starting
  the reference orbit again when a pixel comes nearer to zero than to it
  ("rebasing") is from Zhuoran, "Another solution to perturbation glitches"
  (2021),
  <https://fractalforums.org/fractal-mathematics-and-new-theories/28/another-solution-to-perturbation-glitches/4360>.
- **Finding miniature copies of the set** (`src/gfx/jawbreaker.rs`). The
  period of a box of the plane is found as in the mandelbrot-numerics library
  by C. Heiland-Allen, <https://code.mathr.co.uk/mandelbrot-numerics>. The
  estimate of a copy's size is the one explained by E. Demidov,
  <https://ibiblio.org/e-notes/MSet/windows.htm>, as set out by R. P. Munafo
  in Mu-Ency, <https://www.mrob.com/pub/muency/stabilitywindow.html>.
- **The hall of pearls** (`src/gfx/shaders/presets/gingerbread.wgsl`) uses
  the fold-and-invert rule of the Mandelbox, by T. Lowe (2010), in the form
  known as pseudo-Kleinian, which came out of work by Theli-at and Knighty on
  Fractal Forums (2011).
- **The packing of balls** (`src/gfx/shaders/presets/gumball.wgsl`) is built
  on the chain of circles in H. S. M. Coxeter, "Loxodromic sequences of
  tangent spheres", Aequationes Mathematicae 1 (1968), 104-121, and steps
  from ball to ball by the complex Descartes theorem of J. C. Lagarias,
  C. L. Mallows and A. R. Wilks, "Beyond the Descartes circle theorem",
  American Mathematical Monthly 109 (2002), 338-361.

## Libraries

buttercandy links Rust crates published under the MIT, Apache-2.0, Zlib, BSD,
ISC, Unicode and similar permissive licences, and the Symphonia audio
decoders under the Mozilla Public License 2.0. None of their source is in
this repository; `Cargo.lock` lists the exact versions, and

```bash
cargo tree --format "{p} {l}"
```

prints each one with its licence. Symphonia's source is at
<https://github.com/pdeljanov/Symphonia>.
