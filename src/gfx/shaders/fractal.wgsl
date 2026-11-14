// Shared by every preset of the Fractals genre, on top of preset_header.wgsl.
//
// A fractal has something to show at every scale, and the genre gives the
// five voices their places along that one axis. Depth is pitch:
//
//   KICK   the coarsest things in the picture, from the centre.
//   BASS   the body: the biggest or lowest mass.
//   TUNE   one band of scales, which slides finer as the pitch rises.
//   SNARE  light in the joints: for an instant the cracks, contacts or seams
//          where the pieces of the fractal meet show, in a swath straight
//          across the picture, at the middle scales.
//   HATS   the finest things the picture can draw, towards the edge.
//
// Each preset measures scale its own way (the size of a ball, the distance
// from the set, the height up a pillar) and puts it on a `depth` axis, 0 at
// the coarsest and 1 at the finest. The helpers here then place the voices
// the same way in all of them.
//
// Two more rules of the genre. The shape of a fractal follows only the slow
// signals (uHarmony, uEnergy, uCentroid, uBassLine, uPitch): a hit lights
// what is there and nudges the view, and never reshapes it, because a small
// change to a fractal's numbers moves the whole picture. And every preset
// falls slowly inward for ever, with the kick a push into the fall.

fn cmul(a: vec2f, b: vec2f) -> vec2f {
  return vec2f(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

fn cdiv(a: vec2f, b: vec2f) -> vec2f {
  return vec2f(a.x * b.x + a.y * b.y, a.y * b.x - a.x * b.y) / dot(b, b);
}

fn cross2(a: vec2f, b: vec2f) -> f32 {
  return a.x * b.y - a.y * b.x;
}

// Reactivity, for what is not one of the shared envelopes (which carry it
// already): how fast whatever always moves goes, how big a thing a hit sets
// off is, and how much weight the bass puts on (1 at the default setting).
fn fracPace() -> f32 {
  return mix(0.5, 1.0, uReact);
}

fn fracAmp() -> f32 {
  return mix(0.35, 1.0, uReact);
}

fn fracHeft() -> f32 {
  return mix(0.67, 1.33, uReact);
}

// The kick's push into the fall, 0..1: a preset adds a small multiple of
// this to its depth. Kept small, since it moves every pixel at once.
fn divePush() -> f32 {
  return kickSum();
}

// BASS: how much held bass there is to show, 0..1. A little low end left
// between the kicks is not a bass line, so the bottom of the range is
// pressed down. Nothing while idle.
fn bassOn() -> f32 {
  return sstep(0.08, 0.85, uBassLine) * (1.0 - uIdle);
}

// TUNE: how strongly the tune lights the scale `depth` (0 coarsest, 1
// finest). The band sits at 0.2 for the lowest notes and 0.8 for the
// highest, `width` wide, and a new note plucks it brighter.
fn tuneBand(depth: f32, width: f32) -> f32 {
  let x = (depth - mix(0.2, 0.8, uPitch)) / width;
  return exp(-x * x) * sstep(0.05, 0.4, uTune) * (0.45 + 0.9 * noteSum()) * (1.0 - uIdle);
}

// KICK: a ring that runs out from the centre of the picture, at radius `r`
// (aspect units). It lights what it passes; it does not move it.
fn kickRing(r: f32) -> f32 {
  var ring = 0.0;
  for (var k = 0; k < 4; k++) {
    let a = uKickAges[k];
    let x = (r - 0.05 - 1.4 * fracPace() * a) / 0.2;
    ring += exp(-x * x) * exp(-a * 2.0) * uKickHits[k];
  }
  return min(ring * fracAmp(), 1.0);
}

// SNARE: each hit strikes along a line straight across the picture through
// the centre, at one of twelve angles, a new one each time. The line itself
// is never drawn. What shows is whatever joints of the fractal lie in a swath
// along it: how strongly the snare lights a joint at `ps`, the swath being
// `width` to either side.
fn snareSwath(ps: vec2f, width: f32) -> f32 {
  var swath = 0.0;
  for (var k = 0; k < 4; k++) {
    let en = snareEnv(k);
    if (en < 0.002) { continue; }
    let th = (floor(hash21(vec2f(uSnareN - f32(k), 2.7)) * 12.0) + 0.5) * PI / 12.0;
    let off = dot(ps, vec2f(-sin(th), cos(th))) / width;
    swath += en * exp(-off * off);
  }
  return swath;
}

// HATS: each tick picks a new scatter of things to glint. `id` names a
// thing (a ball, a cell of a grid); about `share` of them are picked.
fn hatPick(id: vec2f, share: f32) -> f32 {
  var glint = 0.0;
  for (var k = 0; k < 4; k++) {
    let en = hatEnv(k);
    if (en < 0.003) { continue; }
    if (hash21(id + 7.3 * (uHatN - f32(k))) > 1.0 - share) {
      glint += en;
    }
  }
  return glint;
}
