// Gumball: glass sweets packed for ever, every one big enough to see into
// holding the whole packing again.
//
// The picture is an Apollonian gasket: wherever three balls touch, another
// sits in the gap between them, and so on down. This one is built on a
// spiral chain of balls, ball n at MU^n with radius R0 * RHO^n, in which
// every four neighbours touch one another. The whole packing is therefore
// unchanged by z -> MU * z (a turn of THETA and a growth of RHO), which makes
// the fall into it exact: the view is only ever a fraction of one turn deep,
// and one turn later it is the same picture. What is inside a ball is the
// world outside it turned inside out through its surface (a circle
// inversion), which is a packing of circles again.
//
// The five voices (see preset_header.wgsl and fractal.wgsl). Depth is pitch,
// and here depth is the size of a ball on screen, the biggest at depth 0:
//
//   voice  what it does                         where                colour
//   -----  -----------------------------------  -------------------  ------
//   KICK   a ring of light swells from the      the biggest balls,   KICK
//          heart of each big ball to its rim    and the far end of
//          (the kick's age is its radius), a    the fall at the
//          light fires at the far end, and the  centre
//          fall is pushed on a little
//   SNARE  the balls clack (snareEnv): for an   where middle-sized   SNARE
//          instant a spark shows at every       balls touch, in a
//          point where two of them touch        swath across the
//                                               picture, a new one
//                                               each hit
//   HATS   a tick (hatEnv): a scatter of the    the smallest balls,  HATS
//          smallest balls glints, a new         towards the edge
//          scatter each tick
//   BASS   the biggest low balls fill from the  the biggest balls    BASS
//          bottom like syrup, as deep as        in the lower part
//          uBassLine
//   TUNE   the balls of one size sing, smaller  a band of sizes;     TUNE
//          ones as the pitch rises (tuneBand),  towards the middle
//          and a new note plucks them brighter
//
// Scenery, in pal() colours and dimmer than anything a voice lights:
//
// - The glass itself: a fine bright edge, a glow just inside it, light
//   gathered on the side away from the lamp and one small hot spot. A ball's
//   colour is its size in the world, which the fall does not change, so the
//   colours fall with the balls.
// - Fullness (uEnergy): how small a ball can be and still be seen into. A
//   breakdown shows fewer worlds inside worlds; a full mix opens them all.
// - Harmony (uHarmony): turns the palette, so a chord change recolours the
//   whole packing by a glide.
// - Idle: no events and the voices' colours are put away; the fall goes on.
//
// Reactivity (uReact): the envelopes carry it. Besides them the fall and the
// kick's ring are slower below 1, the kick's push is smaller, and the syrup
// is shallower.
//
// A ball smaller than a pixel is not looked for, and the eye goes at most
// LEVELS balls deep, so the cost is bounded however deep the packing is.

const RHO: f32 = 2.8900536;
const THETA: f32 = 2.2370358;
const LOGRHO: f32 = 1.0612751;
const R0: f32 = 0.9241764;
// Reflections tried before a point is given up as lying between the balls.
const STEPS: i32 = 40;
// How many balls deep the eye goes, and how big (in half screen heights) a
// ball has to be before it is seen into.
const LEVELS: i32 = 6;
const OPEN: f32 = 0.03;

// MU^t for real t.
fn mupow(t: f32) -> vec2f {
  return exp(t * LOGRHO) * vec2f(cos(t * THETA), sin(t * THETA));
}

// A circle as (curvature, curvature * centre): reflecting one circle of four
// that all touch through the other three is linear in these.
fn chain(n: f32) -> vec3f {
  let k = 1.0 / (R0 * exp(n * LOGRHO));
  return vec3f(k, k * mupow(n));
}

struct Ball {
  z: vec2f,
  r: f32,
  steps: f32,
  // Where it touches the three balls it sits between.
  ta: vec2f,
  tb: vec2f,
  tc: vec2f,
}

// Where two balls that touch do so.
fn touch(a: vec3f, b: vec3f) -> vec2f {
  return (a.yz + b.yz) / (a.x + b.x);
}

struct Circle {
  z: vec2f,
  r: f32,
}

// The ball of the packing that holds p, for 1 <= |p| < RHO. Balls smaller
// than rmin are not looked for (r comes back 0).
fn locate(p: vec2f, rmin: f32) -> Ball {
  var q = array<vec3f, 4>(chain(-2.0), chain(-1.0), chain(0.0), chain(1.0));
  for (var i = 0; i < 4; i++) {
    if (length(q[i].x * p - q[i].yz) < 1.0) {
      return Ball(
        q[i].yz / q[i].x,
        1.0 / q[i].x,
        0.0,
        touch(q[i], q[(i + 1) % 4]),
        touch(q[i], q[(i + 2) % 4]),
        touch(q[i], q[(i + 3) % 4]),
      );
    }
  }
  for (var s = 0; s < STEPS; s++) {
    // Of four balls that all touch, the smallest sits in the hollow between
    // the other three.
    var m = 0;
    if (q[1].x > q[m].x) { m = 1; }
    if (q[2].x > q[m].x) { m = 2; }
    if (q[3].x > q[m].x) { m = 3; }
    let ia = (m + 1) % 4;
    let ib = (m + 2) % 4;
    let ic = (m + 3) % 4;
    let zm = q[m].yz / q[m].x;
    let za = q[ia].yz / q[ia].x;
    let zb = q[ib].yz / q[ib].x;
    let zc = q[ic].yz / q[ic].x;
    let v = p - zm;
    let ua = za - zm;
    let ub = zb - zm;
    let uc = zc - zm;
    let o = sign(cross2(ua, ub) + cross2(ub, uc) + cross2(uc, ua));
    // Which pair of the outer three p lies between, seen from the middle one.
    var z1 = zc;
    var z2 = za;
    var third = ib;
    if (o * cross2(ua, v) >= 0.0 && o * cross2(v, ub) >= 0.0) {
      z1 = za;
      z2 = zb;
      third = ic;
    } else if (o * cross2(ub, v) >= 0.0 && o * cross2(v, uc) >= 0.0) {
      z1 = zb;
      z2 = zc;
      third = ia;
    }
    // Inside the triangle of centres, p is in the gap between the middle ball
    // and that pair, and the third ball is reflected into it. Outside, p is
    // beyond all four, and the middle ball is reflected out there.
    let edge = z2 - z1;
    var rep = m;
    if (cross2(edge, p - z1) * cross2(edge, zm - z1) > 0.0) {
      rep = third;
    }
    let nc = 2.0 * (q[0] + q[1] + q[2] + q[3] - q[rep]) - q[rep];
    q[rep] = nc;
    if (length(nc.x * p - nc.yz) < 1.0) {
      return Ball(
        nc.yz / nc.x,
        1.0 / nc.x,
        f32(s) + 1.0,
        touch(nc, q[(rep + 1) % 4]),
        touch(nc, q[(rep + 2) % 4]),
        touch(nc, q[(rep + 3) % 4]),
      );
    }
    if (1.0 / nc.x < rmin) {
      return Ball(nc.yz / nc.x, 0.0, f32(s) + 1.0, vec2f(0.0), vec2f(0.0), vec2f(0.0));
    }
  }
  return Ball(vec2f(0.0), 0.0, f32(STEPS), vec2f(0.0), vec2f(0.0), vec2f(0.0));
}

// The same for any p: the packing looks the same at every turn of the spiral.
fn find(pw: vec2f, rmin: f32) -> Ball {
  let n0 = floor(log(max(length(pw), 1e-30)) / LOGRHO);
  let s = exp(n0 * LOGRHO);
  let b = locate(cmul(pw, mupow(-n0)), rmin / s);
  let m = mupow(n0);
  return Ball(cmul(b.z, m), b.r * s, b.steps, cmul(b.ta, m), cmul(b.tb, m), cmul(b.tc, m));
}

// The point p turned inside out through circle c.
fn invPoint(p: vec2f, c: Circle) -> vec2f {
  let d = p - c.z;
  return c.z + d * (c.r * c.r / max(dot(d, d), 1e-20));
}

// Circle a turned inside out through circle c.
fn invCircle(a: Circle, c: Circle) -> Circle {
  let d = a.z - c.z;
  let k = c.r * c.r / (dot(d, d) - a.r * a.r);
  return Circle(c.z + d * k, abs(k) * a.r);
}

// One ball, lit. `c` is the ball on screen, `wr` its radius in the world
// (which the fall does not change), `inner` what is seen through it, `open`
// how far it is clear glass (1) rather than solid candy (0), `seen` how many
// balls it is seen through, and `ta`, `tb`, `tc` where on screen it touches
// the three balls it sits between.
fn lit(
  ps: vec2f,
  px: f32,
  c: Circle,
  wr: f32,
  id: vec2f,
  inner: vec3f,
  open: f32,
  seen: f32,
  ta: vec2f,
  tb: vec2f,
  tc: vec2f,
) -> vec3f {
  let live = 1.0 - uIdle;
  let rs = c.r;
  let zs = c.z;
  let d = (ps - zs) / rs;
  let dd = min(dot(d, d), 1.0);
  let rr = sqrt(dd);
  let h = sqrt(1.0 - dd);
  let n = vec3f(d, h);
  // Each ball seen through is a little darker than the one round it.
  let gain = pow(0.82, seen);

  // Clear glass: a fine bright edge, a glow just inside it, light gathered
  // on the side away from the lamp, and one small hot spot however big the
  // ball is.
  let L = normalize(vec3f(-0.45, 0.62, 0.64));
  let wrap = 0.5 + 0.5 * dot(n, L);
  let H = normalize(L + vec3f(0.0, 0.0, 1.0));
  let sw = clamp(0.03 * rs, 1.2 * px, 3.5 * px);
  let sd = (ps - zs - rs * H.xy) / sw;
  let spec = exp(-dot(sd, sd));
  let back = max(dot(d, -normalize(L.xy)), 0.0);
  let caustic = back * back * sstep(0.45, 0.92, dd) * sstep(1.0, 0.95, dd);
  let rim = pow(1.0 - h, 5.0);
  let edge = sstep(1.0, 0.0, (1.0 - rr) * rs / (1.5 * px + 0.012 * rs));
  let detail = sstep(4.0 * px, 20.0 * px, rs);

  let hv = hash22(id);
  let base = pal(log(wr) / LOGRHO + 0.25 * (hv.x - 0.5) + uHarmony);
  let shell = base * (0.12 * rim + 0.42 * edge + 0.16 * caustic) * gain;
  let solid = base * (0.02 + 0.24 * wrap * wrap + 0.4 * caustic + 0.2 * rim) * gain;
  let glass = inner * mix(vec3f(1.0), base, 0.2) * 0.92 + shell;
  var ball = mix(solid, glass, open);

  // What a voice lights: the whole body glows a little, the far side and the
  // edge most.
  let glow = 0.10 + 0.35 * wrap * wrap + 1.1 * caustic + 0.8 * rim + 0.6 * edge;

  // Depth is pitch: 0 for a ball a third of the screen across, 1 for one a
  // few pixels across.
  let depth = log(0.336 / rs) * 0.2934;

  // BASS: the biggest low balls fill from the bottom, like syrup.
  let big = sstep(0.09, 0.26, rs);
  let huge = sstep(0.16, 0.4, rs);
  let low = sstep(0.6, -0.4, zs.y);
  let lvl = -1.05 + 1.0 * bassOn() * fracHeft();
  let slosh = 0.025 * sin(d.x * 6.0 + uTime * 1.3 + hv.y * 6.0);
  let w = huge * low * gain;
  let fill = sstep(lvl + 0.04 + slosh, lvl - 0.04 + slosh, d.y) * w * sstep(0.0, 0.1, bassOn());
  let meniscus = exp(-pow((d.y - lvl - slosh) / 0.025, 2.0)) * w * sstep(0.05, 0.3, bassOn());
  ball = mix(ball, ball * 0.6 + voiceCol(BASS) * (0.03 + 0.4 * glow), 0.7 * fill);
  ball += voiceCol(BASS) * meniscus * 0.3;

  // KICK: a ring of light swells from the heart of each big ball to its rim.
  var ring = 0.0;
  for (var k = 0; k < 4; k++) {
    let a = uKickAges[k];
    let x = (rr - 2.0 * fracPace() * a) / 0.22;
    ring += exp(-x * x) * exp(-a * 2.2) * uKickHits[k];
  }
  ring = min(ring * fracAmp(), 1.0) * big * gain;
  ball += voiceCol(KICK) * ring * (0.15 + 0.8 * glow);
  // The far end of the ball's own spiral.
  ball += (pal(uHarmony + 0.1) * 0.12 + voiceCol(KICK) * kickSum() * 0.6) * exp(-rr / 0.04) * open * gain;

  // TUNE: the balls of one size sing, smaller ones as the pitch rises.
  let sing = tuneBand(depth, 0.07) * sstep(1.6, 0.8, length(zs)) * gain;
  ball += voiceCol(TUNE) * sing * 1.3 * glow;

  // SNARE: the balls clack. For an instant there is a spark at each point
  // where a middle-sized ball touches the three it sits between.
  let mid = sstep(0.012, 0.03, rs) * sstep(0.3, 0.12, rs);
  if (mid > 0.0 && snareSum() > 0.002) {
    let reach = 0.11 * rs + 1.2 * px;
    let da = (ps - ta) / reach;
    let db = (ps - tb) / reach;
    let dc = (ps - tc) / reach;
    let spark = snareSwath(ta, 0.28) * exp(-dot(da, da)) + snareSwath(tb, 0.28) * exp(-dot(db, db))
      + snareSwath(tc, 0.28) * exp(-dot(dc, dc));
    ball += voiceCol(SNARE) * spark * mid * gain * 7.0;
  }

  // HATS: a scatter of the smallest balls glints.
  let small = sstep(1.2 * px, 3.0 * px, rs) * sstep(0.035, 0.014, rs) * sstep(0.35, 0.8, length(zs));
  ball += voiceCol(HATS) * hatPick(id, 0.12) * small * (1.0 + 2.2 * glow);

  // The hot spot stays white, and fades on balls too small to show one.
  ball += vec3f(1.0) * spec * 1.6 * detail * gain;
  return ball;
}

fn scene(uv: vec2f) -> vec3f {
  let ps = aspect(uv);
  let px = 2.0 / uRes.y;

  // The fall: one unit of t is one turn of the spiral, 2.89 times deeper.
  // The kick is a push into it.
  let t = (uTime * 0.013 + uPhase * 0.008) * fracPace() + 0.012 * fracAmp() * divePush();
  let ft = fract(t);
  let s0 = exp(ft * LOGRHO);
  let toS = mupow(ft);

  // Fullness: how small a ball can be and still be seen into.
  let full = mix(sstep(0.35, 0.9, uEnergy), 0.6, uIdle);
  let open = OPEN * mix(2.4, 1.0, full);

  var world: array<Circle, 6>;
  var scr: array<Circle, 6>;
  var wrad: array<f32, 6>;
  var ids: array<vec2f, 6>;
  var opn: array<f32, 6>;
  // Where each ball touches its three neighbours, on screen.
  var con: array<vec2f, 18>;
  let clack = snareSum() > 0.002;
  var n = 0;
  var steps = 0.0;
  var pw = cmul(ps, mupow(-ft));
  // World units to a screen unit, where we are looking now.
  var J = 1.0 / s0;
  var name = vec2f(0.0);
  for (var lv = 0; lv < LEVELS; lv++) {
    let b = find(pw, 0.7 * px * J);
    steps = b.steps;
    if (b.r <= 0.0) { break; }
    // Carry the ball back out through every ball it was seen through.
    var c = Circle(b.z, b.r);
    for (var i = lv - 1; i >= 0; i--) {
      c = invCircle(c, world[i]);
    }
    // Only the snare wants these, and only while it sounds.
    if (clack) {
      var ta = b.ta;
      var tb = b.tb;
      var tc = b.tc;
      for (var i = lv - 1; i >= 0; i--) {
        ta = invPoint(ta, world[i]);
        tb = invPoint(tb, world[i]);
        tc = invPoint(tc, world[i]);
      }
      con[3 * lv] = cmul(ta, toS);
      con[3 * lv + 1] = cmul(tb, toS);
      con[3 * lv + 2] = cmul(tc, toS);
    }
    world[lv] = Circle(b.z, b.r);
    wrad[lv] = c.r;
    scr[lv] = Circle(cmul(c.z, toS), c.r * s0);
    // A stable name for it, whichever turn of the spiral it was found on.
    let n1 = floor(log(b.r) / LOGRHO + 0.5);
    let zz = cmul(b.z, mupow(-n1)) / (b.r * exp(-n1 * LOGRHO));
    name = floor(zz * 12.0 + 0.5) + 31.7 * hash22(name + f32(lv));
    ids[lv] = name;
    n = lv + 1;
    let o = sstep(open, 2.0 * open, scr[lv].r);
    opn[lv] = o;
    if (o <= 0.0 || lv == LEVELS - 1) {
      break;
    }
    // Look inside: the world outside it, through its surface.
    let d = pw - b.z;
    let f = b.r * b.r / max(dot(d, d), 1e-10 * b.r * b.r);
    J *= f;
    pw = b.z + d * f;
  }

  // Between the balls: nearly black, a little haze where they crowd.
  let haze = pal(uHarmony + 0.55) * 0.010 * steps / f32(STEPS);
  var col = haze;
  for (var i = n - 1; i >= 0; i--) {
    let c = scr[i];
    let cover = clamp((c.r - length(ps - c.z)) / px + 0.5, 0.0, 1.0);
    let ball = lit(ps, px, c, wrad[i], ids[i], col, opn[i], f32(i), con[3 * i], con[3 * i + 1], con[3 * i + 2]);
    col = mix(haze, ball, cover);
  }

  // The light at the far end, where the balls shrink out of sight; the kick
  // fires it.
  col += pal(uHarmony + 0.1) * 0.5 * exp(-length(ps) / 0.05) + voiceCol(KICK) * kickSum() * 1.2 * exp(-length(ps) / 0.09);
  return col;
}
