// Wormhole: a tunnel tiled with the spectrogram. Each ring of tiles is one
// moment of music, born at the far end and flying past the camera.
//
// The tunnel is a real tube: it snakes through space and its tiles are slabs
// of glass that stand up off the wall. Time is depth. What you hear now is
// the far end; what you heard four seconds ago is the ring rushing past your
// ears.
//
// The five voices (see preset_header.wgsl). Each has its own colour, its own
// kind of motion and its own place, and leaves no trace when it is not
// playing:
//
//   voice  what it does                                 where                     colour
//   -----  -------------------------------------------  ------------------------  ---------------
//   KICK   a ridge squeezes the whole tube and runs     a ring round the whole    voiceCol(KICK)
//          away down it: a line of light on its         tube, from just inside
//          crest, and the tiles and lattice it          the frame towards the
//          passes lit. kickEnv: it swells in heavily    middle: the biggest
//          and lets go slowly as it recedes.            thing in the picture
//   SNARE  a crack: one seam of the tube splits along   one straight line right   voiceCol(SNARE)
//          its length. The tiles that meet at it        across the picture,
//          part, those beside it fall open like         through the middle, at a
//          louvres, and light bursts through.           new angle each time
//          snareEnv: there at once, gone at once.
//   HATS   a handful of pin-point sparks on the wall,   round the rim of the      voiceCol(HATS)
//          each a tick (hatEnv) darting a hair's        picture, mostly over the
//          breadth down the tube.                       top
//   BASS   the nearest rings draw in, and the seams     the nearest rings: the    voiceCol(BASS)
//          between their tiles glow, reaching           frame of the picture
//          further in the more bass there is. A
//          swell: it follows uBassLine.
//   TUNE   the flower at the far end. Twelve petals,    the far end: the middle   voiceCol(TUNE)
//          one per note, each as long as its note is    of the picture
//          loud (chroma). It is as bright as there
//          is tune (uTune), each new note plucks its
//          light (noteEnv), and the whole flower
//          grows as the tune climbs (uPitch).
//
// The scenery, there whoever is playing, in pal() colours and never flashing:
//
// - The tiles are the spectrogram. Around the tube they run bass -> treble ->
//   bass, twice. A loud tile glows and LIFTS off the wall; where a sound
//   starts (louder than the ring before it) the tile is hotter. Up close each
//   tile shows where in its band the sound sits.
// - Harmony: every ring is coloured by the chord that was sounding when it
//   was born, read from its own row of the spectrogram (circle of fifths,
//   steadied by uHarmony). A chord change is a band of new colour that
//   travels from the far end to the camera.
// - Brightness (uCentroid): the twist. A bass-heavy mix runs straight; a
//   bright one, or a riser, winds the tiles into a helix.
// - Fullness (uEnergy): a full mix flattens the tube into an oval and turns
//   up the small sun at the far end, whose light rims the far edge of every
//   tile. A thin mix gets a gentler brightness curve and more light in the
//   haze, so a breakdown is not a black screen.
// - Silence (uIdle): the ambient spectrum tiles the walls and there are no
//   drums at all. The seams of the nearest rings and the flower still
//   breathe with the drifting uBassLine, uTune and uPitch, but in scenery
//   colours.
// - Reactivity (uReact): turned down, the flight slows, the twist relaxes,
//   the kick's ridge runs slower, the bass moves the tube less and the light
//   of the bass and the tune is fainter. The shared envelopes do the rest.
//
// How it is drawn: a few Newton steps find where the ray meets the bent,
// squeezed tube; then the rings the ray crossed on the way down are walked
// front to back, each tile a box with analytic coverage, so the relief has
// true parallax and occlusion and every edge is anti-aliased. Geometry comes
// only from float uniforms; the 8-bit textures set brightness, colour and
// (per tile, a constant) height.

const K_SYM: f32 = 2.0;          // the spectrum repeats this many times around
const N_AROUND: f32 = 14.0;      // tiles per half period: low -> high -> low
const SEAMS: f32 = 28.0;         // 2 * N_AROUND: seams between tiles in one repeat
const RING_LEN: f32 = 0.25;      // length of one ring of tiles, in tube radii
const ROWS_PER_RING: f32 = 4.0;  // history rows (1/60 s) per ring; divides 256
const Z_FAR: f32 = 15.0;         // where "now" is born, in tube radii
const SPEED: f32 = 3.75;         // 60 / ROWS_PER_RING * RING_LEN, radii per second
const X_LO: f32 = 0.02;          // the span of the spectrum the tiles cover
const X_HI: f32 = 0.93;
const GAP_S: f32 = 0.05;         // half grout width, in rings
const GAP_T: f32 = 0.10;         // half grout width, in cells
const H_MAX: f32 = 0.22;         // tallest lift, in tube radii
const STEPS: i32 = 7;            // rings walked per pixel
const FOCAL: f32 = 1.3;
const BEVEL: f32 = 0.02;         // bevel width, in tube radii

// KICK: the ridge.
const KICK_Z0: f32 = 1.2;        // where it sets off, in tube radii from the lens
const KICK_SPEED: f32 = 6.0;     // how fast it runs down the tube
const KICK_W: f32 = 1.6;         // half length of the ridge
const KICK_A: f32 = 0.25;        // how far it squeezes the tube
// SNARE: the crack.
const CRACK_BAND: f32 = 1.6;     // half width of the band of louvres, in tiles
const CRACK_PART: f32 = 0.3;     // how far the tiles at the seam draw back, in tiles
const CRACK_END: f32 = 10.0;     // it stops short of the far end, which is the tune's
// HATS: the sparks.
const HAT_SPARKS: i32 = 7;       // sparks per hit
const HAT_SPEED: f32 = 3.0;      // how fast they dart down the wall
// BASS: the breathing of the nearest rings.
const BASS_A: f32 = 0.2;         // how far they draw in
const BASS_Z: f32 = 1.8;         // how far down the tube that reaches, in tube radii

// What uReact turns down, besides what the voices' envelopes already do:
// each of these is 1 at uReact = 1 and this at 0.
const FLY_LO: f32 = 0.6;         // flight speed
const HIT_LO: f32 = 0.2;         // how far the bass draws the tube in; how hot an attack burns
const PACE_LO: f32 = 0.55;       // how fast the kick's ridge and the sparks travel
const TWIST_LO: f32 = 0.15;      // how far brightness winds the tiles

// History rows per ring: ROWS_PER_RING at full reactivity, more when the
// flight is slower. Set at the top of scene().
var<private> gRows: f32;

// The centre line of the tube in world space, and its derivative.
fn tubeC(w: f32) -> vec2f {
  return 1.4 * vec2f(sin(0.11 * w), cos(0.085 * w + 1.0))
       + 0.22 * vec2f(sin(0.29 * w + 2.0), sin(0.345 * w));
}
fn tubeD(w: f32) -> vec2f {
  return 1.4 * vec2f(0.11 * cos(0.11 * w), -0.085 * sin(0.085 * w + 1.0))
       + 0.22 * vec2f(0.29 * cos(0.29 * w + 2.0), 0.345 * cos(0.345 * w));
}

// The spectrogram row a ring shows: the middle of its own. Never newer than
// now, and never older than the history holds (it would wrap round to now).
fn histRowOf(ring: f32) -> f32 {
  return clamp(ring * gRows + 0.5 * (gRows - 1.0), uHistHead - 250.0, uHistHead);
}

// Loudness of tile (cell, ring) at the low end, the middle and the high end
// of its band: three taps, so no partial falls between tiles.
fn tileV(cell: f32, ring: f32) -> vec3f {
  let row = histRowOf(ring);
  let w = (X_HI - X_LO) / N_AROUND;
  let x = X_LO + (cell + 0.5) * w;
  return vec3f(histRow(x - 0.32 * w, row), histRow(x, row), histRow(x + 0.32 * w, row));
}
fn max3(v: vec3f) -> f32 {
  return max(v.x, max(v.y, v.z));
}
fn min4(v: vec4f) -> f32 {
  return min(min(v.x, v.y), min(v.z, v.w));
}

// Which tile of a ring an angle falls in: position in cells, mirrored.
fn cellAt(a: f32) -> f32 {
  return abs(fract(a * K_SYM / TAU) * 2.0 - 1.0) * N_AROUND;
}

// How far an angle around the tube is from each of four seams between tiles,
// in tiles. A seam and its twin on the far side of the tube together make a
// straight line across the picture.
fn seamDist(a: f32, seams: vec4f) -> vec4f {
  let u = a * K_SYM / TAU * SEAMS;
  return abs(fract((vec4f(u) - seams) / SEAMS + 0.5) - 0.5) * SEAMS;
}

// Harmony of one ring, read straight from its row of the spectrogram: the
// angle on the circle of fifths of the pitch classes sounding at that moment
// (C4 to B5), steadied by the harmony sounding now. Cyclic, 0..1.
fn ringHue(ring: f32) -> f32 {
  let row = histRowOf(ring);
  let x0 = 0.34513;     // C4 on the log-frequency axis
  let dx = 0.0091991;   // one semitone
  var vals: array<f32, 12>;
  var mean = 0.0;
  for (var i = 0; i < 12; i++) {
    let x = x0 + f32(i) * dx;
    let v = histRow(x, row) + histRow(x + 12.0 * dx, row);
    vals[i] = v;
    mean += v;
  }
  // What all twelve share is drums and noise, not harmony.
  mean /= 12.0;
  var k = vec2f(0.0);
  var tot = 0.0;
  var dir = vec2f(1.0, 0.0);
  // Seven semitones per step walks the circle of fifths: turn by 210 degrees.
  let r7 = mat2x2f(-0.8660254, -0.5, 0.5, -0.8660254);
  for (var i = 0; i < 12; i++) {
    // The square root keeps one loud note from outvoting a whole chord.
    let w = sqrt(max(vals[i] - mean, 0.0));
    k += w * dir;
    tot += w;
    dir = r7 * dir;
  }
  let ha = uHarmony * TAU;
  k = k / max(tot, 0.3) + 0.35 * vec2f(cos(ha), sin(ha));
  return atan2(k.y, k.x) / TAU;
}

// cos and sin of twice the angle of q around the tube, turned by phi.
fn oval(q: vec2f, phi: f32) -> vec2f {
  let c = vec2f(q.x * q.x - q.y * q.y, 2.0 * q.x * q.y) / max(dot(q, q), 1e-6);
  let r = vec2f(cos(2.0 * phi), sin(2.0 * phi));
  return vec2f(c.x * r.x - c.y * r.y, c.y * r.x + c.x * r.y);
}

fn bump4(x: vec4f) -> vec4f {
  let t = max(vec4f(1.0) - x * x, vec4f(0.0));
  return t * t;
}

// Light behind a shock front at d = 0: a sharp leading edge and a tail.
fn wake4(d: vec4f) -> vec4f {
  return exp(min(d, vec4f(0.0)) / 0.9) * (vec4f(1.0) - smoothstep(vec4f(0.0), vec4f(0.6), d));
}

fn scene(uv: vec2f) -> vec3f {
  // ---- reactivity ------------------------------------------------------------
  // The drums' envelopes already follow uReact. What is left to turn down is
  // the flight, the twist, how fast the kick's ridge travels, and the two
  // voices that are levels rather than hits.
  let hit = mix(HIT_LO, 1.0, uReact);
  let pace = mix(PACE_LO, 1.0, uReact);
  // The light of the bass and the tune: 1 at the default (0.5).
  let shine = mix(0.65, 1.35, uReact);
  // Flight. A calmer tube flies slower, so each ring holds more rows of the
  // spectrogram. A whole number of rings must fit its 256 rows, or the tiles
  // would jump every time the head wraps round.
  let nRings = round(HIST_ROWS / ROWS_PER_RING * mix(FLY_LO, 1.0, uReact));
  gRows = HIST_ROWS / nRings;
  let fly = ROWS_PER_RING / gRows;
  // The history is only as long as the full-speed flight (four seconds).
  // Slower, the rings set off from here, short of the far end; from here to
  // the far end they all show "now", like the flower.
  let zBorn = Z_FAR * fly;

  let px = 2.0 / uRes.y / FOCAL;
  let p = aspect(uv) / FOCAL;

  // ---- the five voices ---------------------------------------------------------
  // KICK: a ridge that squeezes the tube and runs away down it. It swells in
  // as it enters the picture and lets go as it recedes.
  let kEnv = vec4f(kickEnv(0), kickEnv(1), kickEnv(2), kickEnv(3));
  let kZ = KICK_Z0 + uKickAges * pace * KICK_SPEED;
  let kAmp = KICK_A * kEnv;

  // SNARE: a crack along one seam of the tube, a different one each time (11
  // seams on from the last, so no two in a row are near each other).
  let sEnv = vec4f(snareEnv(0), snareEnv(1), snareEnv(2), snareEnv(3));
  let sN = (vec4f(uSnareN) - vec4f(0.0, 1.0, 2.0, 3.0)) * 11.0;
  let sSeam = sN - SEAMS * floor(sN / SEAMS);

  // BASS: the nearest rings draw in. (The tail of a kick reads as a bass line
  // of about a quarter; that is not one.)
  let bass = sstep(0.3, 0.8, uBassLine);
  let bAmp = BASS_A * hit * bass;

  // TUNE: the flower at the far end. As bright as there is tune, plucked by
  // each note; it grows as the tune climbs.
  let tune = uTune * (0.6 * shine + 0.9 * noteSum());
  let reach = mix(0.5, 1.5, uPitch);

  // ---- camera ----------------------------------------------------------------
  let cz = uTime * SPEED * fly;
  let c0 = tubeC(cz);
  let slope = tubeD(cz + 1.5);

  let spin = uTime * 0.05;
  // Twist: the brighter the sound, the tighter the tiles wind around the tube.
  let tau = (uCentroid - 0.3) * 0.45 * mix(TWIST_LO, 1.0, uReact * uReact);
  // A full mix flattens the tube into an oval, long axis through the bass
  // tiles, and the twist winds the oval too.
  let ell = 0.11 * sstep(0.25, 0.9, uEnergy) + 0.07 * uIdle * sin(uTime * 0.23);

  // A ray that is outside a ridge's throat when it gets there has already hit
  // the wall: do not let the search step past it.
  var zLim = 99.0;
  for (var k = 0; k < 4; k++) {
    let zk = kZ[k];
    if (zk > 0.0 && zk < zLim && kAmp[k] > 0.002) {
      let qk = zk * p - (tubeC(cz + zk) - c0 - zk * slope);
      let e2 = oval(qk, spin + tau * zk);
      let rk = 1.0 - ell * e2.x - kAmp[k] - bAmp * exp(-zk * zk / (BASS_Z * BASS_Z));
      if (dot(qk, qk) > rk * rk) { zLim = zk; }
    }
  }

  // ---- find the wall -----------------------------------------------------------
  var z = 0.0;
  var q = vec2f(0.0);
  var m = 1.0;
  var rw = 1.0;
  var dr = 0.0;
  let pl = length(p);
  for (var i = 0; i < 9; i++) {
    let d = tubeC(cz + z) - c0 - z * slope;
    let dd = tubeD(cz + z) - slope;
    q = z * p - d;
    let ql = max(length(q), 1e-4);
    let x = (vec4f(z) - kZ) / KICK_W;
    let t = max(vec4f(1.0) - x * x, vec4f(0.0));
    let e2 = oval(q, spin + tau * z);
    let gb = bAmp * exp(-z * z / (BASS_Z * BASS_Z));
    rw = 1.0 - ell * e2.x - dot(kAmp, t * t) - gb;
    dr = 2.0 * ell * tau * e2.y + dot(kAmp, 4.0 * x * t) / KICK_W + 2.0 * z / (BASS_Z * BASS_Z) * gb;
    m = dot(q, p - dd) / ql - dr;
    if (i < 8) {
      let g = rw - ql;
      let stp = g / max(m, 0.3 * pl + 0.04);
      z = clamp(z + clamp(stp, -2.0, 2.5), 0.0, zLim);
    }
  }
  let zw = min(z, Z_FAR);
  let th = atan2(q.y, q.x);
  let sHead = uHistHead / gRows;
  let sw = sHead + (zw - zBorn) / RING_LEN;
  let fS = max(fwidth(zw) / RING_LEN, 1e-4);
  let fT = max(K_SYM * N_AROUND / PI * px * zw / max(rw, 0.5), 1e-5);
  let cellW = PI / (K_SYM * N_AROUND) * rw;
  // Pixel size on the wall, in tube radii: fine detail fades below this.
  let fw = max(fS * RING_LEN, fT * cellW);
  let fine = 1.0 - sstep(0.3, 1.0, fw / BEVEL);
  // The slope of a ridge facing the camera catches the light.
  let facing = 1.0 + 4.0 * max(-dr, 0.0);

  // ---- relief: walk the rings the ray crosses on its way down to the wall ---
  let mm = max(m, 0.02);
  let hs = min(1.0, f32(STEPS - 1) * mm * RING_LEN / H_MAX);
  let hzone = H_MAX * hs * 1.05;
  let sIn = sw - hzone / (mm * RING_LEN);
  let j0 = floor(sIn);
  let jw = floor(sw);
  let fh = fS * mm * RING_LEN;
  // A quiet mix gets a gentler curve, so a lone pad still lights its tiles.
  let full = sstep(0.3, 0.85, uEnergy);
  let curve = mix(2.2, 4.0, full);
  let gain = 1.1 + 1.6 * uIdle;
  // The small sun at the far end: brighter in a full mix, never in a hurry.
  let sun = 0.35 + 0.9 * uEnergy;

  // Far away, where a ring is thinner than a pixel, read the spectrogram
  // between rings instead of snapping to one.
  let far = sstep(0.6, 1.5, fS) * (sw - 0.5 - jw);

  // The kick's line of light is drawn on each surface at that surface's own
  // depth. (Thinner right at the lens, where it would fill the screen.)
  // Blending depths first would light the silhouettes instead.
  let wl0 = mix(0.02, 0.05, sstep(0.0, 2.5, zw));
  let wl = max(wl0, 1.5 * fS * RING_LEN);

  var cov = 0.0;      // coverage so far
  var accL = vec4f(0.0);  // the four kick lines
  var accE = 0.0;     // the tiles' own glow
  var accK = 0.0;     // kick light on faces turned to the camera
  var accR = 0.0;     // light from the far end: rims and glints
  var accZ = 0.0;     // depth of what was hit
  var ringF = jw;     // ring of the front-most thing hit
  var gotF = 0.0;
  var vWall = 0.0;
  var cellF = 0.0;
  var uuW = 0.0;
  // The ring before the first, so the first knows whether it is an attack.
  var vOld = max3(tileV(min(floor(cellAt(th + spin + tau * (zBorn + (j0 - 0.5 - sHead) * RING_LEN))), N_AROUND - 1.0), j0 - 1.0));
  for (var i = 0; i < STEPS; i++) {
    let j = j0 + f32(i);
    if (j > jw) { break; }
    let zc = zBorn + (j + 0.5 - sHead) * RING_LEN;
    let ang = th + spin + tau * zc;
    let fa = cellAt(ang);
    let cell = min(floor(fa), N_AROUND - 1.0);
    let v3 = tileV(cell, j + far);
    let v = max3(v3);
    let att = max(v - vOld, 0.0);
    vOld = v;
    let hj = H_MAX * hs * sstep(0.3, 0.95, v);

    // The snare's crack: the tiles either side of its seam fall open like
    // louvres, and the two that meet at it draw back from each other.
    let sd4 = seamDist(ang, sSeam);
    let sHere = sEnv * (1.0 - sstep(CRACK_END - 4.0, CRACK_END, zc));
    let open = dot(sHere, bump4(sd4 / CRACK_BAND));
    let gapS = mix(GAP_S, 0.4, min(1.6 * open, 1.0) * 0.7);
    let part = min4(clamp((sd4 - GAP_T - CRACK_PART * sHere) / fT + 0.5, vec4f(0.0), vec4f(1.0)));

    let s0 = max(sIn, j + gapS);
    let s1 = j + 1.0 - gapS;
    let h0 = (sw - s0) * mm * RING_LEN;
    let h1 = (sw - s1) * mm * RING_LEN;
    let uu = fract(fa) - 0.5;
    // Where in its band the tile is sounding: its glow sits there.
    let wu = clamp(uu / 0.32, -1.0, 1.0);
    let vl = mix(v3.y, select(v3.x, v3.z, wu > 0.0), wu * wu * (3.0 - 2.0 * abs(wu)));
    let aT = min(clamp((0.5 - GAP_T - abs(uu)) / fT + 0.5, 0.0, 1.0), part);
    let aS = clamp((hj - h1) / fh + 0.5, 0.0, 1.0) * clamp((sw - (j + gapS)) / fS + 0.5, 0.0, 1.0);
    let al = aT * aS;
    let side = clamp((hj - h0) / fh + 0.5, 0.0, 1.0);

    // Where the ray lands on the top face, and how far that is from an edge.
    let sTop = sw - hj / (mm * RING_LEN);
    let vv = clamp(sTop - j - 0.5, -0.5, 0.5);
    let eS = (0.5 - gapS - abs(vv)) * RING_LEN;
    let eT = (0.5 - GAP_T - abs(uu)) * cellW;
    let de = min(eS, eT);
    let bS = (1.0 - sstep(0.0, BEVEL, eS)) * fine;
    let bT = (1.0 - sstep(0.0, BEVEL, eT)) * fine;
    let nearB = bS * step(vv, 0.0);
    let farB = bS * step(0.0, vv);
    // An engraved inner border, and a soft dome of light inside it.
    let groove = fine * sstep(0.008, 0.0, abs(de - 0.034));
    let dome = sstep(0.0, 0.5 * cellW, eT) * sstep(0.0, 0.11, eS);
    // The far end glinting down the middle of the glass.
    let glint = fine * exp(-uu * uu * 110.0) * sstep(0.0, 0.05, eS) * (0.5 + vv);
    let toSun = 0.25 + 0.75 * sstep(0.0, Z_FAR, zc);

    // Tiles are born hot and cool as they fly; where a sound starts, hotter.
    let heat = mix(0.7, 1.0, sstep(0.3, 5.0, zc)) + 0.5 * sstep(7.0, Z_FAR, zc);
    let e = (pow(mix(v, vl, 0.8 * fine), curve) * gain + att * v * 0.9 * hit) * heat;
    // The kick's light, on whatever its ridge is passing.
    let ls = dot(kEnv, exp(-abs(vec4f(zc) - kZ) / vec4f(1.0)));

    var top = e * mix(1.0, 0.7 + 0.45 * dome, fine) * (0.85 + 0.3 * vv) + 0.010;
    top *= (1.0 - 0.3 * bT - 0.3 * farB) * (1.0 - 0.45 * groove);
    let topK = ls * facing * (0.05 + 0.08 * v + (0.2 + 0.5 * v) * nearB);
    let sd = e * 0.35 + 0.005;
    let sdK = ls * (0.10 + 0.5 * v);
    let rim = (farB + 0.7 * glint) * toSun * (1.0 - side);

    let wgt = (1.0 - cov) * al;
    let zHit = zBorn + (mix(sTop, s0, side) - sHead) * RING_LEN;
    let dl = (vec4f(zHit) - kZ) / wl;
    accE += wgt * mix(top, sd, side);
    accK += wgt * mix(topK, sdK, side);
    accR += wgt * rim;
    accZ += wgt * zHit;
    accL += wgt * exp(-dl * dl);
    cov += wgt;
    if (gotF < 0.5 && (al > 0.5 || j == jw)) {
      gotF = 1.0;
      ringF = j;
      cellF = cell;
    }
    if (j == jw) {
      vWall = v;
      uuW = uu;
    }
  }
  let grout = 1.0 - cov;
  let zs = accZ + grout * zw;   // depth of the surface in this pixel

  // ---- grout: a lattice of filaments that carries the tiles' glow and the
  // light of the kick, the snare and the bass ----------------------------------
  let dS = (0.5 - abs(sw - jw - 0.5)) * RING_LEN;
  let dT = (0.5 - abs(uuW)) * cellW;
  let wf = max(0.006, 0.8 * fw);
  let fil = (exp(-dS * dS / (wf * wf)) + exp(-dT * dT / (wf * wf))) * min(1.0, 0.009 / wf);
  let glowG = exp(-min(dS, dT) / 0.035);
  accE += grout * (0.002 + 0.08 * pow(vWall, curve) * gain);
  let groutC = grout * (0.035 + 0.035 * (1.0 - full)) * fil;

  // ---- KICK: a line of light riding each ridge's crest, the lattice lit in
  // its wake, and the tiles it is passing ---------------------------------------
  let dlw = (vec4f(zw) - kZ) / wl;
  let lens = mix(0.5, 1.0, sstep(0.5, 2.5, zw));
  let line = dot(kEnv, accL + grout * exp(-dlw * dlw)) * min(1.0, wl0 / wl) * lens;
  let gk = dot(kEnv, wake4(vec4f(zw) - kZ));
  let band = dot(kEnv, exp(-abs(vec4f(zw) - kZ) / vec4f(0.8)));
  let kickL = line * 1.3 + band * 0.25 + accK * 1.8 + grout * gk * fil * 0.25;

  // ---- SNARE: light bursting through the crack ---------------------------------
  let cd4 = seamDist(th + spin + tau * zw, sSeam);
  let cph = cd4 * cellW;
  let wc = max(0.014, 1.2 * fT * cellW);
  let core = exp(-cph * cph / (wc * wc)) * min(1.0, 0.014 / wc);
  let snareL = dot(sEnv, core * 1.2 + exp(-cd4 / 0.7) * 0.1 + bump4(cd4 / CRACK_BAND) * grout * (0.55 * fil + 0.2 * glowG))
             * (1.0 - sstep(CRACK_END - 4.0, CRACK_END, zw));

  // ---- HATS: pin-point sparks on the wall, round the rim of the picture and
  // mostly over the top ---------------------------------------------------------
  var spark = 0.0;
  let wsp = max(0.011, 1.2 * fT * cellW);
  let asp = uRes.x / uRes.y;
  for (var k = 0; k < 4; k++) {
    let e = hatEnv(k);
    if (e > 0.004) {
      let seed = uHatN - f32(k);
      let run = uHatAges[k] * pace * HAT_SPEED;
      for (var s = 0; s < HAT_SPARKS; s++) {
        let h = hash22(vec2f(seed, f32(s) * 7.31 + 1.7));
        // From a little below one side, over the top, to a little below the
        // other.
        let a = mix(-0.35, PI + 0.35, h.x);
        // How far out the picture goes that way, in half heights; the spark
        // sits on the wall where it shows at 55 to 90% of that.
        let rim = min(1.0 / max(abs(sin(a)), 1e-3), asp / max(abs(cos(a)), 1e-3));
        let zh = FOCAL / (rim * mix(0.55, 0.9, h.y)) + run;
        let dth = (fract((th - a) / TAU + 0.5) - 0.5) * TAU * rw;
        let dz = (zh - zs) / (zh * zh);
        spark += exp(-dth * dth / (wsp * wsp)) * exp(-max(dz, 0.0) / 0.02) * sstep(-0.012, 0.0, dz) * e;
      }
    }
  }
  spark *= min(1.0, 0.011 / wsp);

  // ---- BASS: the seams of the nearest rings glow, round the rim of the
  // picture, and reach further in the more bass there is. (The seams run the
  // way the tube flies, so they stand still as the tiles rush by: a glow, not
  // a flicker.) ---------------------------------------------------------------
  let rho = length(p * FOCAL * vec2f(uRes.y / uRes.x, 1.0));   // 0 centre, 1 the edge
  let bIn = mix(0.12, 0.45, bass);
  let dB = (0.5 - abs(fract(cellAt(th + spin + tau * zw)) - 0.5)) * cellW;
  let wb = max(0.007, fT * cellW);
  let bassL = shine * bass * sstep(1.0 - bIn, 1.0 - 0.3 * bIn, rho)
            * (0.22 * exp(-dB * dB / (wb * wb)) * min(1.0, 0.007 / wb) + 0.02 * exp(-dB / 0.03) + 0.012);

  // ---- colour ---------------------------------------------------------------
  let hue = ringHue(ringF) * 2.0;
  let tc = (cellF + 0.5) / N_AROUND * 0.35 + hue;
  let tint = pal(tc);
  let farC = pal(hue + 0.15);
  // With nothing playing the bass and the tune only drift, and what they move
  // is scenery.
  let bassC = mix(voiceCol(BASS), pal(tc + 0.5), uIdle);
  let tuneC = mix(voiceCol(TUNE), farC, uIdle);
  // Dim tiles take a deeper shade of their colour, so they never go muddy.
  var col = pow(tint, vec3f(1.0 + 2.2 * (1.0 - sstep(0.0, 0.55, accE)))) * accE;
  col += farC * accR * sun * 0.22;
  col += pal(tc + 0.5) * groutC;
  col += voiceCol(KICK) * kickL;
  col += voiceCol(SNARE) * snareL;
  col += voiceCol(HATS) * spark * 6.0;
  col += bassC * bassL;

  let fog = exp(-zw * 0.10);
  col = col * fog;

  // ---- the mouth, where now is born ---------------------------------------------
  let cm = tubeC(cz + Z_FAR) - c0 - Z_FAR * slope;
  let pm = Z_FAR * p - cm;
  let rm = length(pm);
  let seen = sstep(Z_FAR - 3.0, Z_FAR - 0.5, z);
  let depth = 1.0 - exp(-zw * 0.12);
  // The small sun, and its light hanging in the haze, strongest when the mix
  // is thin and there is little else to see.
  col += farC * seen * exp(-rm * rm * 9.0) * sun * 1.4;
  col += farC * farC * depth * depth * sun * 0.75 / (1.0 + 0.12 * rm) * (0.08 + 0.3 * (1.0 - full));

  // ---- TUNE: the flower. Twelve petals, one per note in order round it, each
  // as long as its note is loud; the whole of it larger the higher the tune ---
  let x12 = fract((atan2(pm.y, pm.x) + spin) / TAU) * 12.0;
  let lobe = pow(max(cos(PI * (fract(x12) - 0.5)), 0.0), 0.7);
  let sv = mix(0.4, chroma(i32(floor(x12))) * lobe, sstep(0.08, 0.3, rm));
  let edge = (0.3 + 1.1 * sv) * reach;
  let soft = 0.12 + 2.0 * px * Z_FAR;
  let petal = sstep(edge + soft, edge - soft, rm) * max(1.15 - 0.5 * rm / edge, 0.0);
  // Round it, the same petals again in the haze, soft and much larger: they
  // fill the middle of the picture, and reach further the higher the tune.
  let pl2 = (0.35 + 0.65 * sv) * 4.5 * reach;
  let rays = (1.0 - exp(-zw * 0.25)) * sstep(1.0, 0.45, rm / pl2) * (1.0 - 0.5 * min(rm / pl2, 1.0));
  col += tuneC * tune * (seen * petal * 2.0 + rays * 1.3);
  return col;
}
