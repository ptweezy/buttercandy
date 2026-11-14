// Sugarplum: a tree that branches the way sound does.
//
// An octave is a doubling, and so is a fork. The trunk carries the lowest
// octave the analyser hears (30 to 60 Hz); it forks into two boughs that share
// the next (60 to 120 Hz), they fork into four, and so on up: nine forks, nine
// octaves, 512 twig tips for the top of the spectrum. Each branch carries its
// own slice of its octave, low notes to one side and high to the other, and
// the two branches it forks into carry the same slice an octave up, split in
// two. So:
//
// - Height in the tree is pitch. Low sounds are thick wood near the ground,
//   high sounds fine twigs at the edge of the crown, and a riser climbs.
// - A limb is a note. Follow any branch outward and you pass the same note in
//   every octave above it, so each note of the scale owns a sector of the
//   crown, and a chord lights as many sectors as it has notes.
// - The bass line plays the root of the chord: it is the roots.
//
// The five voices (see preset_header.wgsl). Each has one colour, one kind of
// motion and one place, and leaves no trace when it is not playing.
//
//   voice  what it does                          where                 colour
//   -----  ------------------------------------  --------------------  ---------------
//   KICK   A pulse of light rises from the foot  The trunk and the     voiceCol(KICK)
//          of the trunk and divides at every     great boughs, from
//          fork, spent before it reaches the     below: the biggest
//          twigs; the crown opens a little as    thing there is. And
//          it passes and the ground swells       along the ground,
//          outward from the trunk. A heavy       outward from the
//          push, then a slow release.            foot.
//   SNARE  One bough cracks: it and the limb it  One limb of the       voiceCol(SNARE)
//          carries flash white-hot to the        crown, a different
//          twigs, like a fork of lightning, and  one each time; a
//          are gone. A hard hit takes a bigger   quarter or an eighth
//          limb.                                 of it.
//   HATS   Blossoms twinkle, a different         The twig tips: the    voiceCol(HATS)
//          handful each time. A tick.            finest detail, at
//                                                the top and the edge.
//   BASS   Light runs out along the roots as     Under the ground:     voiceCol(BASS)
//          the bass line swells, further the     the lowest thing in
//          more of it there is, and the roots    the picture.
//          thicken; it drains back as the line
//          falls. Which roots are brightest is
//          the note being played.
//   TUNE   A sugarplum hangs under every bough   The middle of the     voiceCol(TUNE)
//          of the middle crown, and the ones on  crown. The lit band
//          the limbs of the notes now sounding   glides up and down
//          glow from inside, with those limbs.   it with uPitch. And
//          A note starting plucks them brighter. the sky to either
//          And in the sky stand twelve           side, a note to a
//          constellations, one for each note of  patch, round like a
//          the scale: each draws itself, star    clock face.
//          to star, while its note sounds.
//
// What is there whoever is playing is scenery, in pal() colours and slow:
//
//   spectrum   The sap. Every branch is a bundle of strands, one for each
//              branch it will become, and each strand glows as its slice of
//              the spectrum has been loud over the last quarter second. The
//              strands are coloured by where they sit in the octave, so the
//              trunk is every colour and a twig is one.
//   energy     The season. In a breakdown the blossom closes and dims; as the
//              music fills out the tree comes into bloom, and more petals fall.
//   treble     The air in the top of the crown stirs the blossom there.
//   harmony    Slides the palette: the blossom changes colour with the chord.
//   loudness   How hard the wind blows through the crown (uPhase).
//   silence    The tree still sways and sheds petals, the plums and the roots
//              follow the idle tune and bass line in palette colours, and
//              every few seconds a slow pulse of its own climbs the trunk.
//   uReact     The voices' envelopes already carry it. Turned down, the wind,
//              the falling petals, the turning of the tree and the rising
//              pulse all slow too.
//
// The night sky, the hills, the mist and the soil are the darkest colour the
// palette has, whatever the harmony.
//
// How it is drawn. The tree is a real one, in three dimensions, turning
// slowly: 1023 curved branches, each bending out of its parent's direction
// into its own. For each pixel the tree is walked from the trunk outward, and
// a branch is only followed if the pixel lies inside a sphere known to hold
// everything that grows from it. Those spheres are the tables below. They
// depend on every constant of the growth rule, so they are checked by a test
// (`sugarplum_bounds_hold` in src/gfx/sugarplum.rs), which grows the same tree
// from the constants in this file and prints the tables to paste here if they
// no longer hold. The roots are the same tree seen from another side, upside
// down and shallower, without its last two forks.

// ---- the growth rule --------------------------------------------------------
// (Named constants, one to a line: the test reads them from this file.)

// Forks above the trunk.
const NLEV: i32 = 9;
// How far a child leans out of its parent's direction, as a slope: the two
// children of the trunk, then the two of any other branch, and how much each
// branch varies it.
const T0A: f32 = 0.65;
const T0B: f32 = 1.0;
const TA: f32 = 0.42;
const TB: f32 = 0.84;
const TJ: f32 = 0.32;
// Low branches are drawn up towards the light; twigs hang.
const UP0: f32 = 0.07;
const UPS: f32 = 0.036;
// KICK: how much wider every fork opens at the top of a kick's push.
const HEAVE: f32 = 0.035;
// How far the wind moves a fork.
const SWAY: f32 = 0.04;

// Length of a branch at each level, the trunk first.
const LEN: array<f32, 10> = array<f32, 10>(0.2150, 0.2750, 0.2500, 0.2000, 0.1550, 0.1150, 0.0850, 0.0630, 0.0470, 0.0350);
// The sphere that holds everything growing from a branch of each level: how
// far its centre is along the branch's chord, and its radius.
const CTR: array<f32, 10> = array<f32, 10>(0.6987, 0.7150, 0.4500, 0.3900, 0.2790, 0.1898, 0.1275, 0.0788, 0.0447, 0.0193);
const RAD: array<f32, 10> = array<f32, 10>(1.0808, 0.8252, 0.6174, 0.4599, 0.3095, 0.2016, 0.1372, 0.0855, 0.0492, 0.0220);
// Room round that sphere for what is drawn off the branches: their own
// width, the plums that hang from them, the blossom.
const PAD: array<f32, 10> = array<f32, 10>(0.14, 0.085, 0.085, 0.085, 0.07, 0.058, 0.048, 0.034, 0.034, 0.034);
// Foot to twig tip along the branches.
const STOT: f32 = 1.44;

// ---- the picture ------------------------------------------------------------

// Where the ground is.
const GROUND: f32 = -0.48;
// Octaves between 30 Hz and 16 kHz, the span of spec().
const OCTAVES: f32 = 9.06;
// How much shallower the roots are than the crown is tall.
const SQUASH: f32 = 0.47;

// Set once per pixel by scene(), read by everything else.
var<private> gPx: f32; // one pixel, in scene units
var<private> gWide: f32; // half the width of the picture, in scene units
var<private> gHigh: f32; // and half its height
var<private> gHue: f32; // the palette offset the harmony asks for
var<private> gPace: f32; // uReact, as a speed
var<private> gAz: f32; // which way the tree is turned
var<private> gOsc: array<f32, 8>; // the wind: eight slow oscillators
var<private> gHeave: f32;
var<private> gColKick: vec3f;
var<private> gColSnare: vec3f;
var<private> gColHats: vec3f;
var<private> gColBass: vec3f;
var<private> gColTune: vec3f;
// The last four kicks: how far up the tree each pulse is (0 the foot, 1 the
// twigs) and how bright.
var<private> gKickAt: vec4f;
var<private> gKickAmp: vec4f;
// The tree's own pulse, when nothing is playing.
var<private> gIdleAt: f32;
var<private> gIdleAmp: f32;
// The last four snares: the level and number of the bough each cracked, and
// how bright it still is.
var<private> gSnareLv: vec4f;
var<private> gSnareJ: vec4f;
var<private> gSnareAmp: vec4f;
// The last four hats: how bright, and which handful of blossom.
var<private> gHatAmp: vec4f;
var<private> gHatId: vec4u;
var<private> gTune: f32; // how brightly the tune's plums and limbs glow
var<private> gPitchLv: f32; // the level of the tree the tune sits at
var<private> gBass: f32; // how much bass line there is
var<private> gBloom: f32; // the season, 0 bare to 1 in full bloom

// What the tree adds to a pixel.
struct Light {
  // The branches themselves: where two cross, the brighter shows.
  body: vec3f,
  // Blossom, fruit and flashes: these add up.
  add: vec3f,
  // How much of what is behind the tree is hidden.
  cover: f32,
}

// A random word for branch j of level n. Whole numbers only, so that the
// same tree grows on every GPU and in the test.
fn ih(n: u32, j: u32, k: u32) -> u32 {
  var x = j * 747796405u + n * 2891336453u + k * 1013904223u + 12345u;
  x = ((x >> ((x >> 28u) + 4u)) ^ x) * 277803737u;
  return (x >> 22u) ^ x;
}

// Ten bits of a random word, as 0..1.
fn bits(h: u32, at: u32) -> f32 {
  return f32((h >> at) & 1023u) * (1.0 / 1024.0);
}

// Only the last four hits of a voice are remembered. The oldest bows out as
// a new one arrives, so nothing is still alight when it drops off the list.
fn last4(k: i32, newest: f32) -> f32 {
  return select(1.0, sstep(0.09, 0.0, newest), k == 3);
}

// How strongly the note at a place in the octave (0..1) is sounding. The
// tree's octaves start at 30 Hz, and C is 0.1243 of an octave above that.
fn noteAt(xs: f32) -> f32 {
  return chromaAt(fract(xs - 0.1243) + 0.5 / 12.0);
}

// The spectrum at place xs of octave n, over the last quarter second: slow
// enough that no single hit makes the sap jump.
fn sapAt(n: f32, xs: f32) -> f32 {
  let x = min((n + xs) / OCTAVES, 1.0);
  return 0.5 * (histRow(x, uHistHead - 2.0) + histRow(x, uHistHead - 14.0));
}

// 0 at the back of the tree, 1 at the front.
fn nearness(z: f32) -> f32 {
  return sstep(-0.65, 0.65, z);
}

// One blossom, as light. q: from its centre. h: its own random word. xs, n:
// the place in the octave, and the octave, of the twig it grows on.
fn blossom(q: vec2f, h: u32, xs: f32, n: f32, z: f32) -> vec3f {
  let dd = dot(q, q);
  if (dd > 0.00068) {
    return vec3f(0.0);
  }
  let hb = bits(h, 0u);
  let near = nearness(z);
  // A few are big; most are small. Those at the back are out of focus.
  let rb = (0.0034 + 0.0058 * hb * hb) * (0.8 + 0.4 * gBloom) * (1.3 - 0.3 * near);
  let g = max(exp(-dd / (rb * rb)) - 0.033, 0.0);
  if (g <= 0.0) {
    return vec3f(0.0);
  }
  // Scenery: in bloom as the music fills out, stirred by the top of the
  // spectrum, each with a slow shimmer of its own.
  let air = spec(min((n + xs) / OCTAVES, 1.0));
  let shimmer = 0.75 + 0.25 * sin(uTime * (0.3 + 0.5 * hb) * gPace + TAU * bits(h, 10u));
  var c = pal(gHue + xs + 0.08 * hb) * (0.06 + 0.36 * gBloom) * shimmer * (0.6 + 2.0 * air) * (0.35 + 0.65 * near) * (1.5 - hb);
  // HATS: one blossom in sixteen twinkles, a different sixteenth each time.
  var tw = 0.0;
  for (var k = 0; k < 4; k++) {
    tw += select(0.0, gHatAmp[k], ((h >> 20u) + gHatId[k]) % 16u == 0u);
  }
  c += gColHats * tw * 4.5 * (0.5 + 0.5 * near);
  return c * (g + 0.6 * g * g * g);
}

// The tree as light at p. With `roots`, the same tree from another side,
// bare and two forks short, lit only by the bass line: the caller hands in p
// turned upside down and stretched by 1 / SQUASH, and what is drawn comes out
// shallow.
fn walk(p: vec2f, roots: bool) -> Light {
  var out: Light;
  out.body = vec3f(0.0);
  out.add = vec3f(0.0);
  out.cover = 0.0;
  var tLen = LEN;
  var tCtr = CTR;
  var tRad = RAD;
  var tPad = PAD;
  let nlev = select(NLEV, NLEV - 2, roots);
  // Half the width of the trunk, and what a branch keeps of it at each fork.
  // BASS: the roots swell.
  let w0 = select(0.044, 0.015 * (1.0 + 0.5 * gBass), roots);
  let wr = select(0.665, 0.72, roots);

  // The branch in hand: where it starts, the direction it leaves in (its
  // parent's), the direction it ends in (its own), a direction across it that
  // its fork is measured from, and (level, number in its level, distance from
  // the foot, how much of a snare's crack has reached it).
  var o = vec3f(0.0, GROUND, 0.0);
  var dp = vec3f(0.0, 1.0, 0.0);
  var d = dp;
  let az = gAz + select(0.0, 1.3, roots);
  var u = vec3f(cos(az), 0.0, sin(az));
  var m = vec4f(0.0);
  let c0 = o.xy + d.xy * tCtr[0];
  let r0 = tRad[0] + tPad[0];
  var have = dot(p - c0, p - c0) < r0 * r0;
  // Branches put by, to come back to.
  var so: array<vec3f, 10>;
  var sq: array<vec3f, 10>;
  var sd: array<vec3f, 10>;
  var su: array<vec3f, 10>;
  var sm: array<vec4f, 10>;
  var sp = 0;

  for (var it = 0; it < 1100; it++) {
    if (!have) {
      if (sp == 0) {
        break;
      }
      sp -= 1;
      o = so[sp];
      dp = sq[sp];
      d = sd[sp];
      u = su[sp];
      m = sm[sp];
    }
    have = false;
    let ni = i32(m.x);
    let n = m.x;
    let ju = u32(m.y);
    let hn = ih(u32(ni), ju, 0u);
    let L = tLen[ni] * select(0.84 + 0.32 * bits(hn, 0u), 1.0, ni == 0);
    // The branch leaves along its parent's direction and bends into its own:
    // a curve from o by way of cc to e.
    let cc = o + dp * (0.5 * L);
    let e = cc + d * (0.5 * L);

    // ---- this branch ------------------------------------------------------
    let A = o.xy;
    let b1 = cc.xy - A;
    let b2 = A - 2.0 * cc.xy + e.xy;
    let ba = e.xy - A;
    let pa = p - A;
    var t = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-9), 0.0, 1.0);
    let dchord = length(pa - ba * t);
    // The trunk spreads at its foot, and the roots where they leave it.
    let flare = select(0.0, select(1.6, 1.3, roots), ni == select(0, 1, roots));
    let wn = w0 * pow(wr, n);
    // SNARE: is this the bough that cracked?
    var own = 0.0;
    if (!roots && (ni == 2 || ni == 3)) {
      for (var k = 0; k < 4; k++) {
        own += select(0.0, gSnareAmp[k], n == gSnareLv[k] && f32(ju) == gSnareJ[k]);
      }
    }
    let crack = m.w;
    // How far from its middle the branch lights anything, in widths.
    let lit = select(1.0, 2.5, own + crack > 0.002);
    let wmax = wn * (1.0 + flare) / select(1.0, SQUASH, roots);
    if (dchord < lit * wmax + 3.0 * gPx / select(1.0, SQUASH, roots) + 0.125 * L) {
      // From the nearest point of the chord onto the curve.
      for (var i = 0; i < 2; i++) {
        let q = A + (2.0 * b1 + b2 * t) * t - p;
        let tg = b1 + b2 * t;
        t = clamp(t - dot(q, tg) / max(2.0 * dot(tg, tg) + dot(q, b2), 1e-6), 0.0, 1.0);
      }
      let q = p - (A + (2.0 * b1 + b2 * t) * t);
      let tg = normalize(b1 + b2 * t);
      let dist = length(q);
      // Under the ground the picture is squashed: a width or a pixel drawn
      // there is this much bigger in the tree's own space.
      let grow = select(1.0, length(vec2f(tg.x, SQUASH * tg.y)) / SQUASH, roots);
      let px = gPx * grow;
      let w = wn * (1.0 - (1.0 - wr) * t) * (1.0 + flare * exp(-t * 7.0)) * grow;
      if (dist < lit * w + 3.0 * px) {
        let z = o.z + (2.0 * (cc.z - o.z) + (o.z - 2.0 * cc.z + e.z) * t) * t;
        let near = nearness(z);
        let depth = 0.45 + 0.55 * near;
        // Nothing is drawn thinner than the pixels can hold: fainter instead.
        let ww = max(w, 0.7 * px);
        let thin = min(w / (0.7 * px), 1.0);
        let cov = sstep(ww + px, ww - px, dist) * thin;
        // Across the branch, -1 to 1, and how round that makes it look.
        let lat = clamp((tg.x * q.y - tg.y * q.x) / ww, -1.0, 1.0);
        let round = sqrt(max(1.0 - lat * lat, 0.0));
        // How far from the foot, 0 to about 1.
        let sN = (m.z + t * L) / STOT;
        var c = vec3f(0.0);
        if (roots) {
          let xs = (f32(ju) + 0.5) / exp2(n);
          // Scenery: the roots are there, barely.
          c = pal(gHue + 0.8 + 0.3 * xs) * (0.02 + 0.03 * (1.0 - sN)) * round;
          // BASS: light runs out along them as the bass line swells, brighter
          // down the roots of the note being played, and drifts outward.
          let reach = 0.08 + 1.1 * gBass;
          let run = sstep(reach, reach - 0.5, sN) * gBass;
          let tone = 0.55 + 0.9 * sapAt(min(n, 3.0), xs);
          let drift = 0.85 + 0.15 * sin(sN * 26.0 - uTime * 1.6 * gPace);
          let fil = exp(-lat * lat * 5.0);
          c += gColBass * run * tone * drift * (0.22 * round + 0.95 * fil) * (1.25 - 0.7 * sN);
        } else {
          // The strands a branch is made of. Each goes on to one of the
          // branches above and carries its share of the octave: as many as
          // the pixels can show, up to eight.
          let side = mix(vec2f(0.267, 0.964), vec2f(-0.971, 0.239), bits(hn, 10u));
          let u1 = u * side.x + cross(d, u) * side.y;
          let flip = select(-1.0, 1.0, u1.y * tg.x - u1.x * tg.y > 0.0);
          let sub = clamp(floor(log2(max(ww / (1.3 * px), 1.0))), 0.0, min(3.0, f32(nlev - ni)));
          let ns = exp2(sub);
          let tl = clamp(0.5 + 0.5 * lat * flip, 0.0, 0.999);
          let ci = floor(tl * ns);
          let xs = (f32(ju) + (ci + 0.5) / ns) / exp2(n);
          let sn = sin(PI * (tl * ns - ci));
          let strand = select(0.25 + 0.75 * sn * sn, 1.0, ns < 1.5);
          // Scenery: bark, lit at its edges, and the sap in it, which is the
          // spectrum. The finest twigs keep a little light of their own.
          let sap = sapAt(n, xs);
          let rim = 1.0 - round;
          c = pal(gHue + 0.55) * (0.008 + 0.035 * rim * rim);
          c += pal(gHue + xs) * (sap * sap * (0.30 + 0.07 * n) + 0.02 * n * gBloom) * strand * round;
          // KICK: a pulse of light climbs from the foot, sharp in front and
          // trailing behind.
          var kick = 0.0;
          for (var k = 0; k < 4; k++) {
            let dl = sN - gKickAt[k];
            kick += select(exp(-dl * dl / 0.0025), exp(dl / 0.24), dl < 0.0) * gKickAmp[k];
          }
          c += gColKick * kick * round * (0.4 + 0.6 * strand) * 2.6;
          // With nothing playing, a slow pulse of the tree's own (scenery).
          let di = sN - gIdleAt;
          c += pal(gHue + 0.12) * select(exp(-di * di / 0.006), exp(di / 0.3), di < 0.0) * gIdleAmp * round * strand;
          // TUNE: the limbs that carry the notes now sounding, at the height
          // of the tune.
          let wl = exp(-(n - gPitchLv) * (n - gPitchLv) / 1.6);
          if (wl * gTune > 0.02) {
            let note = sstep(0.2, 0.75, noteAt(xs));
            c += gColTune * note * wl * gTune * round * 1.5;
          }
          // SNARE: the bough that cracked, hard and thin in a soft sheath,
          // and the crack running out along everything it carries.
          let cw = 0.3 * ww + px;
          let core = exp(-dist * dist / (cw * cw));
          out.add += gColSnare * (own * (core * 5.0 + exp(-dist / (1.2 * ww + px)) * 0.9) + crack * (core * 3.2 + 0.5 * cov)) * depth;
        }
        out.body = max(out.body, c * cov * depth);
        out.cover = max(out.cover, cov);
      }
    }

    if (!roots) {
      // ---- blossom along the twigs -----------------------------------------
      if (ni >= nlev - 2) {
        let xs = (f32(ju) + 0.5) / exp2(n);
        out.add += blossom(p - e.xy, hn, xs, n, e.z);
        let h2 = hn * 2654435761u;
        let off = (vec2f(bits(h2, 12u), bits(h2, 22u)) - 0.5) * 0.014;
        let mid = 0.25 * o + 0.5 * cc + 0.25 * e;
        out.add += blossom(p - mid.xy - off, h2, xs, n, mid.z);
        if (ni >= nlev - 1) {
          let far = 0.0625 * o + 0.375 * cc + 0.5625 * e;
          out.add += blossom(p - far.xy + off.yx, hn * 40503u + 977u, xs, n, far.z);
        }
      }
      // ---- TUNE: a sugarplum hangs under each bough of the middle crown and
      // glows from inside when its note sounds --------------------------------
      if (ni >= 3 && ni <= 6) {
        let rp = 0.019 * pow(0.83, n - 3.0);
        let q = p - (e.xy + vec2f(0.0, -1.9 * rp));
        let dd = dot(q, q);
        if (dd < 6.0 * rp * rp) {
          let depth = 0.45 + 0.55 * nearness(e.z);
          let xs = (f32(ju) + 0.5) / exp2(n);
          let wl = exp(-(n - gPitchLv) * (n - gPitchLv) / 1.6);
          let note = sstep(0.2, 0.75, noteAt(xs));
          let r = sqrt(dd) / rp;
          let ball = sstep(1.0, 1.0 - 2.0 * gPx / rp, r);
          let shade = sqrt(max(1.0 - r * r, 0.0));
          // Scenery: dim glass in the colour of its note, with a rim and a
          // glint.
          let gl = q / rp - vec2f(-0.35, 0.4);
          var c = pal(gHue + xs) * (0.035 + 0.06 * (1.0 - shade) + 0.3 * exp(-dot(gl, gl) * 14.0)) * ball;
          // Lit from inside, and glowing a little way round.
          let glow = note * wl * gTune;
          c += gColTune * glow * (ball * (0.5 + 1.6 * shade) + 0.3 * exp(-r * r * 0.6)) * 2.4;
          out.add += c * depth;
          out.cover = max(out.cover, ball * 0.5);
        }
      }
    }

    // ---- its two children ---------------------------------------------------
    if (ni < nlev) {
      // The fork lies in a plane turned about the branch by a random angle
      // (by none at the trunk, so that the first fork faces the way the tree
      // is turned).
      let side = select(normalize(mix(vec2f(0.267, 0.964), vec2f(-0.971, 0.239), bits(hn, 10u))), vec2f(1.0, 0.0), ni == 0);
      let u1 = u * side.x + cross(d, u) * side.y;
      let crackOn = (crack + own) * 0.8;
      let nc = ni + 1;
      var keep = vec2<bool>(false, false);
      var kd: array<vec3f, 2>;
      for (var k = 0; k < 2; k++) {
        let hc = ih(u32(nc), ju * 2u + u32(k), 1u);
        var T = select(select(TA, TB, k == 1) + TJ * bits(hc, 0u), select(T0A, T0B, k == 1) + 0.12 * bits(hc, 0u), ni == 0);
        if (!roots) {
          // KICK: every fork opens a little. And the wind: each fork rides
          // one of eight slow oscillators, by its own amount.
          let amp = f32((hc >> 13u) & 255u) / 128.0 - 1.0;
          T = T * (1.0 + gHeave) + SWAY * (0.4 + 0.12 * n) * amp * gOsc[(hc >> 10u) & 7u];
        }
        var dk = d + u1 * ((f32(k) * 2.0 - 1.0) * T);
        dk.y += UP0 - UPS * n;
        dk = normalize(dk);
        kd[k] = dk;
        // Is the pixel inside the sphere that holds all that grows from it?
        let c = e.xy + (d.xy + dk.xy) * (0.5 * tCtr[nc]);
        let rad = tRad[nc] + tPad[nc];
        keep[k] = dot(p - c, p - c) < rad * rad;
      }
      // Go on into one child; put the other by if it is wanted too.
      if (keep[0] && keep[1]) {
        so[sp] = e;
        sq[sp] = d;
        sd[sp] = kd[1];
        su[sp] = normalize(u1 - kd[1] * dot(u1, kd[1]));
        sm[sp] = vec4f(n + 1.0, f32(ju * 2u + 1u), m.z + L, crackOn);
        sp += 1;
      }
      if (keep[0] || keep[1]) {
        let k = select(1, 0, keep[0]);
        let dk = kd[k];
        o = e;
        dp = d;
        u = normalize(u1 - dk * dot(u1, dk));
        d = dk;
        m = vec4f(n + 1.0, f32(ju * 2u + u32(k)), m.z + L, crackOn);
        have = true;
      }
    }
  }
  return out;
}

// KICK: how strongly a kick's swell is passing along the ground at x. It
// leaves the foot of the tree and slows as it goes.
fn kickOnGround(x: f32) -> f32 {
  var s = 0.0;
  for (var k = 0; k < 4; k++) {
    let r = 1.7 * (1.0 - exp(-uKickAges[k] * 1.5 * gPace));
    let d = abs(x) - r;
    s += gKickAmp[k] * exp(-d * d / 0.03) * sstep(0.0, 0.15, r);
  }
  return s;
}

// The height of the ground at x: level under the tree, rolling a little
// further out, and lifted where a kick's swell is passing.
fn ground(x: f32, swell: f32) -> f32 {
  let out = sstep(0.1, 0.5, abs(x));
  return GROUND + (0.007 * sin(x * 3.1 + 1.0) + 0.004 * sin(x * 7.3)) * out + 0.010 * swell;
}

// The deepest colour the palette has: the night, whatever the harmony.
fn deep() -> vec3f {
  var sum = vec4f(0.0);
  for (var i = 0; i < 6; i++) {
    let c = pal((f32(i) + 0.5) / 6.0);
    let w = exp(-9.0 * dot(c, vec3f(0.3, 0.5, 0.2)));
    sum += vec4f(c, 1.0) * w;
  }
  return sum.rgb / sum.a;
}

// ---- the constellations -------------------------------------------------------
//
// TUNE, in the sky. Twelve constellations stand round the tree like the hours
// on a clock, one for each note of the scale: C low on the left, the notes
// after it up that side, then down the right-hand side to B. Each is a fixed
// figure of five or six stars, and it draws itself while its note sounds: as
// the note comes in a line sets out from the first star to the next and on
// along the figure, and as the note stops the lines draw back and are gone.
// So a chord is as many constellations as it has notes, a note held from one
// chord to the next stays where it is, and a scale goes round the sky. They
// are the notes that light the plums, in the same colour, and a note starting
// plucks them brighter in the same way.
//
// The stars they are drawn through are scenery, and always there: brighter
// than the dust, one to each cell of a coarse grid.

// Bright stars to a scene unit, each way.
const STARS: f32 = 8.0;
// A constellation keeps to a patch three bright stars square. The patches
// stand to either side of the tree, two abreast and three high, from here up.
const CHART_FLOOR: f32 = -0.125;

// The sky to either side of the tree is as wide, and as high, as the window
// leaves it. The stars are charted as if each side were 0.75 wide and the
// patches 1.125 high together, and the chart is spread, or squeezed, to fit:
// this is by how much each way, and where in the picture its floor is.
const SKY_FLOOR: f32 = -0.16;
fn spread() -> vec2f {
  return vec2f(clamp((gWide - 1.1) / 0.75, 0.55, 1.25), (clamp(gHigh - 0.1, 0.9, 1.25) - SKY_FLOOR) / 1.125);
}

// From the picture to the chart, and back.
fn toChart(p: vec2f) -> vec2f {
  let by = spread();
  let ax = abs(p.x);
  return vec2f(sign(p.x) * select(ax, 1.0 + (ax - 1.0) / by.x, ax > 1.0), CHART_FLOOR + (p.y - SKY_FLOOR) / by.y);
}
fn fromChart(q: vec2f) -> vec2f {
  let by = spread();
  let ax = abs(q.x);
  return vec2f(sign(q.x) * select(ax, 1.0 + (ax - 1.0) * by.x, ax > 1.0), SKY_FLOOR + (q.y - CHART_FLOOR) * by.y);
}

// Where in the picture the bright star of a cell of the chart is.
fn brightStar(cell: vec2f) -> vec2f {
  return fromChart((cell + 0.2 + 0.6 * hash22(cell + 71.3)) / STARS);
}

// One step of a figure, an eighth of a turn at a time.
fn eighth(i: i32) -> vec2f {
  let a = f32(i) * (TAU / 8.0);
  return round(vec2f(cos(a), sin(a)));
}

// The constellation of the patch of sky that p is in, as far as its note has
// drawn it. q is p on the chart. x: its lines. y: the glow it lends the stars
// it has reached.
fn figure(p: vec2f, q: vec2f) -> vec2f {
  // Which patch: cells out from the tree's side, and cells up.
  let ax = abs(q.x);
  let out = floor(ax * STARS) - STARS;
  let high = floor((q.y - CHART_FLOOR) * STARS);
  if (ax < 1.0 || out > 5.0 || high < 0.0 || high > 8.0) {
    return vec2f(0.0);
  }
  let col = floor(out / 3.0);
  let row = floor(high / 3.0);
  let right = q.x > 0.0;
  // Clockwise from the lower left: up the left side, down the right.
  let note = select(row * 2.0 + 1.0 - col, 6.0 + (2.0 - row) * 2.0 + col, right);
  // (A firmer tune than the plums ask for: the pitch of a drum passing
  // through must not start a figure.)
  let amt = sstep(0.2, 0.55, chroma(i32(note))) * sstep(0.1, 0.35, uTune);
  if (amt <= 0.0) {
    return vec2f(0.0);
  }
  // The figure: from one star of its patch to a neighbour, holding its course
  // or turning, never to a star it has been to. The same every time.
  let hn = hash22(vec2f(note * 3.17 + 1.3, 41.0));
  var at = floor(hash22(vec2f(note, 7.7)) * 2.999);
  var dir = i32(hn.x * 7.99);
  var been = 1u << u32(at.x + 3.0 * at.y);
  let count = 5 + i32(hn.y * 1.99);
  // How many of its lines the note has drawn so far.
  let drawn = amt * f32(count - 1) * 1.1;
  let corner = vec2f(STARS + 3.0 * col, CHART_FLOOR * STARS + 3.0 * row);
  var cell = corner + at;
  var a = brightStar(vec2f(select(-cell.x - 1.0, cell.x, right), cell.y));
  let wide = max(0.8 * gPx, 0.0011);
  let glow = max(2.2 * gPx, 0.0034);
  var lines = 0.0;
  var stars = 0.0;
  for (var i = 0; i < count; i++) {
    // A star lights as the line reaches it (the first, as the note comes in).
    let da = p - a;
    stars += exp(-dot(da, da) / (glow * glow)) * sstep(-0.3, 0.3, drawn - f32(i) + 0.3);
    if (i == count - 1) {
      break;
    }
    let want = dir + i32(floor(hash21(vec2f(note * 5.3 + f32(i), 13.0)) * 2.999)) - 1;
    var found = false;
    for (var k = 0; k < 7; k++) {
      // As wanted, or failing that the nearest turn to a star not yet used.
      let d = want + ((k + 1) / 2) * select(1, -1, k % 2 == 0);
      let c = at + eighth(d);
      if (c.x >= 0.0 && c.y >= 0.0 && c.x < 3.0 && c.y < 3.0) {
        let bit = 1u << u32(c.x + 3.0 * c.y);
        if ((been & bit) == 0u) {
          at = c;
          dir = d;
          been |= bit;
          found = true;
          break;
        }
      }
    }
    if (!found) {
      break;
    }
    cell = corner + at;
    let b = brightStar(vec2f(select(-cell.x - 1.0, cell.x, right), cell.y));
    // The line stops short of the stars it joins, as on a chart.
    let ab = b - a;
    let len = length(ab);
    let gap = min(0.011, 0.3 * len);
    let reach = clamp(drawn - f32(i), 0.0, 1.0);
    let far = min(len * reach, len - gap);
    if (far > gap) {
      let t = clamp(dot(da, ab) / len, gap, far);
      let d = length(da - ab * (t / len));
      lines += sstep(wide + gPx, wide - gPx, d);
      // While it is being drawn a spark rides its end.
      let tip = da - ab * (far / len);
      stars += 0.8 * exp(-dot(tip, tip) / (glow * glow)) * step(reach, 0.999);
    }
    a = b;
  }
  return vec2f(min(lines, 1.0), stars) * sqrt(amt);
}

// Everything behind the tree (scenery).
fn sky(p: vec2f, night: vec3f) -> vec3f {
  let up = sstep(GROUND, 1.0, p.y);
  // Night: lighter low down, deepening upward.
  var c = night * mix(0.05, 0.014, pow(up, 0.55));
  // Stars, fewer and fainter towards the tree's own glow: a dust of faint
  // ones, and the brighter ones the constellations are drawn through.
  let g = p * 30.0;
  let id = floor(g);
  let h = hash22(id);
  let sd = (g - id - 0.15 - 0.7 * h) / 30.0;
  let on = step(0.8, hash21(id + 31.7));
  let tw = 0.55 + 0.45 * sin(uTime * (0.4 + 1.2 * h.x) + TAU * h.y);
  let r = max(1.1 * gPx, 0.0015);
  let hc = (p - vec2f(0.0, GROUND + 0.80)) * vec2f(0.72, 1.0);
  let halo = exp(-dot(hc, hc) / 0.42);
  let seen = sstep(0.12, 0.55, up) * (1.0 - 0.8 * halo);
  let starlight = mix(vec3f(1.0), pal(gHue + 0.5), 0.35);
  c += starlight * on * tw * exp(-dot(sd, sd) / (r * r)) * 0.4 * (0.3 + 0.7 * h.x) * seen;
  let q = toChart(p);
  let cell = floor(q * STARS);
  let bd = p - brightStar(cell);
  let br = max(1.3 * gPx, 0.0019);
  let hs = hash22(cell + 9.1);
  let btw = 0.8 + 0.2 * sin(uTime * (0.3 + 0.6 * hs.x) + TAU * hs.y);
  // (They show lower in the sky than the dust does.)
  let seenBright = sstep(0.04, 0.22, up) * (1.0 - 0.8 * halo);
  c += starlight * exp(-dot(bd, bd) / (br * br)) * 0.55 * btw * seenBright;
  // TUNE: the constellations of the notes now sounding.
  let fig = figure(p, q);
  c += gColTune * (fig.x * 0.9 + fig.y * 1.5) * (0.7 + 0.8 * noteSum()) * seenBright;
  // The light of the tree in the air behind it: more in a fuller mix.
  let air = mix(pal(gHue + 0.3), night, 0.3);
  c += air * halo * (0.04 + 0.07 * uEnergy);
  // Far hills, and nearer ones.
  let h1 = GROUND + 0.085 + 0.10 * (vnoise(vec2f(p.x * 0.9 + 4.0, 0.5)) - 0.4) + 0.03 * vnoise(vec2f(p.x * 3.1, 9.5));
  c = mix(c, night * 0.024, sstep(h1 + 2.0 * gPx, h1 - 2.0 * gPx, p.y));
  let h2 = GROUND + 0.035 + 0.07 * (vnoise(vec2f(p.x * 1.3 + 17.0, 2.5)) - 0.4) + 0.02 * vnoise(vec2f(p.x * 4.3, 3.5));
  c = mix(c, night * 0.011, sstep(h2 + 2.0 * gPx, h2 - 2.0 * gPx, p.y));
  // Mist lying on the ground, lit by the tree.
  let lift = p.y - GROUND;
  if (lift < 0.6) {
    let drift = fbm(vec2f(p.x * 1.3 - uTime * 0.012 * gPace, lift * 6.0 + uTime * 0.004));
    c += air * exp(-lift / 0.13) * (0.25 + 0.75 * drift) * (0.02 + 0.03 * uEnergy) * (0.5 + 0.9 * exp(-p.x * p.x / 0.9));
  }
  return c;
}

// Petals the crown has let go, falling: more of them the fuller the bloom
// (scenery).
fn petals(p: vec2f) -> vec3f {
  let zone = sstep(1.45, 0.6, abs(p.x)) * sstep(0.9, 0.45, p.y);
  if (zone <= 0.0) {
    return vec3f(0.0);
  }
  var c = vec3f(0.0);
  for (var l = 0; l < 2; l++) {
    let fl = f32(l);
    let sc = 6.0 + 3.5 * fl;
    let fall = (uTime * 0.035 + uPhase * 0.008) * gPace * (1.0 + 0.5 * fl);
    var q = p * sc + vec2f(fl * 13.7, fall * sc);
    q.x += 0.5 * sin(q.y * 0.6 + fl * 2.0 + uTime * 0.15 * gPace);
    let id = floor(q);
    let h = hash22(id + fl * 19.3);
    let f = q - id - 0.25 - 0.5 * h;
    let on = step(1.0 - (0.06 + 0.2 * gBloom), hash21(id + 5.1));
    // Each turns over as it falls.
    let a = uTime * (0.5 + h.x) * gPace + TAU * h.y;
    let rr = rot(a) * f;
    let el = length(vec2f(rr.x, rr.y * (2.2 + 1.2 * sin(a * 1.7)))) / (sc * 0.0085);
    c += pal(gHue + 0.1 + 0.5 * h.x) * on * sstep(1.0, 0.5, el) * (0.42 - 0.15 * fl);
  }
  return c * zone;
}

fn scene(uv: vec2f) -> vec3f {
  // The tree is as tall as the picture; in a narrow one it steps back to fit.
  // (Wide enough to keep sky for the constellations on either side.)
  let fit = min(1.0, uRes.x / uRes.y / 1.5);
  let p = aspect(uv) / fit;
  gPx = 2.0 / (uRes.y * fit);
  gWide = uRes.x / uRes.y / fit;
  gHigh = 1.0 / fit;
  gPace = mix(0.5, 1.0, uReact);
  gHue = 2.0 * uHarmony + uTime * 0.004;
  // The tree turns once in a few minutes.
  gAz = uTime * 0.045 * gPace + 0.6;
  // The wind blows harder when the music is louder.
  let st = (uTime * 0.9 + uPhase * 0.35) * gPace;
  for (var i = 0; i < 8; i++) {
    gOsc[i] = sin(st * (0.5 + 0.09 * f32(i)) + 2.4 * f32(i));
  }
  gHeave = HEAVE * kickSum();
  gColKick = voiceCol(KICK);
  gColSnare = voiceCol(SNARE);
  gColHats = voiceCol(HATS);
  // With nothing playing the plums and the roots follow the idle animation,
  // and that is scenery.
  gColBass = mix(voiceCol(BASS), pal(gHue + 0.9) * 0.5, uIdle);
  gColTune = mix(voiceCol(TUNE), pal(gHue + 0.3), uIdle);
  for (var k = 0; k < 4; k++) {
    // KICK: the pulse starts fast and slows as it climbs, and is spent in
    // about half a second: the kick's own push, with a longer release for
    // the light to travel in.
    let age = uKickAges[k];
    gKickAt[k] = 1.2 * (1.0 - exp(-age * 3.0 * gPace));
    gKickAmp[k] = hitEnv(age, uKickHits[k], 0.06, 0.34) * last4(k, uKickAges.x);
    // SNARE: an eighth of the crown, or a quarter for some of the hard hits.
    let id = uSnareN - f32(k);
    let lv = select(3.0, 2.0, uSnareHits[k] > 0.85 && hash21(vec2f(id, 7.3)) > 0.5);
    gSnareLv[k] = lv;
    gSnareJ[k] = floor(hash21(vec2f(id, 3.9)) * exp2(lv));
    gSnareAmp[k] = snareEnv(k) * last4(k, uSnareAges.x);
    gHatAmp[k] = hatEnv(k);
    gHatId[k] = u32(fmod(uHatN - f32(k), 4096.0)) * 5u;
  }
  let own = fmod(uTime, 7.0);
  gIdleAt = 1.2 * (1.0 - exp(-own * 0.7));
  gIdleAmp = uIdle * 0.5 * sstep(0.0, 0.6, own) * exp(-own * 0.6);
  // Below a whisper the tune is the pitch of a drum passing through.
  gTune = sstep(0.04, 0.3, uTune) * (0.7 + 0.8 * noteSum());
  gPitchLv = mix(2.6, 6.4, uPitch);
  // (The merest trace of a bass line is the tail of a kick.)
  gBass = uBassLine * sstep(0.0, 0.3, uBassLine);
  gBloom = mix(sstep(0.1, 0.8, uEnergy), 0.6, uIdle);

  let night = deep();
  let swell = kickOnGround(p.x);
  let gy = ground(p.x, swell);
  var col = vec3f(0.0);
  if (p.y > gy) {
    let tree = walk(p, false);
    col = (sky(p, night) + petals(p)) * (1.0 - tree.cover) + tree.body + tree.add;
  } else {
    // The soil, in faint beds, a little lighter under the tree.
    let depth = gy - p.y;
    let wob = fbm(vec2f(p.x * 2.3, depth * 3.0) + 7.0);
    let band = 0.5 + 0.5 * sin((depth + 0.06 * wob) * 64.0);
    let under = exp(-p.x * p.x / 0.5);
    let soil = night * (0.02 * exp(-depth * 3.0) * (0.5 + under) + 0.006 * band * band * exp(-depth * 1.2));
    // The roots fork straight from the foot of the trunk: the tree is set a
    // trunk's length higher, so that its first fork is at the ground.
    let tree = walk(vec2f(p.x, GROUND + LEN[0] + (GROUND - p.y) / SQUASH), true);
    col = soil * (1.0 - tree.cover) + tree.body + tree.add;
  }
  // The surface: a hairline, brightest under the tree and never quite a
  // voice's colour (scenery).
  let dy = abs(p.y - gy);
  let line = exp(-dy / (1.2 * gPx));
  let lc = pal(gHue + 0.45);
  col += mix(vec3f(dot(lc, vec3f(0.3333))), lc, 0.5) * line * (0.05 + 0.22 * exp(-p.x * p.x / 0.6));
  // KICK: the swell, running outward along it.
  col += gColKick * swell * (line * 1.2 + 0.35 * exp(-dy / 0.02));
  return col;
}
