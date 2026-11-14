// Jawbreaker: an endless fall into the Mandelbrot set, drawn as a sweet cut
// open: layer inside layer round the set, in carved relief.
//
// The fall is real, a hundred thousand million times deep and more, which
// 32-bit floats cannot follow on their own. So this shader does not iterate
// each pixel's point of the plane. Its driver (src/gfx/jawbreaker.rs) picks
// a miniature copy of the set to fall to and works out the orbit of that
// copy's centre in 64 bits; the shader iterates only the *difference*
// between the pixel's orbit and that one, which stays accurate however small
// it gets. The driver also says where to start: the first steps move every
// pixel of the view almost alike, so they are a cubic in the pixel's
// position and are skipped. When the copy has grown into a small jewel at
// the centre, the picture dissolves from it to the whole set drawn in its
// place (they are the same shape), and the fall goes on from the top
// towards the next copy.
//
// What is drawn is the distance to the set. Layers sit at equal steps of its
// logarithm, so each is as thick as it is far from the set, at every depth:
// terraces, each with a riser on its inner side up to the next. Nearer the
// set than a few pixels the terraces melt into a glaze, and the set itself
// is dark.
//
// A pixel needs hundreds of steps, which is too much to do for every pixel
// of every frame, and what it finds does not depend on the music. So this
// preset keeps state (see preset_header.wgsl). state() works out each
// pixel's place in the set (how far from it, how near one of its rays, how
// long it took to leave) and keeps it, and above sixty frames a second it
// works out only a scatter of small tiles afresh each frame, the rest
// keeping what they had, moved to where the fall has carried it. show() then
// paints every pixel of every frame from what is kept, so the voices are
// never out of date.
//
// The five voices (see preset_header.wgsl and fractal.wgsl). Depth is pitch,
// and here depth is nearness to the set: a screen away at depth 0, a pixel
// away at depth 1.
//
//   voice  what it does                         where                colour
//   -----  -----------------------------------  -------------------  ------
//   KICK   a ring (kickRing) runs out from the  the whole picture,   KICK
//          centre, lighting the relief it       from the centre
//          passes, and the fall is pushed on a
//          little (in the driver)
//   SNARE  the sweet cracks (snareEnv):         a swath across the   SNARE
//          fracture lines shoot through the     picture, one of
//          layers, running in from the edge     twelve, a new one
//          and forking as they near the set     each hit; in the
//          like lightning, then are gone        middle layers
//   HATS   a tick (hatEnv): the filaments       the glaze, towards   HATS
//          glint at a scatter of points, a new  the edge
//          scatter each tick
//   BASS   swells with uBassLine: a glow hugs   the body of the set  BASS
//          the set, wider the more bass is      and what lies
//          held, and fills its body             nearest it
//   TUNE   the terraces at one distance from    a band of terraces   TUNE
//          the set light up, nearer the set as  round the set
//          the pitch rises (tuneBand); a new
//          note plucks them brighter
//
// The snare's cracks are the set's own: each is a path that runs from far
// away to the set without ever crossing a layer askew (an external ray), and
// the ones drawn are those whose angle is a whole number of halvings, so two
// become four become eight on the way in.
//
// Scenery, in pal() colours and dimmer than anything a voice lights:
//
// - The terraces, alternately light and dark, a step round the palette each,
//   lit from the upper left; the glaze, whose colour is how long its pixels
//   took to leave, so that the fall passes through the palette; the dark
//   body.
// - Fullness (uEnergy): how deeply the terraces are carved. A breakdown is
//   nearly flat; a full mix is cut deep and glossy.
// - Harmony (uHarmony): turns the palette.
// - Idle: no events and the voices' colours are put away; the fall goes on
//   at its slowest.
//
// Reactivity (uReact): the envelopes carry it. Besides them the fall, the
// kick's ring and its push are smaller or slower below 1, and the bass's
// glow is narrower.

// Steps a pixel may take after the skipped ones. A pixel that needs more is
// drawn as glaze. (The driver's search counts on this number: STEPS in
// jawbreaker.rs.)
const MAXIT: i32 = 640;
// Terraces to each e-fold of distance from the set.
const SHELLS: f32 = 2.1;

// One view of the plane, as the driver hands it over: sixteen numbers.
struct View {
  // The nucleus, in screen units from the centre of the picture, turned
  // into the plane's own directions; plane units to a screen unit; the turn.
  off: vec2f,
  s: f32,
  th: f32,
  // log(s), which s itself cannot give accurately once the terraces of two
  // depths are compared.
  ls: f32,
  // The reference orbit: its period, the step to start at, and where it is
  // in the driver's table.
  period: i32,
  m0: i32,
  base: i32,
  // Where a pixel u screen units from the nucleus starts from, after the
  // skipped steps: a u + b u^2 + c u^3 away from the reference.
  a: vec2f,
  b: vec2f,
  c: vec2f,
  // How many steps were skipped.
  skipped: f32,
}

fn viewAt(i: i32) -> View {
  let d0 = uDrive[i];
  let d1 = uDrive[i + 1];
  let d2 = uDrive[i + 2];
  let d3 = uDrive[i + 3];
  return View(d0.xy, d0.z, d0.w, d1.x, i32(d1.y), i32(d1.z), i32(d1.w), d2.xy, d2.zw, d3.xy, d3.z);
}

struct Orbit {
  // Did it leave?
  out: bool,
  // Where it had got to, and the slope of that against the pixel's place on
  // screen.
  z: vec2f,
  D: vec2f,
  // Steps taken, the skipped ones included.
  n: f32,
}

// Follow the pixel at ps. (This loop is repeated in src/gfx/jawbreaker.rs,
// which tests it against plain 64-bit iteration: change them together.)
fn iterate(v: View, ps: vec2f) -> Orbit {
  let u = v.off + rot(v.th) * ps;
  // Its start, and the slope of that, from the driver's series.
  var dz = cmul(u, v.a + cmul(u, v.b + cmul(u, v.c)));
  var D = v.a + cmul(u, 2.0 * v.b + 3.0 * cmul(u, v.c));
  let dc = v.s * u;
  var m = v.m0;
  var zm = driveData(v.base + m).xy;
  var full = zm + dz;
  var out = false;
  var i = 0;
  for (; i < MAXIT; i++) {
    if (dot(full, full) > 1e6) {
      out = true;
      break;
    }
    // A slope this steep means the set is less than a hundredth of a pixel
    // away: glaze, and no need to go on.
    if (dot(D, D) > 1e11) { break; }
    D = 2.0 * cmul(full, D) + vec2f(v.s, 0.0);
    // One step of the difference from the reference orbit.
    dz = cmul(dz, 2.0 * zm + dz) + dc;
    m += 1;
    if (m >= v.period) { m = 0; }
    zm = driveData(v.base + m).xy;
    full = zm + dz;
    // Once the orbit itself is smaller than its difference from the
    // reference, start the reference again from its beginning, where it is 0.
    if (dot(full, full) < dot(dz, dz)) {
      dz = full;
      m = 0;
      zm = vec2f(0.0);
    }
  }
  return Orbit(out, full, D, v.skipped + f32(i));
}

// What is kept of a pixel, four numbers, each of which can be blended with
// its neighbours' (so that what is kept can be moved by a fraction of a
// pixel as the view falls):
//
//   x  the log of its distance from the set, in screen units (0 at the size
//      of the picture, about -6.5 at a pixel), or NEAR where the set is
//      nearer than can be told
//   y  how far it is from the nearest of the set's rays that the snare
//      cracks along, 0 on the ray to 0.5 half way to the next
//   z  how long it took to leave, in turns of the palette
//   w  0 outside the set; inside its body, 0.5 deep within to 1 at its edge
const NEAR: f32 = -10.0;

// A pixel's place, and the direction on screen away from the set.
struct Place {
  g: vec4f,
  away: vec2f,
}

fn place(v: View, ps: vec2f) -> Place {
  let o = iterate(v, ps);
  if (!o.out) {
    // A copy of the set too small to make out is part of the glaze round
    // it: its slope is steep.
    let steep = log(max(length(o.D), 1e-9));
    if (steep < log(100.0)) {
      // The body's edge is where the slope climbs.
      return Place(vec4f(NEAR, 0.5, 0.0, 0.5 + 0.5 * sstep(log(0.3), log(30.0), steep)), vec2f(1.0, 0.0));
    }
    return Place(vec4f(NEAR, 0.5, 0.004 * o.n, 0.0), vec2f(1.0, 0.0));
  }
  let m = dot(o.z, o.z);
  let d = sqrt(m) * 0.5 * log(m) / max(length(o.D), 1e-20);
  let took = o.n + 1.0 - log2(0.5 * log(m) / log(1000.0));
  return Place(
    vec4f(
      clamp(log(max(d, 1e-9)), NEAR + 0.5, 2.0),
      abs(atan2(o.z.y, o.z.x)) / TAU,
      0.004 * max(took, 0.0),
      0.0,
    ),
    normalize(rot(-v.th) * cdiv(o.z, o.D)),
  );
}

// How far a terrace this far from the set is wide enough, in pixels, to be
// drawn as one: 0 in the glaze, 1 on the terraces.
fn fineness(lam: f32, px: f32) -> f32 {
  return sstep(2.0, 7.0, exp(lam) / px);
}

fn state(uv: vec2f) -> vec4f {
  let px = 2.0 / uRes.y;
  let at = aspect(uv);

  // Where this pixel was in the frame before, which the driver works out.
  let w = uDrive[8];
  let was = unaspect(vec2f(w.x * at.x + w.y * at.y, w.x * at.y - w.y * at.x) + w.zw);
  // Nothing is known of it if it was off the picture, or if the driver says
  // that what is kept is of somewhere else.
  let seen = all(was > vec2f(0.0)) && all(was < vec2f(1.0)) && uDrive[9].y < 0.5;
  var old = vec4f(0.0);
  if (seen) {
    old = kept(was);
    // The view has shrunk since then, so everything is that much further
    // from the set, in screen units.
    if (old.x > NEAR + 0.25) {
      old.x -= log(length(w.xy));
    }
  }

  // Above sixty frames a second only some of the picture is worked out
  // afresh each frame, a new scatter of small tiles every time (tiles,
  // because a GPU runs neighbouring pixels in step, and one busy pixel would
  // hold its idle neighbours up): each tile about once in a fortieth of a
  // second, whatever the frame rate. The rest keep what they had.
  let share = clamp(0.7 * FR, 0.15, 0.7);
  let turn = hash21(floor(uv * uRes / 8.0) + vec2f(fract(uTime * 5.73) * 631.0, fract(uTime * 2.39) * 877.0));
  if (seen && turn > share) {
    return old;
  }

  // In the glaze a pixel holds more than one sample can say: there each
  // sample is taken somewhere new within the pixel and laid over what was
  // kept. On the terraces one sample at the middle is exact.
  let rough = select(1.0, 1.0 - fineness(old.x, px), seen);
  let jitter = hash22(vec2f(fract(uTime * 7.31) * 913.0, fract(uTime * 3.17) * 517.0)) - 0.5;
  let now = place(viewAt(0), at + jitter * px * rough).g;
  let keep = select(0.0, 0.45 * rough * (1.0 - fineness(now.x, px)), seen);
  return mix(now, old, keep);
}

// One terrace's colour, and how much of it faces the lamp and catches it.
struct Tread {
  col: vec3f,
  lambert: f32,
  gloss: f32,
}

// Terrace `idx`, at `b` of the way across it from its inner edge (0, where
// its riser climbs to the next) to its outer (1).
fn tread(idx: f32, b: f32, away: vec2f, carve: f32) -> Tread {
  let riser = 1.0 - sstep(0.0, 0.3, b);
  let slope = (0.12 + 3.4 * riser * (1.0 - riser) + 0.9 * riser) * carve;
  let nrm = normalize(vec3f(away * slope, 1.0));
  let L = normalize(vec3f(-0.5, 0.62, 0.6));
  let lambert = max(dot(nrm, L), 0.0);
  let gloss = pow(max(dot(nrm, normalize(L + vec3f(0.0, 0.0, 1.0))), 0.0), 60.0);
  // The foot of each riser sits in shadow.
  let foot = exp(-pow((b - 0.34) / 0.07, 2.0));
  let tone = mix(0.16, 0.34, fmod(idx, 2.0));
  let col = pal(idx * 0.137 + uHarmony + 0.3) * tone * (0.25 + 1.0 * lambert) * (1.0 - 0.55 * foot);
  return Tread(col, lambert, gloss);
}

// The picture at ps, from what is kept of it and the direction away from the
// set. `ls` is the log of the view's scale, which sets where the terraces
// fall.
fn paint(g: vec4f, away: vec2f, ls: f32, ps: vec2f, px: f32) -> vec3f {
  let r = length(ps);
  let ring = kickRing(r);
  let bass = bassOn();
  let full = mix(sstep(0.35, 0.9, uEnergy), 0.6, uIdle);

  if (g.w > 0.25) {
    // The body of the set: dark, with the bass glowing at its edge.
    let edge = clamp(2.0 * g.w - 1.0, 0.0, 1.0);
    return pal(uHarmony + 0.6) * 0.008 + voiceCol(BASS) * bass * fracHeft() * (0.03 + 0.2 * edge * edge)
      + voiceCol(KICK) * ring * 0.08;
  }

  let lam = g.x;
  let d = exp(lam);
  let de = d / px;
  let fine = fineness(lam, px);

  // Terraces. A pixel on the edge between two is part one, part the other.
  let sh = (lam + ls) * SHELLS;
  let idx = floor(sh);
  let b = sh - idx;
  let per = SHELLS / max(de, 1e-3);
  let carve = fine * mix(0.55, 1.1, full);
  let here = tread(idx, b, away, carve);
  var next = tread(idx + 1.0, 0.0, away, carve);
  var cover = clamp(0.5 + (1.0 - b) / per, 0.0, 1.0);
  if (b < 0.5) {
    next = tread(idx - 1.0, 1.0, away, carve);
    cover = clamp(0.5 + b / per, 0.0, 1.0);
  }
  var col = mix(next.col, here.col, cover);
  let lambert = mix(next.lambert, here.lambert, cover);
  let gloss = mix(next.gloss, here.gloss, cover);

  // The glaze is brightest a pixel or so out, and darker in the thick of it.
  // Its colour is how long the pixel took to leave, which climbs slowly all
  // the way down, so the fall passes through the palette.
  let thick = sstep(-3.0, 0.7, lam - log(px));
  col = mix(pal(uHarmony + 0.75 + 0.1 * thick + g.z) * (0.07 + 0.34 * thick), col, fine);
  col += vec3f(1.0) * gloss * 0.22 * fine * mix(0.5, 1.0, full);
  // The filaments themselves, lit from within.
  let wire = exp(-de / 1.2);
  col += pal(uHarmony + 0.9) * wire * 0.18 * thick;

  // The scenery keeps under the voices.
  col *= 0.6;

  // What a voice lights: the faces turned to the lamp, the lips and the wire.
  let glow = 0.14 + 0.7 * lambert * lambert * fine + 2.0 * gloss + 0.25 * wire * thick;

  // Depth is pitch: a screen away from the set at 0, a pixel away at 1.
  let depth = (-lam - 0.2) * 0.1818;

  // BASS: the glow that hugs the body, wider the more bass is held.
  let hug = exp(-d / (0.02 * (0.6 + fracHeft()))) * bass;
  col = col * (1.0 - 0.45 * hug) + voiceCol(BASS) * hug * (0.10 + 0.8 * glow);

  // TUNE: the terraces at one distance from the set.
  let sing = tuneBand(depth, 0.073);
  col = col * (1.0 - 0.7 * min(sing, 1.0)) + voiceCol(TUNE) * sing * (0.2 + 1.8 * glow);

  // KICK: the ring lights the relief it passes.
  col = col * (1.0 - 0.5 * ring) + voiceCol(KICK) * ring * (0.1 + 1.7 * glow);

  // SNARE: the sweet cracks. The cracks are the set's rays, drawn where this
  // pixel lies on one: its place between the rays changes by about
  // 1.6 / (its distance from the set) a pixel, a little more on the inner
  // side of each step of the escape time than the outer, so a crack tapers.
  let swath = snareSwath(ps, 0.3);
  if (swath > 0.002) {
    let off = g.y * 0.63 * de;
    let crack = clamp(1.3 - off, 0.0, 1.0) + 0.3 * exp(-off / 3.5);
    // In the middle layers: not out where the layers are a screen wide, and
    // not close in, where the rays (0.63 of the distance to the set apart)
    // crowd into a fringe. Nothing but the cracks is lit.
    let middle = sstep(0.02, 0.16, depth) * sstep(9.0, 30.0, de);
    col += voiceCol(SNARE) * swath * middle * crack * 3.4;
  }

  // HATS: the filaments glint at a scatter of points round the edge.
  let gp = ps * 7.0;
  let cell = floor(gp);
  var glint = 0.0;
  for (var k = 0; k < 4; k++) {
    let en = hatEnv(k);
    if (en < 0.003) { continue; }
    let h = hash22(cell + 3.7 * (uHatN - f32(k)));
    if (h.x > 0.7) {
      let o = fract(gp) - 0.25 - 0.5 * vec2f(h.y, fract(h.x * 57.0));
      glint += en * exp(-dot(o, o) * 40.0);
    }
  }
  col += voiceCol(HATS) * glint * exp(-de / 2.5) * sstep(0.3, 0.8, r) * 5.0;

  return col;
}

fn show(uv: vec2f) -> vec3f {
  let px = 2.0 / uRes.y;
  let ps = aspect(uv);
  let at = vec2i(uv * uRes);
  let g = textureLoad(tPrev, at, 0);
  // Which way is away from the set: up the slope of the distance, taken
  // across a few pixels each way, and more of them far from the set, where
  // the distance changes too slowly for what is kept to tell next-door
  // neighbours apart.
  let reach = i32(clamp(exp(g.x) / px / 12.0, 4.0, 16.0));
  let last = vec2i(uRes) - 1;
  let least = g.x - 2.0;
  let e = max(textureLoad(tPrev, min(at + vec2i(reach, 0), last), 0).x, least);
  let wst = max(textureLoad(tPrev, max(at - vec2i(reach, 0), vec2i(0)), 0).x, least);
  let up = max(textureLoad(tPrev, max(at - vec2i(0, reach), vec2i(0)), 0).x, least);
  let dn = max(textureLoad(tPrev, min(at + vec2i(0, reach), last), 0).x, least);
  let away = normalize(vec2f(e - wst, up - dn) + vec2f(1e-7, 0.0));

  var col = paint(g, away, uDrive[1].x, ps, px);
  // The dissolve to the next fall, where the copy at the centre is the
  // whole set. The whole set from this far off is quick to work out, so it
  // is worked out here and not kept.
  let next = uDrive[9].x;
  if (next > 0.0) {
    let there = place(viewAt(4), ps);
    col = mix(col, paint(there.g, there.away, uDrive[5].x, ps, px), next);
  }
  return col;
}
