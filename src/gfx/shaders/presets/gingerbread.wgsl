// Gingerbread: a flight down a hall with no end, its pillars and vaults made
// of chains of pearls made of chains of pearls.
//
// The hall is one small rule applied over and over (the fold-and-invert rule
// behind the Mandelbox and the fractals called pseudo-Kleinian): fold all of
// space into a box, mirror fashion, then turn the inside of the unit sphere
// inside out, and at the end see how far the point has come from the floor.
// The folds make the hall repeat every 4 * CS in each direction, so the
// flight is endless and the camera never leaves the first cell. The floor is
// at z = 0 and the vault at z = 2 * CS.z; the camera keeps to the open air
// half way up, drifting from side to side.
//
// The five voices (see preset_header.wgsl and fractal.wgsl). Depth is pitch,
// and here depth is height: the floor at depth 0, the vault at depth 1.
//
//   voice  what it does                         where                colour
//   -----  -----------------------------------  -------------------  ------
//   KICK   a wave of light runs up the hall     the whole hall,      KICK
//          from its far end to the eye (the     from its far end at
//          kick's age is how far it has come),  the centre of the
//          the far end flares, and the flight   picture
//          is pushed on a little
//   SNARE  light in the seams (snareEnv): for   the joints between   SNARE
//          an instant the joints between the    the pearls, in a
//          pearls glow, as if the hall were     swath across the
//          strung on light                      picture, a new one
//                                               each hit
//   HATS   a tick (hatEnv): glints in the       the vault overhead   HATS
//          tracery, a new scatter each tick
//   BASS   swells with uBassLine: the floor     the floor and the    BASS
//          and the feet of the pillars glow,    air just above it
//          and so does the air along the floor
//   TUNE   a line of light round every pillar   the pillars, higher  TUNE
//          at one height, which rises with the  as the pitch rises
//          pitch (tuneBand); a new note plucks
//          it brighter
//
// Scenery, in pal() colours and dimmer than anything a voice lights:
//
// - The hall itself, coloured by how each point was folded, under a lamp
//   ahead and to one side, with the far end glowing through the mist.
// - Fullness (uEnergy): how far the eye reaches. In a breakdown the mist
//   closes in to the nearest pillars; a full mix shows the hall to its end.
// - Harmony (uHarmony): turns the palette.
// - Idle: no events and the voices' colours are put away; the flight goes on.
//
// Reactivity (uReact): the envelopes carry it. Besides them the flight and
// the kick's wave are slower below 1, the kick's push is smaller, and the
// bass's glow is lighter.

const CS: vec3f = vec3f(0.92, 0.90, 0.92);
const ITER: i32 = 13;
const MARCH: i32 = 96;
const FAR: f32 = 18.0;

struct Field {
  d: f32,
  tr: vec3f,
}

// How far p is from the hall, at least, and how it was folded on the way.
// Every pass of the loop only adds finer pearls to what the passes before
// drew, `k` times smaller than the hall itself, so the loop stops once they
// would be finer than `fine`: no finer than a pixel, where p is on screen.
fn field(p0: vec3f, fine: f32) -> Field {
  var p = p0;
  var k = 1.0;
  var tr = vec3f(1e9);
  for (var i = 0; i < ITER; i++) {
    // Fold all of space into one box, mirror fashion.
    let q = p - CS;
    p = abs(q - 4.0 * CS * floor(q / (4.0 * CS)) - 2.0 * CS) - CS;
    // Inside the unit sphere, turn it inside out.
    let r2 = dot(p, p);
    tr = min(tr, vec3f(r2, abs(p.x), abs(p.y)));
    // Outside the sphere nothing more happens to p, on this pass or any
    // later one: it is in the open air.
    if (r2 >= 1.0) { break; }
    let s = 1.0 / r2;
    p *= s;
    k *= s;
    if (k > fine) { break; }
  }
  return Field(0.5 * abs(p.z) / k, tr);
}

fn scene(uv: vec2f) -> vec3f {
  let ps = aspect(uv);
  let px = 2.0 / uRes.y;

  // The flight: straight down the hall, drifting from side to side, with the
  // kick a push forward.
  let trav = (uTime * 0.09 + uPhase * 0.025) * fracPace() + 0.018 * fracAmp() * divePush();
  let ro = vec3f(0.92 + 0.26 * sin(trav * 0.21), fmod(trav, 4.0 * CS.y), 0.92 + 0.10 * sin(trav * 0.37));
  let fw = normalize(vec3f(0.26 * 0.21 * cos(trav * 0.21) + 0.12 * sin(trav * 0.11), 1.0, 0.08 * sin(trav * 0.29)));
  let rt = normalize(cross(fw, vec3f(0.0, 0.0, 1.0)));
  let up = cross(rt, fw);
  let q = rot(0.07 * sin(trav * 0.13)) * ps;
  let rd = normalize(fw * 1.35 + rt * q.x + up * q.y);

  // Fullness: how far the eye reaches through the mist.
  let full = mix(sstep(0.35, 0.9, uEnergy), 0.6, uIdle);
  let thick = mix(0.30, 0.16, full);

  var t = 0.02;
  var hit = false;
  var steps = 0.0;
  var low = 0.0;
  var f = Field(1.0, vec3f(0.0));
  var fine = 1.0;
  for (var i = 0; i < MARCH; i++) {
    let p = ro + rd * t;
    fine = 0.4 / (t * px);
    f = field(p, fine);
    if (f.d < max(0.0005, t * px * 1.1)) {
      hit = true;
      break;
    }
    let step = f.d * 0.9;
    // The air near the floor, for the bass to light.
    low += step * exp(-p.z / 0.16) * exp(-t * 0.2);
    t += step;
    steps += 1.0;
    if (t > FAR) { break; }
  }

  // The far end of the hall, which the kick flares.
  let ahead = max(dot(rd, fw), 0.0);
  let mist = pal(uHarmony + 0.55) * 0.03 + pal(uHarmony + 0.1) * 0.25 * pow(ahead, 40.0)
    + voiceCol(KICK) * kickSum() * 0.7 * pow(ahead, 60.0);

  var col = mist;
  if (hit) {
    let p = ro + rd * t;
    // The slope of the distance, from four points round p.
    let e = vec2f(1.0, -1.0) * max(0.0005, t * px);
    let n = normalize(
      e.xyy * field(p + e.xyy, fine).d + e.yyx * field(p + e.yyx, fine).d
        + e.yxy * field(p + e.yxy, fine).d + e.xxx * field(p + e.xxx, fine).d,
    );
    // How open the place is: from how hard the march was, and from the room
    // just off the surface.
    let ao = (1.0 - 0.8 * steps / f32(MARCH)) * clamp(field(p + n * 0.06, fine).d / 0.06, 0.15, 1.0);

    let base = pal(f.tr.x * 0.8 + 0.3 * f.tr.y + uHarmony + 0.25);
    let lamp = normalize(fw * 0.5 + up * 0.75 - rt * 0.45);
    let diff = max(dot(n, lamp), 0.0);
    let sky = 0.5 + 0.5 * n.z;
    let rim = pow(clamp(1.0 + dot(n, rd), 0.0, 1.0), 3.0);
    let gloss = pow(max(dot(n, normalize(lamp - rd)), 0.0), 40.0);
    var surf = base * (0.07 + 0.85 * diff + 0.22 * sky) * ao + pal(uHarmony + 0.1) * rim * 0.35 * ao;
    surf += vec3f(1.0) * gloss * 0.25 * ao;

    // What a voice lights: the faces turned to the lamp, the edges and the
    // gloss, less in the corners.
    let glow = (0.15 + 0.6 * diff + 0.5 * rim + 1.5 * gloss) * (0.3 + 0.7 * ao);

    // Depth is pitch: the floor at 0, the vault at 1.
    let depth = p.z / (2.0 * CS.z);

    // BASS: the floor and the feet of the pillars.
    let foot = exp(-p.z / 0.22) * bassOn();
    surf = surf * (1.0 - 0.5 * foot) + voiceCol(BASS) * foot * fracHeft() * (0.2 + 0.9 * glow);

    // TUNE: a line of light at one height, higher as the pitch rises.
    surf += voiceCol(TUNE) * tuneBand(depth, 0.04) * (0.4 + 1.6 * glow);

    // KICK: a wave of light from the far end of the hall to the eye.
    var wave = 0.0;
    for (var k = 0; k < 4; k++) {
      let a = uKickAges[k];
      let x = (t - (8.0 - 14.0 * fracPace() * a)) / 0.9;
      wave += exp(-x * x) * exp(-a * 1.6) * uKickHits[k];
    }
    surf += voiceCol(KICK) * min(wave * fracAmp(), 1.0) * (0.12 + 1.3 * glow);

    // HATS: glints in the tracery overhead.
    let cell = floor(p * 60.0);
    surf += voiceCol(HATS) * hatPick(cell.xy + 17.0 * cell.z, 0.12) * sstep(0.55, 0.8, depth) * (0.4 + 5.0 * gloss + 2.5 * rim);

    // SNARE: light in the seams. A seam is where there is no room just off
    // the surface, a few pixels' worth out along it.
    let swath = snareSwath(ps, 0.3);
    if (swath > 0.002) {
      let out = max(0.008, t * px * 5.0);
      let seam = 1.0 - clamp(field(p + n * out, fine).d / out, 0.0, 1.0);
      surf += voiceCol(SNARE) * swath * sstep(0.55, 0.95, seam) * 3.2;
    }

    col = mix(mist, surf, exp(-t * thick));
  }

  // BASS: the air along the floor.
  col += voiceCol(BASS) * low * bassOn() * fracHeft() * 0.5;

  return col;
}
