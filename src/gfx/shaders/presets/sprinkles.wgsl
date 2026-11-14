// Sprinkles: falling through a jar of sweets under a macro lens.
//
// Eight sheets of candy drift towards the camera through a very shallow depth
// of field. The lens holds one sheet in focus as it falls towards us, and on
// it the candy is crisp and glossy. Everything behind and in front is bokeh:
// each piece spreads into its own shape convolved with the aperture, its
// highlight into a six-bladed disc, and whatever is nearest the lens washes
// across the frame as a veil. Then the lens racks back to the next sheet.
//
// There are four kinds of candy, and the bokeh, and each belongs to one voice
// of the music. While music plays a piece is drawn in its voice's colour:
// the full colour, lit, while that voice is asking something of it, and a
// dull, dim shade of it otherwise.
//
//   voice  what              does                             where              colour
//   -----  ----------------  -------------------------------  -----------------  ---------------
//   KICK   the balls, the    swell and glow with the kick's   from the middle    voiceCol(KICK)
//          largest pieces    push (kickSum), each one later   of the frame
//                            the further out it is; and the   outward
//                            whole fall surges a step
//                            towards the lens at the speed
//                            of that push
//   SNARE  the stars         a crack runs out both ways from  along a straight   voiceCol(SNARE)
//                            a point (snareEnv): a streak of  line that cuts
//                            light, the stars it passes pop,  across the frame,
//                            flash and twist, and a row of    a new one for
//                            fresh stars is thrown along it,  each hit
//                            which then melts away
//   HATS   the sugar specks  each hat fires a different two   the upper part of  voiceCol(HATS)
//                            thirds of the specks in focus    the frame, and
//                            (hatEnv): four-pointed glints    round its edge
//   BASS   the bokeh: the    swell with uBassLine: the veils  the bottom of the  voiceCol(BASS)
//          veils at the      fill with colour (without it     frame
//          lens, the lights  they are thin and grey), the
//          behind the jar,   lights grow and brighten, the
//          a pool of light   pool wells up, and the bokeh
//          under it          swirls more at the corners
//   TUNE   the rods          light in a band, the ones whose  a band across the  voiceCol(TUNE)
//                            note is sounding most (chroma);  middle, at the
//                            ride up and down and lean with   height of the
//                            the pitch; a note starting       pitch (uPitch)
//                            (noteSum) plucks them: a flash
//                            and a twist that springs back
//
// What belongs to no voice is scenery, slow and in pal() colours:
//
//   energy    where the focal plane sits: far off in a breakdown (small crisp
//             candy seen through big soft veils), close in when the mix is
//             full (a few large pieces, sharp, against the bokeh).
//   loudness  sets the speed of the fall and of the slow roll (uPhase).
//   harmony   where the scenery's colours sit in the palette (uHarmony).
//   silence   no voice is playing, so the jar is all scenery: every piece has
//             a note, one of the twelve pitch classes, which gives it its
//             pal() colour and lights it when it sounds in the idle chroma;
//             the balls breathe on their own, the sugar twinkles, the veils
//             are full, and the focus wanders between near and far.
//   uReact    the voices' envelopes already follow it. Here it also sets the
//             size of the kick's surge, how far the bass swells, how many
//             stars a snare throws, and the pace of everything that moves all
//             the time: the fall, the roll, the spin and the rack of the lens
//             slow to half.
//
// How it is drawn: each sheet is a jittered grid with one piece per cell. A
// pixel looks at the four cells nearest to it, which is enough because a
// piece, its jitter and its blur together never reach further than one cell
// from its cell centre; and it visits them in a fixed order, so overlapping
// neighbours stack the same way from every pixel and no cell edge shows.
// Sheets are composited far to near, each piece over what is behind it.

const LAYERS: f32 = 8.0;
// Cells per unit of screen height for the farthest and the nearest sheet.
const S_FAR: f32 = 8.0;
const S_NEAR: f32 = 0.5;
// Fraction of cells that hold a piece, and how far one strays from its cell
// centre. JIT + a piece's size + its blur radius must stay under 1 cell.
const FILL: f32 = 0.72;
const JIT: f32 = 0.24;
// Of the cells left empty, the fraction (of all cells) that hold a speck of
// sugar instead.
const SPARE: f32 = 0.22;
// TUNE: how far, in cells, the rods ride up and down with the pitch. It comes
// out of their share of JIT.
const RIDE: f32 = 0.07;
// Aperture: the blur radius, in cells, of something right at the lens; the
// half-thickness of the slab that counts as in focus (as a fraction of the
// focal distance); and the largest blur radius allowed.
const COC: f32 = 0.34;
const DOF: f32 = 0.19;
const RMAX: f32 = 0.38;
// Which kind a piece is: below ROD a rod, below BALL a ball, below STAR a
// star, sugar above.
const ROD: f32 = 0.42;
const BALL: f32 = 0.68;
const STAR: f32 = 0.83;
// SNARE: how fast a crack runs along its line, in aspect() units a second.
const CRACK: f32 = 15.0;

// Key light and its half vector (towards the viewer), in the jar's frame.
const LIGHT: vec3f = vec3f(-0.453, 0.655, 0.605);
const HALF: vec3f = vec3f(-0.253, 0.366, 0.896);

// State shared by every piece; set in scene().
// What uReact asks of the things the voices' envelopes do not cover: the size
// of a step (1 at full reactivity) and the speed of anything that moves all
// the time (likewise).
var<private> gHit: f32;
var<private> gPace: f32;
var<private> gT: f32;      // uTime at that pace: the clock for drift and spin
// SNARE: the line each of the last four cracks runs along: the point it
// starts from (aspect() units) and the cos and sin of its slant.
var<private> gCut: array<vec4f, 4>;
var<private> gNote: f32;   // TUNE: the pluck of a note
var<private> gTune: f32;   // TUNE: how much tune there is, 0 with none
var<private> gPitchY: f32; // TUNE: the height on screen its pitch asks for
var<private> gRide: f32;   // TUNE: how far the rods have ridden up, in cells
var<private> gLean: f32;   // TUNE: how far the rods lean, in radians
var<private> gBass: f32;   // BASS: the swell
var<private> gHue: f32;    // where the scenery's colours sit in the palette
var<private> gP0: vec2f;   // this pixel, in aspect() units
// Per sheet.
var<private> gAx: vec2f;   // cos, sin of the turn from sheet to screen
var<private> gInv: f32;    // screen units per cell

// How much of a hit's gesture is still to come: 1 at the hit, easing to 0.
// It is one minus the running integral of hitEnv(age, 1, attack, release),
// so something that steps on each hit (count minus the sum of these over the
// last four hits) moves at the speed of that voice's envelope, and nothing
// jumps when the next hit arrives.
fn toGo(age: f32, attack: f32, release: f32) -> f32 {
  let rise = attack * (1.0 + 2.5 * (1.0 - uReact));
  let x = clamp(age / rise, 0.0, 1.0);
  let done = rise * x * x * x * (1.0 - 0.5 * x) + release * (1.0 - exp(-max(age - rise, 0.0) / release));
  return 1.0 - done / (0.5 * rise + release);
}

fn toGo4(ages: vec4f, attack: f32, release: f32) -> f32 {
  return toGo(ages.x, attack, release) + toGo(ages.y, attack, release) + toGo(ages.z, attack, release) + toGo(ages.w, attack, release);
}

// KICK: kickSum() as it arrives `delay` seconds later: the kick's own
// envelope, for something the push takes that long to reach.
fn kickLate(delay: f32) -> f32 {
  let now = uKickAges;
  uKickAges = max(now - delay, vec4f(0.0));
  let e = kickSum();
  uKickAges = now;
  return e;
}

// SNARE: where crack number `n` is: the point it starts from, and a slant
// within 40 degrees of level, so that the line through the point cuts across
// the picture.
fn cutAt(n: f32) -> vec4f {
  let c = (hash22(vec2f(n, 7.3)) - 0.5) * vec2f(1.0, 1.1);
  let slant = (hash21(vec2f(7.3, n)) - 0.5) * 1.4;
  return vec4f(c, cos(slant), sin(slant));
}

// SNARE: snareEnv(k) as it arrives `along` the line from the point the crack
// starts from. The crack runs out both ways at CRACK units a second.
fn snareLate(k: i32, along: f32) -> f32 {
  let now = uSnareAges;
  uSnareAges = max(now - abs(along) / CRACK, vec4f(0.0));
  let e = snareEnv(k);
  uSnareAges = now;
  return e;
}

// Where each voice lives, for a piece centred at `c` (aspect() units).
// HATS: the upper part of the frame, and round its edge.
fn hatPlace(c: vec2f) -> f32 {
  let e = max(abs(c.x) * uRes.y / uRes.x, abs(c.y));
  return max(sstep(-0.05, 0.6, c.y), sstep(0.6, 0.9, e));
}
// BASS: the bottom.
fn bassPlace(y: f32) -> f32 {
  return sstep(0.35, -0.75, y);
}

// Five-pointed star, after Inigo Quilez.
fn sd_star5(p0: vec2f, r: f32, rf: f32) -> f32 {
  let k1 = vec2f(0.809016994375, -0.587785252292);
  let k2 = vec2f(-k1.x, k1.y);
  var p = vec2f(abs(p0.x), p0.y);
  p -= 2.0 * max(dot(k1, p), 0.0) * k1;
  p -= 2.0 * max(dot(k2, p), 0.0) * k2;
  p = vec2f(abs(p.x), p.y - r);
  let ba = rf * vec2f(-k1.y, k1.x) - vec2f(0.0, 1.0);
  let h = clamp(dot(p, ba) / dot(ba, ba), 0.0, r);
  return length(p - ba * h) * sign(p.y * ba.x - p.x * ba.y);
}

// One piece of candy. `v` is the pixel relative to its centre, `sz` its size,
// `R` its circle of confusion and `px` the pixel footprint, all in cells;
// `kind` and the two hashes say which piece it is, `ctr` is where its centre
// is on the screen (aspect() units) and `veil` how far it has gone from candy
// (0) to a veil at the lens (1). Returns premultiplied colour and alpha.
fn sprinkle(v: vec2f, kind: f32, hB: vec2f, hC: vec2f, sz: f32, R: f32, px: f32, ctr: vec2f, veil: f32) -> vec4f {
  let w = sqrt(R * R + px * px);
  let dir = select(-1.0, 1.0, hB.x > 0.5);
  let pc = floor(hC.x * 12.0);

  // Whose it is, and how much that voice is asking of it right now, 0..1.
  var voice = TUNE;
  var act = 0.0;
  var ang = 0.0;
  if (kind < ROD) {
    // TUNE: rods light in a band at the height of the pitch, the ones whose
    // note is sounding most; they lean with the pitch; and a note starting
    // plucks them: brighter, and a twist that springs back.
    let dy = (ctr.y - gPitchY) / 0.5;
    act = min(gTune * exp(-dy * dy) * (0.38 + 0.62 * chroma(i32(pc))) * (0.85 + 0.45 * gNote), 1.0);
    ang = hB.x * TAU + gT * (hC.y - 0.5) * 0.7 + gLean + dir * (TAU / 16.0) * gNote * act;
  } else if (kind < BALL) {
    // KICK: the push spreads from the middle of the frame, and is heaviest
    // there.
    voice = KICK;
    let rc = length(ctr);
    act = kickLate(0.16 * rc) * mix(1.0, 0.6, sstep(0.3, 2.0, rc));
  } else if (kind < STAR) {
    // SNARE: the stars along the line of the crack, as it passes them.
    voice = SNARE;
    for (var k = 0; k < 4; k++) {
      let rel = ctr - gCut[k].xy;
      let dl = (gCut[k].z * rel.y - gCut[k].w * rel.x) / 0.28;
      act += snareLate(k, dot(gCut[k].zw, rel)) * exp(-dl * dl);
    }
    // (A star that is large on the screen answers less, so that the crack
    // stays at the scale of a limb however near the focal plane is.)
    act = min(act, 1.0) * min(1.0, 0.11 / (0.165 * sz * gInv));
    ang = hB.x * TAU + gT * (hC.y - 0.5) * 0.5 + dir * act * (TAU / 10.0);
  } else {
    // HATS: each hat fires a different two thirds of the specks that are
    // sharp enough to be points.
    voice = HATS;
    for (var k = 0; k < 4; k++) {
      act += hatEnv(k) * step(fract(hC.y * 7.31 + (uHatN - f32(k)) * 0.618034), 0.65);
    }
    act = min(act, 1.0) * hatPlace(ctr) * sstep(0.08, 0.03, R);
  }
  // A veil has stopped being candy: it belongs to the BASS.
  act *= 1.0 - veil;

  let cs = vec2f(cos(ang), sin(ang));
  let u = vec2f(cs.x * v.x + cs.y * v.y, cs.x * v.y - cs.y * v.x);
  let Ll = vec3f(cs.x * LIGHT.x + cs.y * LIGHT.y, cs.x * LIGHT.y - cs.y * LIGHT.x, LIGHT.z);
  let Hl = vec3f(cs.x * HALF.x + cs.y * HALF.y, cs.x * HALF.y - cs.y * HALF.x, HALF.z);

  var d: f32;        // signed distance to the silhouette
  var rin: f32;      // its inradius
  var area: f32;
  var nxy: vec2f;    // surface normal
  var hc: vec2f;     // highlight: centre,
  var hh = 0.0;      //   half-length,
  var wh: f32;       //   half-width in focus,
  var gloss: f32;    //   power,
  var keep = 0.25;   //   and how much of it survives as bokeh
  var gain = 1.0;    // how much light its voice gives it
  var lim = w + 0.1 * sz; // nothing of it lies further out than this
  var body = 1.0;
  var flare = 0.0;
  var glass = 0.0;

  if (kind < ROD) {
    // Rod: a capsule lit as the cylinder it is. The tune brightens the streak
    // of light along it, and a note flashes it.
    let h = 0.18 * sz;
    let r = 0.057 * sz;
    let qv = vec2f(u.x - clamp(u.x, -h, h), u.y);
    d = length(qv) - r;
    rin = r;
    area = 4.0 * h * r + PI * r * r;
    nxy = qv / r;
    hc = vec2f(0.8 * r * Hl.x, r * Hl.y / sqrt(Hl.y * Hl.y + Hl.z * Hl.z));
    hh = 0.8 * h;
    wh = 0.13 * r;
    let al = 1.0 - Hl.x * Hl.x;
    gloss = al * al * al * al * (0.3 + act * (1.2 + 3.0 * gNote));
    gain = 1.4;
    keep = 0.12;
  } else if (kind < BALL) {
    // Ball: the largest piece. Swells on the kick.
    // (With nothing playing, each breathes slowly on its own instead.)
    let calm = uIdle * 0.12 * (0.5 + 0.5 * sin(uTime * 1.3 + hC.y * TAU));
    let r = 0.19 * sz * (1.0 + 0.4 * act + calm);
    d = length(u) - r;
    rin = r;
    area = PI * r * r;
    nxy = u / r;
    hc = 1.25 * r * Hl.xy;
    wh = 0.15 * r;
    gloss = 0.35 + 0.95 * act;
    gain = 1.2;
    glass = 1.0;
  } else if (kind < STAR) {
    // Star: pops on the snare (and twists, above, up to a tenth of a turn).
    let rs = 0.165 * sz * (1.0 + 0.3 * act);
    d = sd_star5(u, rs, 0.5) - 0.03 * sz;
    rin = 0.09 * sz;
    area = 2.4 * rs * rs;
    let bev = sstep(-0.08 * sz, 0.0, d);
    nxy = normalize(u + vec2f(1e-5, 0.0)) * bev * 0.75;
    hc = 0.35 * rs * Hl.xy;
    wh = 0.2 * rs;
    gloss = 0.0;
    gain = 1.35;
    keep = 0.0;
  } else {
    // Sugar crystal: a speck that is nearly all highlight.
    // (With nothing playing, each twinkles slowly on its own.)
    let r = 0.035 * sz;
    d = length(u) - r;
    rin = r;
    area = PI * r * r;
    nxy = u / r;
    hc = vec2f(0.0);
    wh = 0.45 * r;
    let tw = 0.5 + 0.5 * sin(uTime * (0.7 + 1.1 * hC.y) + hB.x * TAU);
    let spark = act + uIdle * 0.35 * tw * tw * tw;
    gloss = 0.15 + 10.0 * spark;
    keep = 1.0;
    body = 0.5;
    lim = 1e9;
    // In focus it throws a little four-pointed glint.
    let au = abs(u);
    // (Its arms grow with the tick, but stay fine on the screen.)
    let fa = max(13.0 / (sz * (1.0 + 0.8 * spark)), 22.0 * gInv);
    let fb = min(170.0 / sz, 0.6 / px);
    flare = (exp(-au.x * fa - au.y * fb) + exp(-au.y * fa - au.x * fb)) * (0.04 + 1.0 * spark) * sstep(0.27 * sz, 0.1 * sz, max(au.x, au.y));
  }

  if (d > lim) {
    return vec4f(0.0);
  }

  let n2 = min(dot(nxy, nxy), 1.0);
  let nz = sqrt(1.0 - n2);
  let n = vec3f(nxy, nz);

  // Shading flattens as the sprinkle leaves the focal plane.
  let sharp = clamp(1.0 - 0.8 * w / rin, 0.0, 1.0);
  let lo = 0.72 + 0.28 * clamp(dot(v, LIGHT.xy) / (rin + w), -1.0, 1.0);
  let dif = mix(lo, clamp(dot(n, Ll) * 0.65 + 0.35, 0.0, 1.0), sharp);
  let edge = mix(1.0, 0.6 + 0.4 * nz, sharp);

  // Colour. With music, a piece is its voice's colour: dull and dim at rest,
  // the full colour while the voice plays. A veil at the lens is the BASS's,
  // and swells from the bottom of the frame. With nothing playing the jar is
  // scenery: each piece has a note, which places it in the palette (a fifth
  // apart is one step apart, and one piece in eight takes the colour
  // opposite, as an accent) and lights it when it sounds.
  let live = 1.0 - uIdle;
  let swell = veil * gBass * bassPlace(ctr.y);
  let on = act + swell;
  let vcol = mix(voiceCol(voice), voiceCol(BASS), swell / (on + 1e-4));
  let rcol = mix(voiceCol(voice), voiceCol(BASS), veil);
  let dull = mix(vec3f(dot(rcol, vec3f(0.3, 0.5, 0.2))), rcol, 0.38 * (1.0 - 0.6 * veil)) * 0.6;
  let scen = pal(gHue + fract(pc * 7.0 / 12.0) * 0.32 + select(0.0, 0.5, hB.y > 0.88));
  let tint = mix(mix(dull, scen, uIdle), vcol, sstep(0.0, 0.4, on) * live);
  let glow = gain * on + uIdle * chroma(i32(pc));
  // Out of focus a piece is a veil over what is behind it. Unlit veils are
  // all about as bright as each other: darker ones only muddy the picture,
  // and a lit one right at the lens would flood it.
  let soft = sstep(rin, 3.0 * rin, w);
  let lum = max(0.42 + glow * (1.0 - 0.45 * soft), mix(0.65, 0.95, uIdle) * soft);
  var rgb = tint * (0.2 + 0.8 * dif) * edge * lum;
  // Translucency: light coming through the rim.
  rgb += tint * tint * n2 * n2 * 0.3 * sharp * (0.2 + lum);
  // Balls are boiled sweets: each one is a lens holding a small upside-down
  // picture of the jar (the previous frame), which holds balls of its own.
  // What it sees is capped, so the picture within the picture can only fade.
  if (glass * sharp > 0.0) {
    let dv = vec2f(gAx.x * v.x - gAx.y * v.y, gAx.y * v.x + gAx.x * v.y) * gInv;
    let seen = min(prev(unaspect(gP0 - dv * (1.0 + 4.0 * (1.0 + 0.8 * n2 * n2)))), vec3f(1.2));
    rgb = mix(rgb, rgb * 0.45 + seen * tint * 0.9, glass * sharp * (1.0 - 0.5 * sstep(0.0, 0.5, on)));
  }
  // A broad sheen under the hard highlight.
  let sheen = max(dot(n, Hl), 0.0);
  let sh2 = sheen * sheen;
  let sh4 = sh2 * sh2;
  rgb += mix(vec3f(1.0), tint, 0.5) * (0.22 * sharp * sh4 * sh4 * sh4 * sh2);

  // Coverage: the silhouette convolved with the aperture disc.
  let m = min(w, rin);
  let pk = area / (PI * w * w);
  let peak = pk / sqrt(1.0 + pk * pk);
  var a = sstep(w, w - 2.0 * m, d) * peak;
  a *= 1.0 + 0.15 * soft * sstep(w - 2.0 * m - 0.5 * w, w - 2.0 * m, d);
  // BASS: a veil is thin until the bass line fills it.
  a = min(a, 1.0) * body * mix(1.0, 0.35 + 0.65 * sstep(0.0, 0.6, on), veil * live);

  // Contact shadow on whatever is behind.
  let sh = 0.4 * (1.0 - soft) * sstep(0.1 * sz + w, -0.03 * sz, d) * body;

  // Specular highlight: far brighter than the display, so out of focus it
  // spreads into a bokeh disc (the aperture has six blades) with a bright rim.
  let hq = u - hc;
  let hv = vec2f(hq.x - clamp(hq.x, -hh, hh), hq.y);
  let hx = abs(vec2f(gAx.x * hv.x - gAx.y * hv.y, gAx.y * hv.x + gAx.x * hv.y));
  let softh = sstep(1.5 * wh, 4.0 * wh, w);
  let dh = mix(length(hv), max(hx.x * 0.866 + hx.y * 0.5, hx.y) * 1.07, 0.6 * softh * keep);
  let Rh = sqrt(w * w + wh * wh);
  let hs = max(px, 0.9 * wh);
  let a0 = 4.0 * hh * wh + PI * wh * wh;
  let a1 = 4.0 * hh * Rh + PI * Rh * Rh;
  var hl = min(3.5 * gloss * a0 / a1, 1.5) * sstep(Rh + hs, Rh - hs, dh);
  hl *= (1.0 + 0.9 * softh * sstep(0.55 * Rh, Rh, dh)) * mix(1.0, keep, softh);
  let scol = mix(vec3f(1.0), tint, 0.3 + 0.5 * softh);
  hl += flare * (1.0 - softh);

  return vec4f(rgb * a + scol * hl, a + sh * (1.0 - a));
}

// Lights far behind the jar, so far out of focus that each is a wide, faint
// image of the aperture. One per cell, kept inside its cell. They are the
// BASS's: `b` is how much of it reaches this part of the frame, and they grow
// and brighten with it.
fn lights(q: vec2f, seed: f32, ax: vec2f, px: f32, b: f32) -> vec3f {
  let id = floor(q);
  let g = q - id - 0.5;
  let h = hash22(id + seed);
  let h2 = fract(h.yx * vec2f(151.7, 97.3) + h * 61.1);
  let r1 = 0.16 + 0.2 * h.x;
  let r = r1 * (0.8 + 0.2 * b);
  let v = g - (h2 - 0.5) * (0.96 - 2.0 * r1);
  let hx = abs(vec2f(ax.x * v.x - ax.y * v.y, ax.y * v.x + ax.x * v.y));
  let dd = mix(length(v), max(hx.x * 0.866 + hx.y * 0.5, hx.y) * 1.07, 0.6);
  let e = max(2.0 * px, 0.1 * r);
  let disc = sstep(r, r - e, dd) * (1.0 + 0.7 * sstep(0.55 * r, r, dd));
  let on = step(h.y, 0.55);
  let tint = mix(pal(gHue + 0.1 + 0.3 * h2.x), voiceCol(BASS), sstep(0.0, 0.4, b) * (1.0 - uIdle));
  return tint * (disc * on * (0.014 + 0.07 * b) * 0.04 / (r1 * r1));
}

// One sheet of candy: a jittered grid, one sprinkle per cell. `ratio` is the
// sheet's distance over the focal distance; a cell is left empty when its
// centre lies further than `span` from `mid` along either axis; and `only`,
// unless negative, is the kind every piece is to be.
fn sheet(q: vec2f, lid: f32, ratio: f32, px: f32, dens: f32, mid: vec2f, span: vec2f, rh: vec2f, sw: f32, only: f32) -> vec4f {
  let id = floor(q);
  let g = q - id - 0.5;
  let sgn = vec2f(select(-1.0, 1.0, g.x > 0.0), select(-1.0, 1.0, g.y > 0.0));
  let par = id - 2.0 * floor(id * 0.5);
  // Screen-up, in the sheet's frame.
  let up = vec2f(gAx.y, gAx.x);
  var acc = vec4f(0.0);
  // The four cells nearest the pixel, always visited in the same order
  // (by parity) so overlapping neighbours stack the same way everywhere.
  for (var c = 0; c < 4; c++) {
    let want = vec2f(f32(c & 1), f32(c >> 1));
    let o = sgn * abs(want - par);
    // Nothing reaches further than this from its cell centre.
    let far = g - o;
    if (dot(far, far) > 0.9604) {
      continue;
    }
    let cell = id + o;
    let hA = hash22(cell + lid * 37.0);
    let off = abs(cell + 0.5 - mid);
    // Some of the cells left empty hold a speck of sugar.
    let spare = hA.x > dens;
    if (hA.x > dens + SPARE || (spare && only >= 0.0) || off.x > span.x || off.y > span.y) {
      continue;
    }
    let hB = fract(hA.yx * vec2f(151.7, 97.3) + hA * 61.1);
    let hC = fract(hA * vec2f(113.9, 173.3) + hA.yx * 43.7);
    var kind = fract(hA.x * 211.3 + hA.y * 59.9);
    if (only >= 0.0) {
      kind = only;
    }
    if (spare) {
      kind = 0.5 * (STAR + 1.0);
    }
    // Each sprinkle sits a little in front of or behind its sheet.
    let dz = 1.0 + (hA.y - 0.5) * 0.36;
    let sz = min((0.7 + 1.0 * hB.y * hB.y * hB.y) / dz, 1.9);
    let Rt = COC * max(abs(1.0 - ratio * dz) - DOF, 0.0);
    let R = min(Rt, min(RMAX, 0.98 - JIT - 0.28 * sz - px));
    // TUNE: the rods ride up the screen as the pitch rises.
    var v0 = far - (hC - 0.5) * (2.0 * JIT);
    if (kind < ROD) {
      v0 = far - (hC - 0.5) * (2.0 * (JIT - RIDE)) - up * gRide;
    }
    let reach = 0.28 * sz + max(R + px, 0.1 * sz);
    if (dot(v0, v0) > reach * reach) {
      continue;
    }
    // Where its centre is on the screen.
    let ctr = gP0 - vec2f(gAx.x * v0.x - gAx.y * v0.y, gAx.y * v0.x + gAx.x * v0.y) * gInv;
    // In front of the focal plane and well out of focus it is a veil.
    let veil = sstep(0.04, 0.15, R) * step(ratio * dz, 1.0);
    // Swirl: away from the optical axis, what is out of focus is squeezed
    // along the radius, as an old portrait lens does to its bokeh.
    let v = v0 + rh * (dot(v0, rh) * sw * R / (R + 0.1 * sz));
    // The blur radius is capped so a sprinkle never leaves its
    // neighbourhood; beyond the cap it thins out instead, as the light of a
    // still larger disc would.
    let there = select(sstep(dens, dens - 0.06, hA.x), sstep(dens, dens + 0.05, hA.x) * sstep(dens + SPARE, dens + SPARE - 0.05, hA.x), spare);
    let thin = (R + 1e-3) / (Rt + 1e-3) * there;
    let s = sprinkle(v, kind, hB, hC, sz, R, px, ctr, veil) * thin;
    acc = acc * (1.0 - s.a) + s;
  }
  return acc;
}

fn scene(uv: vec2f) -> vec3f {
  let p0 = aspect(uv);
  let pxs = 2.0 / uRes.y;
  gHit = mix(0.4, 1.0, uReact);
  gPace = mix(0.5, 1.0, uReact);
  gT = uTime * gPace;
  let roll = 0.03 * gT + 0.012 * gPace * uPhase + 0.15 * sin(gT * 0.17);
  let p = rot(roll) * p0;
  gP0 = p0;
  let live = 1.0 - uIdle;

  // SNARE: where the last four cracks run.
  for (var k = 0; k < 4; k++) {
    gCut[k] = cutAt(fmod(uSnareN - f32(k), 64.0));
  }
  // TUNE: none of it shows until there is some tune.
  gNote = noteSum();
  gTune = sstep(0.05, 0.45, uTune);
  gPitchY = clamp((uPitch - 0.55) * 2.4, -0.75, 0.75);
  gRide = clamp((uPitch - 0.55) * 0.25, -RIDE, RIDE) * gTune;
  gLean = (uPitch - 0.55) * 1.6 * gTune;
  // BASS: the swell, smaller at lower reactivity. (A kick drum on its own
  // leaves a little in uBassLine, which the first edge keeps out.)
  gBass = sstep(0.08, 0.8, uBassLine) * mix(0.5, 1.0, uReact);
  gHue = uHarmony * 2.0 + 0.01 * uTime;

  // Backdrop: the dark of the jar, a pool of light where the key is, and
  // lights far behind it all.
  let lp = p - vec2f(-0.9, 0.75);
  let pool = exp(-dot(lp, lp) * 0.35);
  let murk = vnoise(p * 1.3 + vec2f(gT * 0.03, 0.0)) * 0.6 + vnoise(p * 2.9 - vec2f(0.0, gT * 0.05)) * 0.4;
  let bgc = mix(pal(gHue + 0.62), pal(gHue + 0.9), murk);
  var col = mix(vec3f(dot(bgc, vec3f(0.3, 0.5, 0.2))), bgc, 0.55) * (0.004 + 0.035 * pool) * (0.5 + murk);
  // BASS: its own pool of light wells up from under the jar.
  let below = exp(-(p0.y + 1.2) * (p0.y + 1.2) * 1.1 - p0.x * p0.x * 0.12);
  col += mix(bgc, voiceCol(BASS), live) * (0.06 * gBass * below * (0.5 + murk));

  let lit = 0.7 + 0.8 * pool;
  let reach = gBass * bassPlace(p0.y);
  for (var b = 0.0; b < 2.0; b += 1.0) {
    let turn = 1.0 + 2.2 * b;
    let sc = 1.15 + 0.6 * b;
    let q = rot(turn) * (p * sc + vec2f(0.02, 0.013) * gT * (1.0 + b)) + b * 31.7;
    col += lights(q, 5.0 + 9.0 * b, vec2f(cos(turn + roll), sin(turn + roll)), pxs * sc, reach) * (0.6 + pool);
  }

  // The fall: a steady drift, faster when loud, and KICK: a surge on each
  // kick, at the speed of the kick's push (an eighth of a sheet at full
  // reactivity; 64 sheets is where they repeat).
  let lurch = gHit / 8.0;
  let zc = 0.05 * gT + 0.08 * gPace * uPhase + fmod(uKickN * lurch, 64.0) - toGo4(uKickAges, 0.06, 0.20) * lurch;
  let zi = floor(zc);
  let zf = zc - zi;
  let cam = 0.7 * vec2f(sin(gT * 0.11), cos(gT * 0.13));

  // Where the music wants the focal plane: close when the mix is full, far
  // when it is thin. The lens follows the sheet nearest to that as it falls
  // towards us, then racks back to the next one behind it (over more of the
  // sheet's travel, so more slowly, at lower reactivity).
  let want = mix(0.34, 0.62, sstep(0.3, 0.85, uEnergy)) + 0.1 * uIdle * sin(uTime * 0.23);
  let fx = want * LAYERS - zf + 0.5;
  let fj = floor(fx);
  let focus = (fj - 1.0 + sstep(0.0, mix(0.4, 0.22, uReact), fx - fj) + zf) / LAYERS;
  // BASS: swirl grows towards the corners, and with the bass line.
  let r2 = dot(p, p);
  let rr = sqrt(r2 + 1e-6);
  let ph = p / rr;
  let sw = min((0.18 + 0.25 * gBass) * r2, 1.0);
  let sfocus = S_FAR * pow(S_NEAR / S_FAR, focus);

  let cr = cos(roll);
  let sr = sin(roll);
  var scale = S_FAR * pow(S_NEAR / S_FAR, zf / LAYERS);
  let closer = pow(S_NEAR / S_FAR, 1.0 / LAYERS);
  for (var j = 0.0; j < LAYERS; j += 1.0) {
    let depth = (j + zf) / LAYERS;
    let lid = fmod(j - zi, 64.0);
    let turn = lid * 2.4;
    let ct = cos(turn);
    let st = sin(turn);
    gAx = vec2f(ct * cr - st * sr, st * cr + ct * sr);
    gInv = 1.0 / scale;

    let rt = mat2x2f(ct, -st, st, ct);
    let q = rt * (p * scale + cam) + vec2f(lid * 13.7, lid * 7.3);
    // Sheets fill up as they come closer.
    let dens = mix(0.3, FILL, sstep(0.0, 0.45, depth)) * mix(1.0, 0.4, sstep(0.66, 1.0, depth));
    let s = sheet(q, lid, scale / sfocus, 0.75 * pxs * scale, dens, vec2f(0.0), vec2f(1e6), rt * ph, sw, -1.0);
    let fade = sstep(0.0, 0.1, depth) * sstep(1.0, 0.82, depth);
    let fog = mix(0.3, 1.0, sstep(0.0, 0.5, depth));
    col = col * (1.0 - s.a * fade) + s.rgb * (fade * fog * lit);
    scale *= closer;
  }

  // SNARE: a crack runs straight across the picture, out both ways from a
  // point: a streak of light, and a row of fresh stars thrown along it. The
  // row is one more sheet, a single row of it, that shoots out along the line
  // while the crack lasts, then melts out of focus. The snare's number seeds
  // where and at what slant; its age drives the throw. A softer hit, or lower
  // reactivity, throws fewer stars, less far, and they melt less.
  let ph0 = p0 / rr;
  for (var k = 0; k < 4; k++) {
    let a = uSnareAges[k];
    if (a > 1.3) {
      continue;
    }
    let n = fmod(uSnareN - f32(k), 64.0);
    let ax = gCut[k].zw;
    let up = vec2f(-ax.y, ax.x);
    let sb = 3.0 * (1.0 + 1.5 * gHit * exp(-24.0 * gPace * a)) * exp(-0.2 * gPace * a);
    let pb = (p0 - gCut[k].xy) * sb;
    let qb = vec2f(dot(pb, ax), dot(pb, up));
    if (abs(qb.y) > 1.0 || abs(qb.x) > 4.6) {
      continue;
    }
    // The crack itself.
    let along = qb.x / sb;
    col += voiceCol(SNARE) * (0.4 * snareLate(k, along) * exp(-qb.y * qb.y * 26.0) * sstep(4.4, 1.5, abs(qb.x)) * lit);
    gAx = ax;
    gInv = 1.0 / sb;
    let ratio = 1.0 + 0.8 * exp(-40.0 * a) + 1.4 * gHit * sstep(0.6, 1.3, a);
    let dens = mix(0.6, 1.0, uReact) * mix(0.5, 1.0, uSnareHits[k]);
    // (The grid is set half a cell down, so that one row of cells straddles
    // the line.)
    let s = sheet(qb + vec2f(0.0, 0.5), 100.0 + n, ratio, 0.75 * pxs * sb, dens, vec2f(0.0, 0.5), vec2f(3.6, 0.25), vec2f(dot(ph0, ax), dot(ph0, up)), sw, 0.5 * (BALL + STAR));
    let fade = sstep(0.0, 0.03, a - abs(along) / CRACK) * sstep(1.3, 0.9, a);
    col = col * (1.0 - s.a * fade) + s.rgb * (fade * lit);
  }
  return col;
}
