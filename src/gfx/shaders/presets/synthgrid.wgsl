// Synthgrid: a wet neon highway through mountains made of sound, under a
// striped sun. The terrain is the spectrogram itself: bass rises beside the
// road, treble further out, and every row is a moment of music rolling in
// from the horizon. The land is black glass, quiet land lies under water, and
// the road and the water mirror everything above them.
//
// The land is the scenery: a record of the last few seconds, in pal()
// colours, that nothing flashes. What is playing NOW is told by the five
// voices, each in its own colour, place and kind of motion:
//
//   voice  what it does                         where                   colour
//   -----  -----------------------------------  ----------------------  ----------------
//   KICK   IS the sun. Each kick pushes it      the centre of the       voiceCol(KICK)
//          (kickEnv): the disc swells and       picture, the biggest
//          brightens, its glow and corona push  thing in it (and its
//          out, a heavy ring rolls off the      mirror image in the
//          rim, the view leans in a touch. Its  road)
//          light is the day: the haze on the
//          horizon, the glints on the glass.
//          No kick for a couple of seconds and
//          a moon covers it: total eclipse.
//          The first kick back throws the moon
//          off in under half a second, with a
//          far bigger ring.
//   SNARE  a crack: one straight streak right   across the sky, above   voiceCol(SNARE)
//          across the sky for each snare or     the sun
//          clap, there at once and gone in a
//          seventh of a second (snareEnv).
//   HATS   a tick: each hat sparks a different  the stars at the top    voiceCol(HATS)
//          scatter of stars for an instant      and round the edge of
//          (hatEnv).                            the sky
//   BASS   a swell (uBassLine): the road's      the bottom of the       voiceCol(BASS)
//          lines and the grid of the land       frame: the road and the
//          beside the road take the bass's      land beside it
//          colour, glow and fatten, its light
//          fills the glass and the wet ground,
//          the peaks beside the road stand
//          taller and the water ripples harder.
//   TUNE   the aurora: twelve curtains, C on    the middle of the sky,  voiceCol(TUNE)
//          the left to B on the right, lit by   behind the sun; its hem
//          the notes sounding (chroma); a note  rides up and down with
//          starting plucks them brighter        uPitch
//          (noteEnv); how much tune there is
//          (uTune) is how bright they are.
//
// Scenery, in pal() colours, slow, never flashing:
//   spectrum       the land. Each row across the road is one sixtieth of a
//                  second of sound; it takes 2.7 s to reach you (up to 4 s
//                  with the reactivity down), so a drop is a mountain range
//                  you watch arrive. Loud bass is tall peaks by the road,
//                  treble the lower fields beyond. Silence is lake. The grid
//                  glows with what sounded when that piece of land was made.
//                  Beside the road, where the bass lives, land that the
//                  palette would paint in the bass's colour goes pale, and
//                  the road's lines are white until the bass lights them.
//   harmony        turns every scenery colour, gliding with the chords.
//   a riser        (bright and full) stretches the stars into streaks away
//                  from the vanishing point.
//   eclipse        with no kick: the moon, slow pearl streamers round it,
//                  more stars.
//   silence        the eclipse, a slow swell for land, and the aurora
//                  drifting in the palette's own colours; its hem and the
//                  land breathe with uPitch and uBassLine.
//   reactivity     (uReact) turned down, the land rolls in more slowly and
//                  stands lower, the grid glows less with the sound, and
//                  what each voice sets off is smaller and swells in more
//                  slowly (the shared envelopes see to that).

// ---- the world -------------------------------------------------------------
const FOC: f32 = 1.5;        // focal length
const HORIZON: f32 = -0.12;  // screen height of the vanishing point
const CAM_H: f32 = 0.85;
const ROAD: f32 = 1.0;       // half-width of the road
const ROW_Z: f32 = 0.25;     // world units per history row (15 units a second), flat out
const Z_FAR: f32 = 40.0;
const H_SCALE: f32 = 3.0;
const H_MAX: f32 = 3.45;     // nothing is taller than this
const ENV_W: f32 = 3.0;      // the land rises over this distance from the road
const ENV_K: f32 = 1.125 * H_MAX / ENV_W;
const SPAN: f32 = 26.0;      // bass to treble takes this many units
const WATER: f32 = 0.3;      // quiet land lies this far under the water
const FOG: f32 = 0.0017;
const HIST_INV: vec2f = vec2f(1.0 / 128.0, 1.0 / 256.0);

var<private> gPx: f32;     // one pixel, in aspect() units
var<private> gRow0: f32;   // history row under the camera
var<private> gRowZ: f32;   // world units per history row: how fast the land rolls
var<private> gRows: f32;   // and history rows per world unit
// What uReact turns down; all of these are 1 flat out.
var<private> gPace: f32;   // the speed of what travels or churns
var<private> gGlow: f32;   // how far the sound that made the land brightens the grid
var<private> gAlong: f32;  // and how far it brightens the lines along the road
var<private> gTall: f32;   // how high the spectrum lifts the land, times H_SCALE
var<private> gScroll: f32; // how far the land has rolled
// The voices.
var<private> gKickCol: vec3f;
var<private> gSnareCol: vec3f;
var<private> gHatCol: vec3f;
var<private> gBassCol: vec3f;
var<private> gTuneCol: vec3f;
var<private> gK: f32;      // KICK: the push of the last kicks
var<private> gBass: f32;   // BASS: the size of its swell
var<private> gBassV: f32;  // BASS: 1 while there is a bass line to colour things
var<private> gSwell: f32;  // BASS: how far the land beside the road stands taller
var<private> gTune: f32;   // TUNE: how brightly the aurora shows
var<private> gNote: f32;   // TUNE: the pluck of the last notes
var<private> gGain: f32;
var<private> gHue: f32;    // palette offset from the harmony
var<private> gEcl: f32;    // 1 when the kick is gone: the sun is eclipsed
var<private> gDay: f32;
var<private> gSun: vec2f;
var<private> gSunR: f32;
var<private> gMoon: vec2f;
var<private> gBig: vec4f;  // which of the last kicks came after a long gap
var<private> gDrop: f32;   // the push of such a kick
var<private> gWarp: f32;   // a riser stretches the stars
var<private> gHaze: vec3f;
var<private> gSunCol: vec3f;
var<private> gStarCol: vec3f;
var<private> gSunDir: vec3f;

// ---- terrain ---------------------------------------------------------------

// The history texture through a bicubic B-spline (four bilinear taps), so the
// 8-bit rows and columns become a surface with continuous curvature. `s` is in
// texels, whole numbers at texel centres.
fn hsmooth(s: vec2f) -> f32 {
  let i = floor(s);
  let f = s - i;
  let f2 = f * f;
  let f3 = f2 * f;
  let m = 1.0 - f;
  let w0 = m * m * m;
  let w1 = 3.0 * f3 - 6.0 * f2 + 4.0;
  let g1 = 1.0 - (w0 + w1) / 6.0;
  let a = (i - 0.5 + w1 / (w0 + w1)) * HIST_INV;
  let b = (i + 1.5 + f3 / (6.0 * g1)) * HIST_INV;
  let s00 = textureSampleLevel(tHist, sHist, a, 0.0).r;
  let s10 = textureSampleLevel(tHist, sHist, vec2f(b.x, a.y), 0.0).r;
  let s01 = textureSampleLevel(tHist, sHist, vec2f(a.x, b.y), 0.0).r;
  let s11 = textureSampleLevel(tHist, sHist, b, 0.0).r;
  return mix(mix(s00, s10, g1.x), mix(s01, s11, g1.x), g1.y);
}

// 0 at the road (bass), 1 at SPAN (treble), then folding back.
fn triOf(ax: f32) -> f32 {
  let u = max(ax - ROAD, 0.0) / SPAN;
  return 1.0 - abs(fract(u * 0.5) * 2.0 - 1.0);
}

// What the land can do at x, whatever the music: (position from bass to
// treble, 0 in the tall bass country to 1 in the low fields beyond, the
// height a full-scale sound reaches).
fn site(x: f32, z: f32) -> vec3f {
  let ax = abs(x);
  // The frequency axis meanders as it goes, differently on each side, so a
  // held note is a winding ridge and not a straight wall.
  let zt = z + gScroll;
  let sg = sign(x);
  let wob = 0.9 * sin(zt * 0.29 + sg * 1.3) + 0.45 * sin(zt * 0.71 - sg * 2.1 + ax * 0.35);
  let tri = triOf(ax + wob * sstep(ROAD, ROAD + 5.0, ax));
  let out = sstep(0.25, 0.5, tri);
  let amp = gTall * sstep(ROAD, ROAD + ENV_W, ax) * mix(1.0, 0.4, out) * (1.0 + gSwell * (1.0 - out));
  return vec3f(tri, out, amp);
}

// Where a point on the ground reads the history, in texels.
fn texel(st: vec3f, z: f32) -> vec2f {
  return vec2f((0.06 + 0.9 * st.x) * 128.0 - 0.5, gRow0 + z * gRows);
}

// Height above the water of a spectrogram value.
fn shape(v: f32, st: vec3f) -> f32 {
  return pow(min(v * gGain, 1.0), 2.5 - st.y) * st.z - WATER;
}

// (height above the water, spectrogram value) at a point on the ground.
fn field(x: f32, z: f32) -> vec2f {
  let st = site(x, z);
  let v = hsmooth(texel(st, z));
  return vec2f(shape(v, st), min(v * gGain, 1.0));
}

// March the ray x = ox + vx z, y = oy + vy z over the land from z0 to z1 and
// return the depth of the hit, or -1. Above the tallest the land can be here
// the ray flies without looking. Below that one plain tap tells whether the
// ground is near; only then is the smooth surface consulted. Steps follow the
// clearance, never longer than the thinnest ridge of this country (times `q`:
// reflections can be coarser), and the crossing is then refined.
fn march(ox: f32, oy: f32, vx: f32, vy: f32, z0: f32, z1: f32, n: i32, q: f32) -> f32 {
  let avx = max(abs(vx), 1e-4);
  var z = z0;
  var zp = z0;
  var dp = 1.0;
  var d = 1.0;
  var hit = false;
  for (var i = 0; i < n; i++) {
    let x = ox + vx * z;
    let st = site(x, z);
    let y = oy + vy * z;
    let over = y - (st.z - WATER);
    // The tallest the land can be rises steeply beside the road and hardly
    // at all elsewhere; heading inwards, stop at the far side of the road.
    let ax = abs(x);
    let rise = select(0.6, 1.75, ax < ROAD + ENV_W);
    var dz = over * 0.9 / max(rise * avx - vy + 0.3, 0.05);
    if (x * vx < 0.0) {
      dz = min(dz, (ax + ROAD) / avx + 0.01);
    }
    if (dz > 0.15) {
      d = over;
    } else {
      let s = texel(st, z);
      d = y - shape(textureSampleLevel(tHist, sHist, (s + 0.5) * HIST_INV, 0.0).r, st) - 0.25;
      if (d < 0.1) {
        d = y - shape(hsmooth(s), st);
        if (d < 0.0) {
          hit = true;
          break;
        }
      }
      let cap = min(0.3, mix(0.5, 0.2, st.y) / avx) * (1.0 + 0.03 * z) * q;
      dz = clamp(d * 0.4, 0.02 + 0.012 * z, cap);
    }
    if (z >= z1) {
      break;
    }
    zp = z;
    dp = d;
    z = min(z + dz, z1);
  }
  if (!hit) {
    return -1.0;
  }
  for (var j = 0; j < 4; j++) {
    let zm = zp + (z - zp) * select(dp / (dp - d), 0.5, j % 2 == 0);
    let dm = oy + vy * zm - field(ox + vx * zm, zm).x;
    if (dm < 0.0) {
      z = zm;
      d = dm;
    } else {
      zp = zm;
      dp = dm;
    }
  }
  return zp + (z - zp) * dp / (dp - d);
}

// A filtered grid line: `c` is the grid coordinate (lines on whole numbers),
// `w` how much it changes across a pixel, `n` the line's share of the cell.
// The line never gets thinner than a pixel; it gets dimmer instead, and ends
// as the average brightness of the grid.
fn gridLine(c: f32, w: f32, share: f32) -> f32 {
  let n = min(share, 0.015 * w / gPx);
  let dw = clamp(n, w, 0.5);
  let aa = w * 1.5;
  let g = 1.0 - abs(fract(c) * 2.0 - 1.0);
  var l = sstep(dw + aa, dw - aa, g);
  l *= clamp(n / dw, 0.0, 1.0);
  return mix(l, n, clamp(w * 2.0 - 1.0, 0.0, 1.0));
}

// The grid at (gx, gz), with a finer one fading in close up: (the lines that
// run along the road, the lines that cross it). `thin` narrows them where
// the ground is steep, so they keep their width on the slope.
fn grid(gx: f32, gz: f32, wx: f32, wz: f32, thin: vec2f) -> vec2f {
  let fine = 0.4 * sstep(0.03, 0.008, max(wx, wz));
  return vec2f(
    max(gridLine(gx, wx, 0.035 * thin.x), fine * gridLine(gx * 2.0, wx * 2.0, 0.03 * thin.x)),
    max(gridLine(gz, wz, 0.035 * thin.y), fine * gridLine(gz * 2.0, wz * 2.0, 0.03 * thin.y)),
  );
}

// A single line of half-width `hw` at distance `d`, `w` world units a pixel.
fn lineCov(d: f32, w: f32, hw: f32) -> f32 {
  let dw = max(hw, 0.5 * w);
  return sstep(dw + 0.75 * w, dw - 0.75 * w, d) * clamp(hw / dw, 0.0, 1.0);
}

// The light of the grid: every piece of land glows with what sounded when it
// was made (`v`). `net` is (along, across, contour). Where the land is the
// bass's (`low`: beside the road, near the camera) the lines take the bass's
// colour while there is a bass line, and glow with its swell.
fn gridLight(tri: f32, h: f32, v: f32, zt: f32, net: vec3f, low: f32) -> vec3f {
  // (The lines along the road glow a little less than those across it when
  // the reactivity is down.)
  let glow = 1.5 * gGlow * v * v;
  let lit = max(max(net.x * (0.2 + gAlong * glow), net.y * (0.2 + glow)), net.z * 0.4 * (0.2 + glow));
  let hue = 0.55 + gHue + 0.2 * tri + 0.06 * h + 0.05 * sin(zt * 0.21 - uPhase * 0.7);
  // The scenery steps aside: where the bass lives, land that the palette
  // would paint in the bass's own colour goes pale instead, so that colour
  // there always means the bass.
  var own = pal(hue);
  own = mix(own, vec3f(0.6), 0.8 * low * sstep(0.45, 0.15, distance(own, gBassCol)));
  return mix(own, gBassCol * (1.0 + 0.5 * gBass), low * gBassV) * lit;
}

// How far a place on the ground belongs to the bass: `out` is site().y.
fn lowLand(out: f32, z: f32) -> f32 {
  return (1.0 - out) * sstep(24.0, 6.0, z);
}

// ---- sky -------------------------------------------------------------------

fn skyGrad(y: f32) -> vec3f {
  return mix(pal(0.7 + gHue) * 0.012, gHaze, exp(-y * 3.2));
}

fn sunGlow(p: vec2f) -> vec3f {
  let d = length(p - gSun) - gSunR;
  return gSunCol * (0.03 + gDay * (0.14 + 0.3 * gK) + 0.3 * gDrop) * exp(-max(d, 0.0) * 5.0);
}

// The colour distant things fade into.
fn hazeAt(p: vec2f) -> vec3f {
  let q = vec2f(p.x, max(p.y, HORIZON));
  return skyGrad(q.y - HORIZON) + sunGlow(q) * 0.5;
}

// TUNE. Curtains of light, one for each of the twelve notes, left to right,
// lit by the notes that are sounding. The hem rides the pitch of the tune.
fn aurora(p: vec2f) -> vec3f {
  let y = p.y - HORIZON;
  let t = uTime * 0.05;
  let wx = p.x + 0.22 * sin(y * 2.3 + t * 3.0) + 0.4 * (vnoise(vec2f(p.x * 0.9 + t, y * 1.2 - t)) - 0.5);
  let xn = wx * uRes.y / (2.0 * uRes.x) + 0.5;
  let note = chromaAt(xn);
  let hb = 0.08 + 0.56 * uPitch + 0.07 * sin(wx * 2.1 + t * 5.0);
  let yy = y - hb;
  let prof = sstep(-0.07, 0.0, yy) * exp(-max(yy, 0.0) * (5.5 - 2.5 * note));
  let rays = 0.5 + 0.5 * vnoise(vec2f(wx * 55.0, y * 1.5 + t * 8.0));
  // With nothing playing the curtains drift in the palette's own colours.
  let tint = mix(gTuneCol, pal(xn + gHue + 0.3 + yy * 0.25), uIdle);
  return tint * (note * sqrt(note) * prof * rays * gTune * (1.0 + 0.8 * gNote));
}

// Stars, laid out in rings around the vanishing point so that they can
// stretch into streaks along the direction of travel when a riser builds.
// Returns (their own light, the HATS' sparkle): each hat sparks a different
// scatter of them for an instant.
fn stars(p: vec2f) -> vec2f {
  let q = p - vec2f(0.0, HORIZON);
  let r = length(q);
  let cell = vec2f((atan2(q.x, q.y) / TAU + 0.5) * 128.0, log(max(r, 1e-3)) * 5.0);
  let id = floor(cell);
  let pick = hash21(id + 3.1);
  if (pick > 0.39 * r * r) {
    return vec2f(0.0);
  }
  let o = hash22(id + 11.7);
  let big = o.x * o.x * o.x;
  let rs = exp((id.y + 0.1 + 0.3 * o.y) * 0.2);
  let da = (fract(cell.x) - 0.3 - 0.4 * fract(o.x * 7.3)) * (TAU / 128.0) * r;
  let len = gWarp * 0.1 * rs;
  let dr = r - rs - clamp(r - rs, 0.0, len);
  let r0 = max(0.0012 + 0.0024 * big, 0.8 * gPx);
  let tw = 0.4 + 0.3 * sin(uTime * (1.0 + 3.0 * big) + pick * 900.0);
  let own = exp(-(da * da + dr * dr) / (r0 * r0)) * tw * (0.25 + 0.75 * big) * min(1.0, 4.0e-6 / (r0 * r0)) * 1.6 / (1.0 + 0.3 * len / r0);
  var tick = 0.0;
  for (var k = 0; k < 4; k++) {
    let sel = hash21(id + vec2f(uHatN - f32(k), 1.7) * 13.7);
    tick += step(0.65, sel) * hatEnv(k);
  }
  // The sparkle is a point however the star is stretched, never so small
  // that it is lost between the pixels, and faded out before the edge of the
  // star's cell.
  let r1 = max(0.0028 + 0.0025 * big, 1.4 * gPx);
  let d0 = r - rs;
  let dd = da * da + d0 * d0;
  let fc = fract(cell);
  // (Up top and round the edge of the sky: the middle is the tune's.)
  let win = sstep(0.0, 0.25, fc.x) * sstep(1.0, 0.75, fc.x) * sstep(0.0, 0.1, fc.y) * sstep(0.45, 0.9, r);
  let spark = exp(-dd / (r1 * r1)) * win * tick * (0.6 + 0.4 * big);
  return vec2f(own, spark);
}

fn segDist(p: vec2f, a: vec2f, b: vec2f) -> vec2f {
  let pa = p - a;
  let ba = b - a;
  let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
  return vec2f(length(pa - ba * h), h);
}

// SNARE. A crack for each snare or clap: one straight streak right across
// the sky, above the sun, there at once and gone with snareEnv.
fn crack(p: vec2f) -> f32 {
  var m = 0.0;
  for (var k = 0; k < 4; k++) {
    let e = snareEnv(k);
    if (e > 0.003) {
      let h = hash22(vec2f(uSnareN - f32(k), 5.3) * 7.1);
      let side = select(-1.0, 1.0, h.x > 0.5);
      let mid = vec2f((h.x - 0.5) * 1.0, HORIZON + 0.72 + 0.18 * h.y);
      let dir = normalize(vec2f(side, -0.02 - 0.07 * fract(h.y * 7.3)));
      let half = 1.0 + 0.3 * h.x;
      // The head crosses in a twentieth of a second; the tail tapers to nothing.
      let run = sstep(0.0, 0.05 / gPace, uSnareAges[k] + 1e-4);
      let sd = segDist(p, mid - dir * half, mid + dir * half * (2.0 * run - 1.0));
      let w = gPx * 0.8 + 0.004 * sd.y;
      m += e * exp(-sd.x * sd.x / (w * w)) * sd.y * (0.4 + 0.6 * sd.y);
    }
  }
  return m;
}

fn moonAt(p: vec2f) -> f32 {
  return sstep(gPx, -gPx, length(p - gMoon) - gSunR * 1.02) * sstep(0.0, 0.5, gEcl);
}

// KICK. The sun's disc, as (colour, cover): stripes that sink. Every kick
// pushes its light up.
fn sunDisc(p: vec2f) -> vec4f {
  let c = p - gSun;
  let disc = sstep(gPx, -gPx, length(c) - gSunR);
  let gap = (0.16 - c.y) / (gSunR + 0.16) * 0.8;
  let sy = c.y * 14.0 + uTime * 0.25;
  let aaw = 14.0 * gPx * 1.5;
  let edge = 0.5 - 0.5 * max(gap, 0.0) + aaw;
  let lit = sstep(edge + aaw, edge - aaw, abs(fract(sy) - 0.5));
  let hgt = clamp(c.y / gSunR * 0.5 + 0.5, 0.0, 1.0);
  let hot = mix(gKickCol, vec3f(1.0), 0.3 * hgt * hgt);
  return vec4f(hot * (0.5 + 0.55 * hgt) * (0.8 + 0.5 * gK), disc * lit * (1.0 - moonAt(p)));
}

// (How much the moon covers this point, how much of the corona shows here:
// none behind the moon or in front of the disc).
fn covers(p: vec2f) -> vec2f {
  let moon = moonAt(p);
  return vec2f(moon, (1.0 - moon) * sstep(-gPx, gPx, length(p - gSun) - gSunR));
}

// Everything in the sky but the sun's disc; `cv` is covers(p), or its
// average where a rough mirror smears the view. Mirrors (`refl`) leave out
// the finest things.
fn skyBack(p: vec2f, refl: bool, cv: vec2f) -> vec3f {
  let y = max(p.y - HORIZON, 0.0);
  var col = skyGrad(y);
  let c = p - gSun;
  let r = length(c);
  let d = r - gSunR;
  let moon = cv.x;

  var far = aurora(p);
  if (!refl) {
    let st = stars(p) * sstep(0.02, 0.35, y);
    // (Against the night sky the sparkles need less light to show.)
    far += gStarCol * (st.x * (0.25 + 0.75 * gEcl)) + gHatCol * (st.y * (10.0 - 3.5 * gEcl));
    far += gSnareCol * (crack(p) * 6.0);
  }
  col = col * (1.0 - 0.6 * moon) + (far + sunGlow(p)) * (1.0 - moon);

  // Corona. In eclipse, slow streamers round the moon, in the scenery's
  // colours. With the sun out it is the kick's, and each kick pushes it out.
  let ang = atan2(c.x, c.y);
  let fa = abs(ang) / PI;
  let rr = max(d, 0.0);
  let streak = 0.6 * vnoise(vec2f(ang * 14.0, uTime * 0.25)) + 0.4 * vnoise(vec2f(ang * 47.0 + 5.0, uTime * 0.4 - rr * 3.0));
  let len = 0.025 + 0.07 * gEcl * streak + 0.07 * gDay * gK;
  let rim = exp(-rr / len) * (0.5 + 0.8 * streak) * mix(0.12 + 0.5 * gK, 0.4, gEcl);
  // (Pearl, with a tint of the palette: a kick's colour here, with no kick
  // playing, would say there was one.)
  let rimCol = mix(gKickCol, mix(pal(0.08 + gHue + 0.2 * fa), vec3f(1.0), 0.6), gEcl);
  col += rimCol * rim * cv.y;

  // KICK. A heavy ring rolls off the sun with each kick: out fast, then
  // slowing; a much bigger one when the kick comes back.
  var rings = 0.0;
  for (var k = 0; k < 4; k++) {
    let a = uKickAges[k];
    let e = kickEnv(k);
    if (e > 0.003) {
      let big = gBig[k];
      let out = (0.24 + 0.5 * big) * (1.0 - exp(-a * 4.0 * gPace));
      let q = (r - (gSunR + 0.03 + out)) / (0.010 + (0.05 + 0.06 * big) * a);
      rings += exp(-q * q) * e * (1.0 + 3.0 * big);
    }
  }
  return col + gKickCol * rings * 0.4;
}

fn sky(p: vec2f) -> vec3f {
  let sun = sunDisc(p);
  return mix(skyBack(p, false, covers(p)), sun.rgb, sun.a);
}

// ---- ground ----------------------------------------------------------------

// The land where a ray hit it at P, without fog.
fn surface(P: vec3f, vx: f32, vy: f32) -> vec3f {
  let e = 0.09;
  let f0 = field(P.x, P.z);
  let fx = field(P.x + e, P.z).x;
  let fz = field(P.x, P.z + e).x;
  let n = normalize(vec3f(f0.x - fx, e, f0.x - fz));
  // Height as the land's colour knows it: of what it would be flat out.
  let h = (f0.x + WATER) * (H_SCALE / gTall);
  let st = site(P.x, P.z);
  let thin = inverseSqrt(vec2f(1.0) + vec2f(f0.x - fx, f0.x - fz) * vec2f(f0.x - fx, f0.x - fz) / (e * e));

  // How far the hit point moves across the surface per pixel.
  let nv = min(n.x * vx + n.y * vy + n.z, -1e-3);
  let eps = gPx / FOC * P.z;
  let wx = eps * (abs(1.0 - vx * n.x / nv) + abs(vx * n.y / nv));
  let wz = eps * (abs(n.x) + abs(n.y)) / -nv;

  var col = pal(0.85 + gHue) * 0.006;
  // Contours every quarter unit of height, drawn only where the ground is
  // steep: they net the cliffs the grid runs along, ring the shore, and
  // slide up and down the slopes as the bass line swells the land.
  let wh = eps * (abs(vy * n.x / nv) + abs(1.0 - vy * n.y / nv));
  let steep = length(n.xz);
  let net = vec3f(grid(P.x, gRow0 * gRowZ + P.z, wx, wz, thin), gridLine(f0.x * 4.0, wh * 4.0, 0.09 * steep));
  let low = lowLand(st.y, P.z);
  col += gridLight(st.x, h, f0.y, P.z + gScroll, net, low);
  // BASS. Its glow fills the glass of the land beside the road.
  col += gBassCol * (gBassV * gBass * low * 0.05);

  // Black glass: the slopes pick up the haze and the sun at grazing angles.
  let rdn = normalize(vec3f(vx, vy, 1.0));
  let fres = pow(1.0 - clamp(-dot(n, rdn), 0.0, 1.0), 4.0);
  let rfl = reflect(rdn, n);
  let sd = max(dot(rfl, gSunDir), 0.0);
  col += fres * (gHaze * 0.25 * (1.0 - 0.7 * clamp(rfl.y, 0.0, 1.0)) + gSunCol * (pow(sd, 40.0) * 0.8 + pow(sd, 6.0) * 0.1) * (0.2 + 0.8 * gDay));
  return col;
}

// 1 when the kick has stopped: `since` is how long ago the last one was and
// `gap` how long it had been silent before that. A lone thump after a long
// silence (a tom, the start of a riser) only counts as the kick coming back
// if another follows soon.
fn kickGone(since: f32, gap: f32) -> f32 {
  return max(sstep(1.5, 3.0, since), sstep(1.5, 3.0, gap) * sstep(0.7, 1.2, since));
}

fn scene(uv: vec2f) -> vec3f {
  gPx = 2.0 / uRes.y;
  // Reactivity. What the voices set off gets smaller and softer in step with
  // it (the shared envelopes see to the hits; `size` is the same measure for
  // the bass line's swell); the land, which never stops moving, gives most of
  // what it has to give by the middle of the dial.
  let r2 = uReact * uReact;
  let size = mix(0.35, 1.0, uReact);
  gPace = mix(0.5, 1.0, uReact);
  // The idle swell is no reaction to anything, and keeps its height and glow.
  gGlow = mix(mix(0.33, 1.0, r2), 1.0, uIdle);
  gAlong = mix(mix(0.6, 1.0, uReact), 1.0, uIdle);
  gTall = H_SCALE * mix(mix(0.7, 1.0, r2), 1.0, uIdle);
  // The land rolls at down to two thirds of its speed: any slower and what
  // is in view would be longer than the history. In 32nds of full speed, so
  // that the history's 256 rows are a whole number of grid cells and of road
  // markings, and the lines pass its seam unbroken.
  gRowZ = ROW_Z * round(32.0 * mix(0.65, 1.0, r2 * uReact)) / 32.0;
  gRows = 1.0 / gRowZ;
  gRow0 = uHistHead - 2.0 - Z_FAR * gRows;
  gScroll = uTime * 60.0 * gRowZ;
  gGain = 1.0 + 1.3 * uIdle;
  gHue = uHarmony;

  // The voices.
  gKickCol = voiceCol(KICK);
  gSnareCol = voiceCol(SNARE);
  gHatCol = voiceCol(HATS);
  gBassCol = voiceCol(BASS);
  gTuneCol = voiceCol(TUNE);
  gK = kickSum();
  // BASS. Its colour is there while it plays (and not while idle, when
  // nothing does); its swell is what breathes. Under a fifth, what is left
  // in the low end is mostly the tails of the kicks.
  gBassV = sstep(0.2, 0.45, uBassLine) * (1.0 - uIdle);
  gBass = sstep(0.2, 0.4, uBassLine) * uBassLine * mix(size, 1.0, uIdle);
  gSwell = 0.15 * gBass;
  // TUNE. The aurora is as bright as there is tune; idle, it drifts.
  gTune = mix(sstep(0.08, 0.45, uTune) * (0.5 + 0.5 * uTune), 0.9, uIdle);
  gNote = noteSum();

  // The sun IS the kick. A moon covers it when the kick has been gone a
  // while, and is thrown off in under half a second when the kick comes back.
  let since = uKickAges.x;
  let gap = uKickAges.y - since;
  let before = kickGone(gap, uKickAges.z - uKickAges.y) * sstep(0.45, 0.0, since);
  gEcl = max(kickGone(since, gap), before);
  gDay = 1.0 - gEcl;
  // A riser (bright and full; hats on their own are bright but thin).
  gWarp = sstep(0.48, 0.70, uCentroid) * sstep(0.25, 0.5, uEnergy) * mix(0.25, 1.0, uReact);
  gSun = vec2f(0.0, HORIZON + 0.24);
  gSunR = 0.38 + 0.035 * gK;
  gMoon = gSun + vec2f(0.8, 0.6) * (1.0 - gEcl) * 2.3 * gSunR;
  gBig = vec4f(0.0);
  gDrop = 0.0;
  for (var k = 0; k < 3; k++) {
    let b = sstep(1.2, 3.0, uKickAges[k + 1] - uKickAges[k]);
    gBig[k] = b;
    gDrop += b * kickEnv(k);
  }
  // The haze on the horizon: the scenery's by night, the sun's light by day.
  gHaze = mix(pal(0.85 + gHue) * 0.05, gKickCol * 0.12, gDay);
  // The light round the sun: the kick's by day, the scenery's in eclipse.
  gSunCol = mix(mix(pal(0.08 + gHue), vec3f(1.0), 0.6), gKickCol, gDay);
  gStarCol = mix(pal(0.62 + gHue), vec3f(1.0), 0.35);
  gSunDir = normalize(vec3f(gSun.x / FOC, (gSun.y - HORIZON) / FOC, 1.0));

  let p0 = aspect(uv);
  let roll = 0.025 * cos(uTime * 0.17);
  let p = rot(roll) * (p0 - vec2f(0.0, HORIZON)) * (1.0 - 0.012 * gK) + vec2f(0.0, HORIZON);
  let cx = 0.35 * sin(uTime * 0.17);
  let cy = CAM_H;
  let vx = p.x / FOC;
  let vy = (p.y - HORIZON) / FOC;

  // Where can this ray meet the land at all? Not over the road, not above
  // the wedge the land rises in, not below the water.
  let den = ENV_K * abs(vx) - vy;
  var z1 = 1e9;
  if (den > 1e-5) {
    z1 = (cy + ENV_K * (ROAD - select(-1.0, 1.0, vx >= 0.0) * cx)) / den;
  }
  let zg = select(1e9, cy / max(-vy, 1e-6), vy < 0.0);
  var zend = min(Z_FAR, zg);
  if (vy > 0.0) {
    zend = min(zend, (H_MAX - WATER - cy) / vy);
  }

  var zh = -1.0;
  if (z1 < zend) {
    zh = march(cx, cy, vx, vy, z1, zend, 128, 1.0);
  }
  if (zh > 0.0) {
    let col = surface(vec3f(cx + vx * zh, cy + vy * zh, zh), vx, vy);
    return mix(col, hazeAt(p), 1.0 - exp(-zh * zh * FOG));
  }
  if (vy >= 0.0) {
    return sky(p);
  }
  if (zg >= Z_FAR) {
    return hazeAt(p);
  }

  // The flat: road and standing water, both mirrors.
  let z = zg;
  let x = cx + vx * z;
  let ax = abs(x);
  let eps = gPx / FOC * z;
  let wx = eps * (1.0 + abs(vx / vy));
  let wz = eps / -vy;
  let f0 = field(x, z);
  let depth = max(-f0.x, 0.0);
  let road = clamp((ROAD - ax) / max(wx, 1e-4) + 0.5, 0.0, 1.0);
  let zr = gRow0 * gRowZ + z;
  let zt = z + gScroll;
  let base = pal(0.85 + gHue) * 0.006;
  let rdn = normalize(vec3f(vx, vy, 1.0));
  let fres = mix(0.08, 0.85, pow(1.0 - abs(rdn.y), 3.0));

  var col = base;
  var wet = 0.0;
  let st = site(x, z);
  let low = lowLand(st.y, z);
  // How the mirror bends what it shows: reflected slope scale, sideways push.
  var bends = array<vec2f, 3>(vec2f(1.0, 0.0), vec2f(1.0, 0.0), vec2f(1.0, 0.0));
  if (road < 1.0) {
    // Ripples, which swell with the bass line.
    let calm = sstep(1.2, 0.3, wz * 2.0) * (1.0 + 1.2 * gBass);
    let r1 = (vnoise(vec2f(x * 1.7 + uTime * 0.2 * gPace, zt * 2.0)) - 0.5) * calm;
    let r2 = (vnoise(vec2f(x * 4.1 - uTime * 0.3 * gPace, zt * 5.0 + 3.0)) - 0.5) * calm;
    let bw = vec2f(1.0 + 0.08 * r1 + 0.04 * r2, 0.03 * r2);
    bends = array<vec2f, 3>(bw, bw, bw);
    // The lake bed's grid shows through, bent by the water over it.
    let deep = sstep(0.0, 0.15, depth);
    let r3 = (vnoise(vec2f(x * 1.3 + 5.0, zt * 1.5 - uTime * 0.25 * gPace)) - 0.5) * calm;
    let sway = vec2f(r3, r1) * 0.14 * deep;
    let net = grid(x + sway.x, zr + sway.y, wx, wz, vec2f(1.0)) * (1.0 - 0.4 * deep);
    col += gridLight(st.x, 0.0, f0.y * (1.0 - deep), zt, vec3f(net, 0.0), low);
    wet = fres * sstep(0.0, 0.1, depth);
  }
  if (road > 0.0) {
    var rc = base;
    let glassy = sstep(0.42, 0.62, vnoise(vec2f(x * 1.1 + 3.0, zt * 0.3)));
    let rough = mix(1.0, 0.1, glassy) * sstep(0.6, 0.15, wx * 60.0);
    // Three takes on the roughness of the tarmac, to blur what it mirrors
    // into streaks that run with the road.
    for (var j = 0; j < 3; j++) {
      let n1 = vnoise(vec2f(x * 60.0 + f32(j) * 17.3, zt * 1.0)) - 0.5;
      let n2 = vnoise(vec2f(x * 23.0 + 7.0 + f32(j) * 31.7, zt * 0.6)) - 0.5;
      let rb = vec2f(mix(1.0, 0.9 + 0.3 * n1 + 0.5 * n2, rough), 0.3 * n2 * rough);
      bends[j] = mix(bends[j], rb, road);
    }
    // BASS. The road's lines take the bass's colour, glow and fatten with it.
    // (Unlit, they are plain white paint: no colour of their own to mistake
    // for it.)
    let sh = mix(vec3f(0.28), gBassCol * (0.7 + 1.3 * gBass), gBassV);
    let fat = 1.0 + 0.8 * gBass;
    let dash = abs(fract(zr * 0.5) - 0.5) * 2.0;
    let dc = mix(sstep(0.5 + 0.75 * wz, 0.5 - 0.75 * wz, dash), 0.5, clamp(wz - 0.5, 0.0, 1.0));
    rc = mix(rc, sh, lineCov(ax, wx, 0.035 * fat) * dc);
    col = mix(col, rc, road);
    wet = mix(wet, fres * mix(0.7, 1.0, glassy), road);
    col = mix(col, sh, lineCov(abs(ax - ROAD), wx, 0.045 * fat));
  }

  // And their light spills on to the wet ground beside them.
  col += gBassCol * (gBassV * gBass * 0.10 * exp(-abs(ax - ROAD) * 2.2) * sstep(24.0, 6.0, z));

  if (wet > 0.003) {
    let bend = bends[0];
    let rvy = max(-vy * bend.x, -vy * 0.3);
    let rvx = vx + bend.y * -vy;
    let pr = vec2f(rvx * FOC, rvy * FOC + HORIZON);
    let ox = x - rvx * z;
    let oy = -rvy * z;
    var zs = z;
    var go = true;
    if (road >= 1.0) {
      let dr = ENV_K * abs(rvx) - rvy;
      go = dr > 1e-5;
      if (go) {
        zs = max((oy + ENV_K * (ROAD - select(-1.0, 1.0, rvx >= 0.0) * ox)) / dr, z);
      }
    }
    let ze = min(Z_FAR, z + (H_MAX - WATER) / rvy);
    var zm = -1.0;
    if (go && zs < ze) {
      zm = march(ox, oy, rvx, rvy, zs, ze, 48, 1.6);
    }
    var refl: vec3f;
    if (zm > 0.0) {
      refl = mix(surface(vec3f(ox + rvx * zm, oy + rvy * zm, zm), rvx, rvy), hazeAt(pr), 1.0 - exp(-zm * zm * FOG));
    } else {
      // The discs of the sun and the moon are the hard edges up there: on
      // the road, look at them three times.
      var sun = sunDisc(pr);
      sun = vec4f(sun.rgb * sun.a, sun.a);
      var cv = covers(pr);
      if (road > 0.0) {
        for (var j = 1; j < 3; j++) {
          let b = bends[j];
          let pj = vec2f((vx + b.y * -vy) * FOC, max(-vy * b.x, -vy * 0.3) * FOC + HORIZON);
          let t = sunDisc(pj);
          sun += vec4f(t.rgb * t.a, t.a);
          cv += covers(pj);
        }
        sun /= 3.0;
        cv /= 3.0;
      }
      refl = skyBack(pr, true, cv) * (1.0 - sun.a) + sun.rgb;
    }
    col += refl * wet;
  }
  return mix(col, hazeAt(p), 1.0 - exp(-z * z * FOG));
}
