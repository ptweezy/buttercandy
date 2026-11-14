// Gumdrop: a sugared jelly sweet on a black mirror, inside a bead curtain of
// light that is the music. The candy is a lens and a lamp: everything seen on
// it, in it or through it is the sound, reflected and refracted.
//
// The five voices (see preset_header.wgsl). Each has one colour, one kind of
// motion and one place, and leaves no trace when it is not playing.
//
//   voice  what it does                         where                colour
//   -----  -----------------------------------  -------------------  ---------------
//   KICK   Pushes the jelly down and out        The whole sweet,     voiceCol(KICK):
//          (kickEnv), and it wobbles and sways  the biggest thing    the light
//          on the way jelly does. The push      there is, from its   inside the
//          lights it from inside, from the      base; and the floor  jelly and the
//          base up, and squeezes its bubbles.   outward from under   ring on the
//          Each kick sends one ripple out       it.                  floor.
//          across the floor with a ring of
//          light riding it, and throws one
//          droplet, which arcs round and falls
//          back in. A kick out of silence (the
//          drop) lands hardest.
//   SNARE  One flat sheet of light per hit, at  Straight across the  voiceCol(SNARE)
//          its own angle, edge on to the        sweet and the floor
//          camera. It crosses the stage and is  in front of and
//          gone with snareEnv: a bright line    behind it.
//          over the skin, the slice it cuts
//          inside the jelly, a straight line
//          on the floor.
//   HATS   Each tick (hatEnv) sets its own      The upper skin of    voiceCol(HATS)
//          sixteenth of the sugar grains        the sweet: the
//          glinting, chosen by hashing the hat  finest detail in
//          count, whether or not the grain has  the picture.
//          grown into a crystal yet.
//   BASS   The pool of light on the floor,      The floor under and  voiceCol(BASS)
//          webbed with caustics, swells and     round the sweet,
//          brightens with uBassLine. It shows   the lowest thing.
//          through the base of the jelly too.
//   TUNE   The filament in the jelly lights,    The middle of the    voiceCol(TUNE)
//          and its light gathers into a lamp    sweet, up and down
//          that rides up and down it with       its axis; and the
//          uPitch. Twelve lamps overhead, one   crown on its dome.
//          per pitch class and seen only as a
//          crown of highlights on the wet
//          dome, light the notes that are
//          sounding (chroma), and their ring
//          stands higher as the tune rises. A
//          note starting (noteEnv) makes all
//          of it flare and bows the filament.
//
// The scenery, in pal() colours, which never flashes:
//
//   spectrum       is the curtain round the stage: frequency runs bass to
//                  treble and back, three times round the ring, and every
//                  moment climbs its strand as it ages. The floor mirrors it,
//                  the candy reflects it and its droplets turn it upside
//                  down.
//   centroid       the brighter the sound, the taller the curtain stands: a
//                  riser pulls it up to the ceiling.
//   energy         is the state of the sweet. In a breakdown the sugar
//                  dissolves and it slumps into clear glass, bubbles and
//                  filament bare; as the energy returns it stands up and the
//                  frost line climbs back. The softbox is a little brighter
//                  in a full section.
//   the flavour    (the colour of the jelly) drifts round the palette by the
//                  clock. It no longer follows the harmony: the chord shows
//                  in the lamps.
//   idle           it breathes and sways, the frost comes and goes like a
//                  tide, crystals twinkle by the clock, one slow droplet
//                  keeps leaving and returning, and the pool and the lamp
//                  drift with uBassLine, uTune and uPitch in the colours of
//                  the scenery.
//   uReact         the envelopes carry it for everything a hit sets off. It
//                  also slows what never stops: the camera's orbit (down to
//                  a fifth, since every strand of the curtain crosses the
//                  picture as it turns), the folds of the curtain, the
//                  caustics and the turning of the filament; and it shortens
//                  the reach of the snare's sheet, the kick's ripple and
//                  droplet, and the swell of the pool.
//
// The shape is driven only by float uniforms and analytic functions. The
// 8-bit textures colour and light it but never move the surface. There is no
// feedback, so nothing here depends on the frame rate.

const CAM_F: f32 = 2.0;
const RING_R: f32 = 10.0;
// map() is not an exact distance (squash, sway), so it is scaled.
const SDF_K: f32 = 0.72;
const IOR: f32 = 1.33;
const FLIGHT: f32 = 1.05;
const SUGAR_N: f32 = 25.0;
// The kick: how far its push squashes the jelly, and how much it wobbles
// and sways afterwards.
const SQUASH: f32 = 0.10;
const WOBBLE: f32 = 0.045;
const SWAY: f32 = 0.07;
// The snare: how fast its sheet of light crosses, in stage units a second.
const CUT_SPEED: f32 = 6.0;
const STAGE_C: vec3f = vec3f(0.0, 0.55, 0.0);

var<private> gDrop: array<vec4f, 4>; // droplet centres and radii
var<private> gCone: vec4f;           // r1, r2, h, height of the lower centre
var<private> gConeAB: vec2f;
var<private> gTop: f32;              // height of the tip at rest
var<private> gSq: vec2f;             // 1/s and the widening for a vertical scale s
var<private> gSway: vec2f;
var<private> gKey: vec3f;
var<private> gKeyU: vec3f;
var<private> gKeyV: vec3f;
var<private> gKeyI: f32;
var<private> gJelly: f32;            // palette position of the flavour (scenery)
var<private> gCrust: f32;            // 0 glass .. 1 sugared
var<private> gPace: f32;             // uReact on the speed of what never stops
var<private> gCam: vec2f;            // the camera's angle round the stage, and its clock
// KICK
var<private> gKickCol: vec3f;
var<private> gPush: f32;             // the push of the last four kicks, summed
var<private> gKickAmp: vec4f;        // how hard each landed, for what outlasts its push
// SNARE: one sheet of light per hit, a plane sweeping through the stage
var<private> gSnareCol: vec3f;
var<private> gCutN: array<vec3f, 4>; // its normal
var<private> gCutC: vec4f;           // how far along the normal it has got
var<private> gCutE: vec4f;           // its crack
var<private> gCut: f32;              // all four
// HATS
var<private> gHatCol: vec3f;
var<private> gTick: vec4f;           // each hat's tick
var<private> gTicks: f32;            // all four
var<private> gSpark: f32;            // the grains they lit at this point of the skin
// BASS
var<private> gBassCol: vec3f;
var<private> gBass: f32;             // the swell
var<private> gPoolR: f32;            // how far the pool of light reaches
// TUNE
var<private> gTuneVoice: vec3f;      // the tune's own colour
var<private> gTuneCol: vec3f;        // the same, or the scenery's while idle
var<private> gTune: f32;             // how much tune there is
var<private> gTuneY: f32;            // the height its light sits at in the jelly
var<private> gPluck: f32;            // notes starting

fn h31(p: vec3f) -> f32 {
  var q = fract(p * 0.1031);
  q += dot(q, q.zyx + 31.32);
  return fract((q.x + q.y) * q.z);
}

fn h33(p: vec3f) -> vec3f {
  var q = fract(p * vec3f(0.1031, 0.1030, 0.0973));
  q += dot(q, q.yxz + 33.33);
  return fract((q.xxy + q.yxx) * q.zyx);
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
  let h = max(k - abs(a - b), 0.0) / k;
  return min(a, b) - h * h * k * 0.25;
}

// Everything that depends only on the uniforms.
fn setup() {
  gPace = mix(0.4, 1.0, uReact);
  // The orbit slows with uReact: every strand of the curtain crosses the
  // picture as the camera turns, so its pace is most of the restlessness.
  // (Half the reactivity is four tenths of the speed.)
  let ct = uTime * mix(0.2, 1.0, uReact * uReact);
  gCam = vec2f(ct * 0.14 + 0.5 * sin(ct * 0.05), ct);
  gKickCol = voiceCol(KICK);
  gSnareCol = voiceCol(SNARE);
  gHatCol = voiceCol(HATS);

  // Scenery: the state of the sweet follows the energy of the section.
  let wake = sstep(0.42, 0.74, uEnergy);
  let tide = 0.5 + 0.5 * sin(uTime * 0.23);
  gCrust = mix(wake, 0.2 + 0.7 * tide, uIdle);
  let melt = (1.0 - wake) * (1.0 - 0.5 * uIdle);
  let r1 = mix(0.80, 0.93, melt);
  let r2 = mix(0.50, 0.66, melt);
  let hh = mix(0.64, 0.36, melt);
  let c1 = mix(0.32, 0.16, melt);
  gCone = vec4f(r1, r2, hh, c1);
  let cb = (r1 - r2) / hh;
  gConeAB = vec2f(cb, sqrt(1.0 - cb * cb));
  gTop = c1 + hh + r2;
  // The flavour is scenery: it drifts round the palette by the clock.
  gJelly = 0.58 + uTime * 0.004;

  // KICK. kickEnv() is the push; the wobble, the droplet and the ripple
  // outlast it, so they take the kick's size (how hard, uReact, and the
  // same slow attack) and run on its age.
  let rise = 0.06 * (1.0 + 2.5 * (1.0 - uReact));
  let hard = mix(0.35, 1.0, uReact);
  var push = 0.0;
  var wob = 0.0;
  var sway = vec2f(0.0);
  for (var k = 0; k < 4; k++) {
    var a = uKickAges[k];
    var seed = uKickN - f32(k);
    // A kick out of silence lands hardest: the longer the gap before it,
    // the bigger its splash. (The oldest of the four has no predecessor to
    // measure from, and has all but died away by then.)
    var gap = 0.0;
    if (k < 3) {
      gap = sstep(1.2, 3.5, uKickAges[k + 1] - a);
    }
    let amp = uKickHits[k] * hard * sstep(0.0, rise, a);
    gKickAmp[k] = amp * (1.0 + gap);
    push += kickEnv(k) * (1.0 + 0.8 * gap);
    // Jelly rings on after it is pushed.
    wob += amp * (1.0 + 0.8 * gap) * exp(-a * 4.5) * sin(a * 22.0);
    let th = TAU * hash21(vec2f(seed, 3.7));
    sway += vec2f(cos(th), sin(th)) * amp * exp(-a * 3.6) * sin(a * 20.0);

    // The droplet this kick threw.
    // A calmer or softer kick throws a smaller one, and not so far.
    var u = a / FLIGHT;
    var size = mix(0.7, 1.0, uReact) * mix(0.6, 1.0, uKickHits[k]);
    var fling = mix(0.75, 1.0, uReact);
    if (k == 3 && uIdle > 0.5) {
      let it = uTime * 0.085;
      u = fract(it);
      seed = floor(it) + 100.0;
      size = sstep(0.5, 1.0, uIdle);
      fling = 1.0;
    }
    let h1 = hash21(vec2f(seed, 1.3));
    let h2 = hash21(vec2f(seed, 7.1));
    let h3 = hash21(vec2f(seed, 13.7));
    let uu = clamp(u, 0.0, 1.0);
    let arc = 4.0 * uu * (1.0 - uu);
    let ang = TAU * h1 + (1.3 + 1.3 * h2) * uu * sign(h3 - 0.5);
    let rad = 0.35 + (0.85 + 0.25 * h2) * arc * fling;
    let y = 0.60 + (0.10 + 0.60 * h3) * arc * fling;
    let rho = (0.12 + 0.07 * h1) * (1.0 + 0.4 * gap) * size * sstep(0.0, 0.12, uu) * sstep(1.0, 0.86, uu) * step(u, 1.0);
    gDrop[k] = vec4f(cos(ang) * rad, y, sin(ang) * rad, rho);
  }
  gPush = min(push, 1.4);
  // Pushed down, the jelly is pushed out: a little wider than its volume asks.
  let s = 1.0 - (SQUASH * gPush + WOBBLE * wob) + 0.022 * sin(uTime * 0.8) * (0.3 + 0.7 * uIdle);
  gSq = vec2f(1.0 / s, sqrt(s) / (1.0 + 0.04 * gPush));
  gSway = sway * SWAY + uIdle * 0.03 * vec2f(sin(uTime * 0.6), cos(uTime * 0.47));

  // SNARE. Each hit is a sheet of light at its own angle, which crosses the
  // candy in the time its crack lasts. It stands more or less edge on to the
  // camera, so that it shows as a line across the candy and the floor and
  // never as a flash of the whole jelly.
  let across = vec2f(cos(gCam.x), sin(gCam.x));
  let along = vec2f(-across.y, across.x);
  for (var k = 0; k < 4; k++) {
    let seed = uSnareN - f32(k);
    let turn = (hash21(vec2f(seed, 2.9)) - 0.5) * 1.9;
    let tilt = (hash21(vec2f(seed, 8.3)) - 0.5) * 1.1;
    let side = sign(hash21(vec2f(seed, 5.1)) - 0.5);
    let nh = across * cos(turn) + along * sin(turn);
    gCutN[k] = normalize(vec3f(nh.x, tilt, nh.y)) * side;
    gCutC[k] = (uSnareAges[k] - 0.06) * CUT_SPEED * mix(0.6, 1.0, uReact);
    gCutE[k] = snareEnv(k);
  }
  gCut = gCutE.x + gCutE.y + gCutE.z + gCutE.w;

  // HATS.
  gTick = vec4f(hatEnv(0), hatEnv(1), hatEnv(2), hatEnv(3));
  gTicks = gTick.x + gTick.y + gTick.z + gTick.w;

  // BASS and TUNE have no hits to wait for; while idle they drift, and what
  // they light goes back to the colours of the scenery.
  // (The swell is a little steeper than the bass line, so that the pool
  // visibly breathes in a full mix, where the line sits near the top.)
  gBass = uBassLine * sqrt(uBassLine);
  gBassCol = mix(voiceCol(BASS), pal(gJelly), uIdle);
  gPoolR = 0.95 + 0.9 * gBass * mix(0.5, 1.0, uReact);
  gTune = uTune * sstep(0.02, 0.3, uTune);
  gTuneVoice = voiceCol(TUNE);
  gTuneCol = mix(gTuneVoice, mix(pal(gJelly + 0.5), vec3f(1.0), 0.5), uIdle);
  gTuneY = gTop * mix(0.27, 0.76, uPitch);
  gPluck = noteSum();

  gKey = normalize(vec3f(-0.62, 0.62, -0.48));
  gKeyU = normalize(cross(vec3f(0.0, 1.0, 0.0), gKey));
  gKeyV = cross(gKey, gKeyU);
  // Scenery: the softbox is a little brighter in a full section.
  gKeyI = 0.9 + 0.2 * uEnergy;
}

// World space to the jelly's rest space: undo the squash and the sway.
fn toBody(p: vec3f) -> vec3f {
  let sw = gSway * (p.y * p.y * 0.5);
  return vec3f((p.x - sw.x) * gSq.y, p.y * gSq.x, (p.z - sw.y) * gSq.y);
}

fn sdBody(p: vec3f) -> f32 {
  let b3 = toBody(p);
  let q = vec2f(length(b3.xz), b3.y - gCone.w);
  let k = dot(q, vec2f(-gConeAB.x, gConeAB.y));
  var d: f32;
  if (k < 0.0) {
    d = length(q) - gCone.x;
  } else if (k > gConeAB.y * gCone.z) {
    d = length(q - vec2f(0.0, gCone.z)) - gCone.y;
  } else {
    d = dot(q, vec2f(gConeAB.y, gConeAB.x)) - gCone.x;
  }
  return d;
}

fn map(p: vec3f) -> f32 {
  var d = sdBody(p);
  for (var k = 0; k < 4; k++) {
    let c = gDrop[k];
    if (c.w > 0.0005) {
      d = smin(d, length(p - c.xyz) - c.w, 0.22);
    }
  }
  return d * SDF_K;
}

fn calcNormal(p: vec3f) -> vec3f {
  let e = vec2f(1.0, -1.0) * 0.0015;
  return normalize(
    e.xyy * map(p + e.xyy) + e.yyx * map(p + e.yyx) + e.yxy * map(p + e.yxy) + e.xxx * map(p + e.xxx),
  );
}

// Where a ray could meet the candy: one sphere round the body and one round
// each droplet in flight. Returns the span (t0, t1), empty if t1 <= t0.
fn bound(o: vec3f, d: vec3f) -> vec2f {
  var t0 = 1e5;
  var t1 = -1e5;
  var oc = o - vec3f(0.0, 0.55, 0.0);
  var b = dot(oc, d);
  var h = b * b - dot(oc, oc) + 1.25 * 1.25;
  if (h > 0.0) {
    h = sqrt(h);
    t0 = -b - h;
    t1 = -b + h;
  }
  for (var k = 0; k < 4; k++) {
    let c = gDrop[k];
    if (c.w > 0.0005) {
      let r = c.w + 0.07;
      oc = o - c.xyz;
      b = dot(oc, d);
      h = b * b - dot(oc, oc) + r * r;
      if (h > 0.0) {
        h = sqrt(h);
        t0 = min(t0, -b - h);
        t1 = max(t1, -b + h);
      }
    }
  }
  return vec2f(t0, t1);
}

// Sphere-trace the candy. Returns (t, coverage): coverage is 1 on a hit and
// falls to 0 over a pixel for rays that only graze it, which is what
// anti-aliases the silhouette. `tofs` is the path length before `o`.
fn march(o: vec3f, d: vec3f, t0: f32, t1: f32, tofs: f32, pxw: f32, steps: i32) -> vec2f {
  var t = t0;
  var best = 1e5;
  var tb = t0;
  var hit = false;
  for (var i = 0; i < steps; i++) {
    let dist = map(o + d * t) / SDF_K;
    let w = pxw * (t + tofs);
    let ratio = dist / w;
    if (ratio < best) {
      best = ratio;
      tb = t;
    }
    if (ratio < 0.25) {
      hit = true;
      break;
    }
    // Never less than a fraction of a pixel, so grazing rays get past the
    // edge instead of stalling beside it.
    t += max(dist * SDF_K, 0.4 * w);
    if (t > t1) {
      break;
    }
  }
  if (hit) {
    return vec2f(t, 1.0);
  }
  return vec2f(tb, clamp(1.25 - best, 0.0, 1.0));
}

// The key light: a softbox, as a mask over directions.
fn softbox(d: vec3f) -> f32 {
  let kd = dot(d, gKey);
  if (kd < 0.5) {
    return 0.0;
  }
  let q = vec2f(dot(d, gKeyU), dot(d, gKeyV)) / kd;
  let bx = abs(q) - vec2f(0.21, 0.12);
  let sd = length(max(bx, vec2f(0.0))) + min(max(bx.x, bx.y), 0.0) - 0.05;
  return sstep(0.035, -0.035, sd);
}

// What surrounds the stage: the spectrogram as a wall of light, the chroma
// lamps overhead, and (when `box` is set) the softbox.
fn sky(o: vec3f, d: vec3f, box: f32, fine: f32) -> vec3f {
  let a = dot(d.xz, d.xz) + 1e-6;
  let b = dot(o.xz, d.xz);
  let c = dot(o.xz, o.xz) - RING_R * RING_R;
  let t = (-b + sqrt(max(b * b - a * c, 0.0))) / a;
  let hp = o + d * t;
  let h = max(hp.y, 0.0);
  let az = atan2(hp.z, hp.x) / TAU;
  // It hangs in slow folds.
  let azw = az + 0.0012 * sin(h * 2.3 - uTime * 0.6 * gPace + az * 37.7) + 0.0007 * sin(h * 5.1 + uTime * 0.9 * gPace - az * 81.0);
  let fx = 0.5 - 0.5 * cos(azw * 3.0 * TAU);
  let x = mix(0.04, 0.88, fx);
  let row = uHistHead - min(h * 18.0, 200.0);
  let past = (histRow(x - 0.012, row) + 2.0 * histRow(x, row) + histRow(x + 0.012, row)) * 0.25;
  // The brighter the sound, the taller the light stands.
  let lift = exp(-h * mix(0.85, 0.30, sstep(0.22, 0.62, uCentroid)));
  var inten = past * past * (0.22 + 0.9 * exp(-h * 1.5) + 1.6 * exp(-h * 7.0)) * lift;
  if (fine > 0.0) {
    // Strands, so it hangs like a bead curtain rather than a flat screen:
    // each one carries the history of its own frequency upward.
    let u = azw * 400.0;
    let iu = floor(u);
    let fu = u - iu;
    let c = 0.5 - 0.5 * cos(fu * TAU);
    let strand = c * c * (0.5 + 1.2 * hash21(vec2f(iu, 5.0)));
    inten *= mix(1.0, 0.22 + 1.9 * strand, fine);
  }
  var col = pal(0.55 + fx * 0.5 + h * 0.03) * inten * 0.9;
  col += pal(0.72 + fx * 0.2) * 0.012 * lift;

  // TUNE: twelve lamps overhead, one per pitch class. The notes of the tune
  // light theirs, and the ring of them stands higher as the tune rises.
  if (d.y > 0.42) {
    let la = atan2(d.z, d.x) / TAU * 12.0;
    let i = floor(la);
    let ang = (i + 0.5) / 12.0 * TAU;
    let el = mix(0.64, 0.86, uPitch);
    let ce = sqrt(1.0 - el * el);
    let ld = vec3f(cos(ang) * ce, el, sin(ang) * ce);
    let m = sstep(0.988, 0.996, dot(d, ld));
    let ch = chroma(i32(i) + 12);
    let hue = pal((i + 12.5) / 12.0);
    let lit = 14.0 * ch * ch * (0.5 + uTonal) * gTune * (1.0 + 0.8 * gPluck);
    col += (mix(vec3f(1.0), hue, 0.5) * 0.2 + mix(gTuneVoice, hue, uIdle) * lit) * m * select(1.0, 0.12, box > 0.0);
  }
  if (box > 0.0) {
    col += mix(vec3f(1.0), pal(gJelly + 0.5), 0.2) * softbox(d) * box * gKeyI;
  }
  return col;
}

fn caustic(p: vec2f, t: f32) -> f32 {
  var i = p;
  var c = 1.0;
  for (var n = 0; n < 3; n++) {
    let tt = t * (1.0 - 3.5 / f32(n + 1));
    i = p + vec2f(cos(tt - i.x) + sin(tt + i.y), sin(tt - i.y) + cos(tt + i.x));
    let s = sin(i.x + tt);
    let k = cos(i.y + tt);
    c += abs(s * k) / (1.25 * sqrt(s * s + k * k + 1e-6));
  }
  c = 1.17 - pow(c / 3.0, 1.4);
  let c2 = c * c;
  let c4 = c2 * c2;
  return c4 * c4;
}

// The light on the floor under and round the candy. A little of it is
// scenery: what the softbox sends through the jelly, in the flavour's colour,
// where `focus` says the lens gathers it. The rest is the BASS: a pool in its
// colour, webbed with caustics, that swells and spreads with the bass line.
fn pool(fp: vec3f, focus: f32) -> vec3f {
  let r2 = dot(fp.xz, fp.xz);
  // (It has an edge, so that its swelling shows as a change of size.)
  let rr = r2 / (gPoolR * gPoolR);
  let spread = exp(-rr * rr);
  if (focus <= 0.0 && spread * gBass < 0.002) {
    return vec3f(0.0);
  }
  let jc = pal(gJelly);
  let web = caustic(fp.xz * 4.0, (uPhase * 0.5 + uTime * 0.2) * gPace);
  var c = jc * jc * focus * (0.06 + 1.5 * web) * 0.22 * gKeyI;
  c += gBassCol * gBass * ((0.10 + 1.8 * web) * max(focus, 0.8 * spread) + 0.26 * spread);
  return c;
}

fn floorDiffuse(fp: vec3f, sh: f32, cau: vec3f) -> vec3f {
  let r2 = dot(fp.xz, fp.xz);
  let spot = exp(-r2 * 0.10);
  var c = mix(vec3f(0.6), pal(0.6), 0.5) * 0.022 * spot * (0.2 + 0.8 * sh) * gKeyI;
  c += pal(gJelly) * 0.03 * exp(-r2 * 1.1);
  return c + cau;
}

// The floor as seen in or through the candy: no ripples, no shadow.
fn floorLite(fp: vec3f, d: vec3f) -> vec3f {
  let f = 0.04 + 0.96 * pow(1.0 - clamp(-d.y, 0.0, 1.0), 5.0);
  let r = vec3f(d.x, abs(d.y) + 1e-3, d.z);
  // Where the pool of light lies, without marching for it: under the candy
  // and out along its shadow.
  let sd = -gKey.xz / gKey.y;
  let v = fp.xz - sd * 0.5;
  let al = dot(v, normalize(sd));
  let focus = max(exp(-(al * al) / 0.42 - (dot(v, v) - al * al) / 0.30), 0.55 * exp(-dot(fp.xz, fp.xz) / 0.45));
  return floorDiffuse(fp, 1.0, pool(fp, focus)) + sky(fp, r, 0.0, 0.0) * f;
}

fn envAll(o: vec3f, d: vec3f, box: f32) -> vec3f {
  if (d.y < -1e-4 && o.y > 0.0) {
    let fp = o + d * (-o.y / d.y);
    if (dot(fp.xz, fp.xz) < RING_R * RING_R) {
      return floorLite(fp, d);
    }
  }
  return sky(o, d, box, 0.0);
}

// The filament set in the jelly, a lazy helix: where it crosses height y.
// A note of the TUNE plucks it wider.
fn filament(y: f32) -> vec2f {
  let v = clamp(y / gTop, 0.0, 1.0);
  let amp = (0.045 + 0.05 * gPluck) * sin(PI * v);
  let ang = y * 4.5 + uTime * 0.7 * gPace;
  return vec2f(cos(ang), sin(ang)) * amp + gSway * (y * y * 0.5);
}

// SNARE: how brightly the sheets of light strike a point of the stage.
fn cut(p: vec3f) -> f32 {
  var b = 0.0;
  for (var k = 0; k < 4; k++) {
    let x = dot(p - STAGE_C, gCutN[k]) - gCutC[k];
    b += gCutE[k] * (exp(-x * x * 900.0) + 0.15 * exp(-x * x * 60.0));
  }
  return b;
}

// The same sheets seen through the jelly, along the ray from p for len: the
// slice each one cuts through it.
fn cutInside(p: vec3f, d: vec3f, len: f32) -> f32 {
  var b = 0.0;
  for (var k = 0; k < 4; k++) {
    let dn = dot(d, gCutN[k]);
    let t = (gCutC[k] - dot(p - STAGE_C, gCutN[k])) / (dn + select(-1e-4, 1e-4, dn >= 0.0));
    b += gCutE[k] * sstep(0.0, 0.04, t) * sstep(len, len - 0.04, t) * exp(-clamp(t, 0.0, 4.0) * 1.2);
  }
  return b;
}

// One layer of sugar: cubes scattered through the jelly's rest space and cut
// by its surface, each lit face by face. Returns lit colour and coverage, and
// adds to gSpark the grains the hats have lit here.
fn sugar(q0: vec3f, scale: f32, cover: f32, pxw: f32, n: vec3f, rd: vec3f) -> vec4f {
  let aa = pxw * scale;
  // A lattice turned off the body's axes, so no rows show.
  let g = vec3f(
    dot(q0, vec3f(0.756, 0.436, -0.488)),
    dot(q0, vec3f(-0.218, 0.873, 0.436)),
    dot(q0, vec3f(0.617, -0.218, 0.756)),
  ) * scale;
  let id = floor(g);
  let hs = h33(id + scale);
  let ax = normalize(h33(id + scale + 17.0) - 0.5 + 1e-4);
  var v = g - id - 0.5 - (hs - 0.5) * 0.44;
  v -= 2.0 * dot(v, ax) * ax;
  let av = abs(v);
  let cheb = max(av.x, max(av.y, av.z));
  let grown = sstep(hs.z, hs.z + 0.15, cover);
  let whole = 0.16 + 0.12 * hs.x;
  let rad = whole * grown;
  // HATS: each one picks its own sixteenth of the grains to glint, whether
  // or not they have grown into crystals.
  if (gTicks > 0.002) {
    var tw = 0.0;
    for (var k = 0; k < 4; k++) {
      let nk = fmod(uHatN - f32(k), 1024.0);
      tw += step(0.94, h31(id + vec3f(nk * 0.37, nk * 1.71, nk * 0.93))) * gTick[k];
    }
    gSpark += tw * sstep(whole + aa, whole - aa, cheb) * sstep(1.5, 0.5, aa);
  }
  let m = sstep(rad + aa, rad - aa, cheb) * sstep(0.0, 0.06, rad) * sstep(0.6, 0.3, aa);
  if (m <= 0.0) {
    return vec4f(0.0);
  }
  // The face of the cube this point is nearest.
  var e = vec3f(0.0, 0.0, sign(v.z));
  if (av.x > av.y && av.x > av.z) {
    e = vec3f(sign(v.x), 0.0, 0.0);
  } else if (av.y > av.z) {
    e = vec3f(0.0, sign(v.y), 0.0);
  }
  e -= 2.0 * dot(e, ax) * ax;
  let nf = normalize(n + e * 0.9);
  let dif = max(dot(nf, gKey), 0.0);
  let gl = pow(max(dot(reflect(rd, nf), gKey), 0.0), 14.0);
  // Idle: crystals twinkle by the clock.
  let it = uTime * 2.2;
  let ip = step(0.93, h31(id + vec3f(floor(it) * 0.61)));
  let fi = sin(PI * fract(it));
  let tw = uIdle * ip * fi * fi;
  let c = vec3f(0.10 + 0.75 * dif * gKeyI) + (gl * 3.5 + tw * 7.0) * mix(vec3f(1.0), pal(hs.y), 0.3);
  return vec4f(c, m);
}

// Bubbles rising round the filament, seen along the ray inside the jelly
// (qa to qb, in rest space). Returns (shade, light): what they block, and
// what they catch. A kick's push squeezes them with the jelly.
fn bubbles(qa: vec3f, qb: vec3f, pxw: f32) -> vec2f {
  let seg = qb - qa;
  let len = length(seg) + 1e-4;
  let dir = seg / len;
  let a2 = dot(dir.xz, dir.xz) + 1e-4;
  let sc = clamp(-dot(qa.xz, dir.xz) / a2, 0.0, len);
  let rise = uTime * 0.04 + uPhase * 0.015;
  let key = normalize(gKey - dir);
  var shade = 1.0;
  var light = 0.0;
  for (var i = 0; i < 5; i++) {
    let sp = sc + (f32(i) - 2.0) * 0.17;
    if (sp > 0.0 && sp < len) {
      var q = qa + dir * sp;
      q.y -= rise;
      let id = floor(q / 0.24);
      let hs = h33(id + 91.0);
      var c = (id + 0.5 + (hs - 0.5) * 0.5) * 0.24;
      c.y += rise;
      let room = exp(-dot(c.xz, c.xz) / 0.16) * sstep(0.08, 0.3, c.y) * sstep(gTop - 0.12, gTop - 0.4, c.y);
      let rad = (0.02 + 0.045 * hs.x * hs.x) * room * step(hs.z, 0.8) * (1.0 - 0.3 * min(gPush, 1.0));
      let oc = c - qa;
      let tc = dot(oc, dir);
      let off = oc - dir * tc;
      let x = length(off) / max(rad, 1e-4);
      if (x < 1.0 && rad > 0.004 && tc > 0.0 && tc < len) {
        let m = sstep(1.0, 1.0 - 1.5 * pxw / rad, x);
        // The front of the little sphere, for its highlight.
        let nb = normalize(-off - dir * rad * sqrt(1.0 - x * x));
        let hl = pow(max(dot(nb, key), 0.0), 30.0) * 2.5 + 0.45 * exp(-(x - 0.82) * (x - 0.82) * 90.0);
        shade = min(shade, mix(1.0, mix(0.8, 0.3, sstep(0.5, 1.0, x)), m));
        light = max(light, hl * m);
      }
    }
  }
  return vec2f(shade, light);
}

fn shadeBody(pos: vec3f, n: vec3f, rd: vec3f, pxw: f32) -> vec3f {
  let ndv = clamp(dot(n, -rd), 0.0, 1.0);
  let fres = min(0.045 + 0.955 * pow(1.0 - ndv, 5.0), 0.65);
  let rr = reflect(rd, n);
  let reflC = envAll(pos, rr, 0.0);
  let jc = pal(gJelly + 0.10 * (pos.y - 0.7));

  let tr = refract(rd, n, 1.0 / IOR);
  var s = 0.03;
  var dd = 0.0;
  for (var i = 0; i < 16; i++) {
    dd = -map(pos + tr * s);
    if (dd < 0.004) {
      break;
    }
    s += max(dd * 1.2, 0.012);
  }
  var bgc: vec3f;
  let p2 = pos + tr * s;
  if (p2.y < 0.0 && tr.y < 0.0) {
    // Out through the base: the floor under the sweet.
    s = max(pos.y, 0.0) / -tr.y;
    bgc = floorLite(pos + tr * s, tr);
  } else {
    let e = 0.002;
    let m0 = -dd;
    let n2 = -normalize(vec3f(
      map(p2 + vec3f(e, 0.0, 0.0)) - m0,
      map(p2 + vec3f(0.0, e, 0.0)) - m0,
      map(p2 + vec3f(0.0, 0.0, e)) - m0,
    ));
    var t2 = refract(tr, n2, IOR);
    if (dot(t2, t2) < 0.5) {
      t2 = reflect(tr, n2);
    }
    bgc = envAll(p2, t2, 5.0);
    // Light the lens gathers onto its own far wall.
    bgc += jc * jc * gKeyI * 1.6 * pow(max(dot(n2, gKey), 0.0), 9.0);
  }
  let len = s;
  var thru = bgc * pow(jc, vec3f(0.35 + len * 1.5));

  // The filament: nearest approach of the inner ray to the curve.
  let a2 = dot(tr.xz, tr.xz) + 1e-4;
  var sc = clamp(-dot(pos.xz, tr.xz) / a2, 0.0, len);
  for (var i = 0; i < 2; i++) {
    let fy = pos.y + tr.y * sc;
    sc = clamp(-dot(pos.xz - filament(fy), tr.xz) / a2, 0.0, len);
  }
  let fy = pos.y + tr.y * sc;
  let dv = pos.xz + tr.xz * sc - filament(fy);
  let d2 = dot(dv, dv);
  let along = sstep(0.06, 0.25, fy) * sstep(gTop - 0.10, gTop - 0.32, fy);
  let core = exp(-d2 / 0.0004);
  // Scenery: the thread itself, barely lit. Clear jelly shows it alone;
  // sugared jelly is cloudier and glows around it.
  let fcol = mix(pal(gJelly + 0.5), vec3f(1.0), 0.5) * core * 0.3 + jc * jc * mix(0.12, 0.34, gCrust) * 0.012 / (d2 + 0.035);
  thru += fcol * along * pow(jc, vec3f(0.2 + sc * 1.2));
  // KICK: its push lights the jelly from inside, from the base up.
  let q = toBody(pos);
  let dens = 1.0 - exp(-len * 1.3);
  let low = exp(-max(fy, 0.0) * 1.3);
  let glow = 0.9 * low * 0.10 / (d2 + 0.10) + 0.25 * dens * mix(1.0, 0.3, clamp(q.y / gTop, 0.0, 1.0));
  thru += gKickCol * gPush * glow * pow(jc, vec3f(0.1 + sc * 0.3)) * (1.0 + gCrust);
  // TUNE: the thread lights in the tune's colour, and its light gathers into
  // a lamp that rides up and down it with the pitch. A note makes it flare.
  let yb = fy - gTuneY;
  let bead = exp(-yb * yb / 0.018) * (1.0 + 1.2 * gPluck);
  // The lamp is a ball of light, tight enough that the lens of the jelly
  // does not smear it into a band. Where it is, its light stands in for the
  // jelly's own colour instead of adding to it, so that it keeps the tune's.
  let ball = clamp(gTune * bead * exp(-d2 / 0.018) * along, 0.0, 0.85);
  thru = mix(thru, gTuneCol * 1.8, ball);
  let lamp = core * (0.7 + 2.0 * bead) + bead * 0.004 / (d2 + 0.02);
  thru += gTuneCol * gTune * lamp * along * pow(jc, vec3f(0.1 + sc * 0.4)) * (1.0 + 0.6 * gCrust);

  let bb = bubbles(toBody(pos), toBody(pos + tr * len), pxw);
  thru = thru * bb.x + mix(vec3f(1.0), jc, 0.5) * bb.y * (0.25 + 0.5 * gKeyI);

  // Light from the softbox, soaked through however much jelly lies between.
  let k1 = max(-map(pos + gKey * 0.18) / SDF_K, 0.0);
  let k2 = max(-map(pos + gKey * 0.50) / SDF_K, 0.0);
  let depth = k1 + 0.8 * k2;
  let turb = mix(0.12, 0.9, gCrust);
  let jd = jc * jc;
  var scat = jd * pow(jc, vec3f(depth * 2.5)) * gKeyI * 0.30 * dens * turb;
  scat += jd * gKeyI * 0.55 * pow(max(dot(n, -gKey), 0.0), 2.0) * dens;
  scat += jd * 0.04 * dens;

  // Sugar, up to a frost line that rises with the energy. Droplets are bare:
  // wet beads of jelly.
  let onBody = sstep(0.05, 0.005, sdBody(pos));
  let line = mix(-0.15, 0.62 * gTop, gCrust);
  let cover = sstep(line + 0.35, line - 0.35, q.y) * onBody;

  var col = mix(thru + scat * (1.0 - 0.7 * ball), reflC, fres);
  // The softbox, mirrored in the wet jelly and dulled where it is frosted.
  col += mix(vec3f(1.0), jc, 0.15) * softbox(rr) * gKeyI * (1.3 + 3.0 * pow(1.0 - ndv, 3.0)) * (1.0 - 0.75 * cover);

  gSpark = 0.0;
  if (cover > 0.0 || gTicks > 0.002) {
    let tint = mix(vec3f(1.0), jc, 0.5);
    // A powdering under the crystals.
    col = mix(col, col * 0.72 + tint * 0.08 * gKeyI, cover * 0.5);
    let s1 = sugar(q, 62.0, cover * 0.95, pxw, n, rd);
    col = mix(col, col * 0.5 + s1.rgb * tint * 0.6, s1.a);
    let s2 = sugar(q, SUGAR_N, cover * 0.45, pxw, n, rd);
    col = mix(col, col * 0.45 + s2.rgb * tint * 0.7, s2.a);
    // HATS: the grains they picked glint, most of all on the upper skin.
    col += gHatCol * gSpark * 6.0 * sstep(0.22, 0.6, q.y / gTop) * onBody;
  }
  // SNARE: the sheet of light strikes the skin and shows as a slice inside.
  if (gCut > 0.002) {
    col += gSnareCol * (cut(pos) * 3.2 + cutInside(pos, tr, len) * 0.4);
  }
  return col;
}

fn scene(uv: vec2f) -> vec3f {
  setup();
  let p = aspect(uv);

  let ca = gCam.x;
  let ct = gCam.y;
  let ro = vec3f(sin(ca) * 3.7, 1.0 + 0.3 * sin(ct * 0.13), -cos(ca) * 3.7);
  let fwd = normalize(vec3f(0.0, 0.62, 0.0) - ro);
  let right = normalize(cross(vec3f(0.0, 1.0, 0.0), fwd));
  let up = cross(fwd, right);
  let rd = normalize(fwd * CAM_F + right * p.x + up * p.y);
  let pxw = 2.0 / (uRes.y * CAM_F);

  // Floor or wall behind the candy?
  var col = vec3f(0.0);
  var tBg = 1e5;
  var onFloor = false;
  var fp = vec3f(0.0);
  if (rd.y < -1e-4) {
    let tf = -ro.y / rd.y;
    fp = ro + rd * tf;
    if (dot(fp.xz, fp.xz) < RING_R * RING_R) {
      onFloor = true;
      tBg = tf;
    }
  }

  // The candy first: where it covers the pixel there is no need to shade
  // what is behind it.
  var hitc = vec2f(0.0);
  let bd = bound(ro, rd);
  if (min(bd.y, tBg) > max(bd.x, 0.0)) {
    hitc = march(ro, rd, max(bd.x, 0.0), min(bd.y, tBg), 0.0, pxw, 72);
  }

  if (hitc.y >= 1.0) {
    // Hidden.
  } else if (onFloor) {
    let r = length(fp.xz);
    // KICK: the ripple each one sends out across the floor from under the
    // candy, and the ring of light that rides it.
    var slope = 0.0;
    var ring = 0.0;
    for (var k = 0; k < 4; k++) {
      let a = uKickAges[k];
      let x = r - (0.75 + 3.0 * mix(0.7, 1.0, uReact) * a);
      let live = gKickAmp[k] * exp(-a * 1.5) * sstep(1.5, 0.9, a);
      let env = exp(-x * x * 5.0);
      slope += 0.02 * live * env * (-8.0 * sin(x * 8.0) - 10.0 * x * cos(x * 8.0));
      ring += live * env * env * env * env;
    }
    slope += uIdle * 0.01 * cos(r * 5.0 - uTime * 1.2) / (1.0 + r);
    let rdir = fp.xz / max(r, 1e-3);
    let n = normalize(vec3f(-slope * rdir.x, 1.0, -slope * rdir.y));
    var rr = reflect(rd, n);
    rr = normalize(vec3f(rr.x, abs(rr.y) + 1e-3, rr.z));
    let f = 0.08 + 0.92 * pow(1.0 - clamp(dot(-rd, n), 0.0, 1.0), 5.0);
    var refl = sky(fp, normalize(vec3f(rr.x, rr.y * 0.45, rr.z)), 0.0, 1.0);

    var sh = 1.0;
    var focus = 0.0;
    if (r < 3.0) {
      var pen = 0.0;
      for (var i = 0; i < 4; i++) {
        let s = 0.12 + 0.42 * f32(i);
        let dd = map(fp + gKey * s) / SDF_K;
        sh *= sstep(-0.05, 0.30 * s + 0.05, dd);
        pen += max(-dd, 0.0);
      }
      focus = sstep(0.0, 0.7, pen);
    }

    // The candy mirrored in the floor.
    let bm = bound(fp, rr);
    if (bm.y > max(bm.x, 0.02)) {
      let m = march(fp, rr, max(bm.x, 0.02), bm.y, tBg, pxw, 40);
      if (m.y > 0.0) {
        let hp = fp + rr * m.x;
        let hn = calcNormal(hp);
        refl = mix(refl, shadeBody(hp, hn, rr, pxw * (tBg + m.x)), m.y);
      }
    }
    col = floorDiffuse(fp, sh, pool(fp, focus)) + refl * f;
    col += gKickCol * ring * 0.3 / (1.0 + 0.3 * r);
    // SNARE: the sheet of light strikes the floor in a straight line.
    if (gCut > 0.002) {
      col += gSnareCol * cut(fp) * 1.0 * exp(-r * r * 0.10);
    }
  } else {
    col = sky(ro, rd, 0.0, 1.0);
  }

  if (hitc.y > 0.0) {
    var pos = ro + rd * hitc.x;
    let n = calcNormal(pos);
    pos -= n * (map(pos) / SDF_K);
    col = mix(col, shadeBody(pos, n, rd, pxw * hitc.x), hitc.y);
  }
  return col;
}
