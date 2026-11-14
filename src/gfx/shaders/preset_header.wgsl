// Shared by every preset. A preset is a WGSL fragment that defines
//
//     fn scene(uv: vec2f) -> vec3f
//
// returning linear HDR colour (values above 1 bloom). `uv` is (0,0) top-left.
//
// A preset that has to remember more than its own picture (marked `stateful`
// in src/gfx/presets.rs) defines these two instead:
//
//     fn state(uv: vec2f) -> vec4f   four numbers a pixel, kept from frame to
//                                    frame and never shown
//     fn show(uv: vec2f) -> vec3f    the picture, made from them
//
// state() runs first and reads the state of the frame before with kept();
// show() runs second, and there kept() reads the state just written. So what
// is slow to work out can be kept and reused, while what must answer the
// music at once is drawn afresh every frame.
//
// Everything named u* below is set before scene() runs and can be read from
// any function. The textures (spectrum, waveform, chroma, history) are 8-bit:
// good for colour and brightness, too coarse to drive geometry or anything
// that gets differentiated. Use the u* floats for shape.

struct Globals {
  a: vec4f, // resolution.xy, time, dt
  b: vec4f, // bass, mid, treble, level
  c: vec4f, // beat pulse, audio phase, beat count, history head row
  d: vec4f, // frame-rate factor (dt * 60), centroid, idle, energy
  e: vec4f, // onset pulses: low, mid, high; harmony
  f: vec4f, // onset counts: low, mid, high; tonal
  beat_ages: vec4f,
  low_ages: vec4f,
  mid_ages: vec4f,
  high_ages: vec4f,
  g: vec4f, // reactivity, bass line, tune, pitch
  note_ages: vec4f,
  kick_hits: vec4f,
  snare_hits: vec4f,
  hat_hits: vec4f,
  note_hits: vec4f,
  h: vec4f, // note pulse, note count, unused
  drive: array<vec4f, 10>,
}

@group(0) @binding(0) var<uniform> G: Globals;
@group(0) @binding(1) var tAudio: texture_2d<f32>;
@group(0) @binding(2) var tHist: texture_2d<f32>;
@group(0) @binding(3) var tPal: texture_2d<f32>;
@group(0) @binding(4) var sClamp: sampler;
@group(0) @binding(5) var sHist: sampler;
@group(0) @binding(6) var sPal: sampler;
@group(0) @binding(7) var tData: texture_2d<f32>;
@group(1) @binding(0) var tPrev: texture_2d<f32>;

// Render-target size in pixels.
var<private> uRes: vec2f;
// Seconds since launch, and seconds since the last frame.
var<private> uTime: f32;
var<private> uDt: f32;
// Smoothed band energies, 0..1.
var<private> uBass: f32;
var<private> uMid: f32;
var<private> uTreb: f32;
var<private> uLevel: f32;
// 1 on a beat (an onset below 200 Hz: the kick), decaying to 0 over about
// half a second; and how many beats there have been.
var<private> uBeat: f32;
var<private> uBeats: f32;
// Seconds since each of the last four beats, newest first (10 = long ago or
// never). An event launched on a beat knows how far it has travelled, and
// the three before it are still there to fade out instead of popping:
//   for (var k = 0; k < 4; k++) { ripple(uBeatAges[k], uBeats - f32(k)); }
// The second argument is that beat's number: hash it for a per-event seed.
var<private> uBeatAges: vec4f;
// uBeatAges.x: seconds since the last beat.
var<private> uBeatAge: f32;
// Older names for the drum voices (see "The five voices" below, and prefer
// those): Low is the kick, Mid the snare, High the hats.
var<private> uOnsetLow: f32;
var<private> uOnsetMid: f32;
var<private> uOnsetHigh: f32;
var<private> uOnsetLowN: f32;
var<private> uOnsetMidN: f32;
var<private> uOnsetHighN: f32;
var<private> uOnsetLowAges: vec4f;
var<private> uOnsetMidAges: vec4f;
var<private> uOnsetHighAges: vec4f;
// Spectral centroid, 0..1 along the same log-frequency axis as spec(): how
// bright the sound is right now. Smooth. About 0.25 for a bass-heavy mix,
// 0.35-0.45 for a bright one, 0.6 and up for a riser or hats on their own.
var<private> uCentroid: f32;
// How full the mix has been over the last couple of seconds, 0..1: low in a
// breakdown, high in a drop. Compare with uLevel to catch the change.
var<private> uEnergy: f32;
// Where the harmony sits on the circle of fifths, 0..1 and CYCLIC: use it as
// a palette offset or an angle, e.g. pal(uHarmony + ...). It glides to a new
// value when the chord changes, and related chords are close together: the
// chords of one key span only about 0.15. Multiply it by a whole number
// (which keeps it cyclic) for a bigger swing.
var<private> uHarmony: f32;
// How clearly the sound has a tonal centre: 0 noise or silence, towards 1 a
// single clear note. A major triad is about 0.6.
var<private> uTonal: f32;
// 1 while nothing is playing and the ambient idle animation is feeding the
// spectrum, waveform and chroma; 0 with music. There are no beats or onsets
// while idle, so give the picture a life of its own (from uTime) then.
var<private> uIdle: f32;
// How hard the user wants the picture to react, 0..1 (a setting; 0.5 by
// default). The signals above already follow it: pulses peak at uReact, the
// band energies swing less, the spectrum moves more smoothly and fewer claps
// and hats count. The rest is the preset's job. Scale by it the size of
// anything an event sets in motion (a shove, a punch, a squash, a camera
// kick, a flash), and let it slow whatever moves all the time, for example
// speed * mix(0.5, 1.0, uReact). At 0 the picture should still drift and
// still show the music, only gently.
var<private> uReact: f32;
// A clock that runs faster when the music is louder.
var<private> uPhase: f32;
// Fractional row of "now" in the spectrogram history.
var<private> uHistHead: f32;
// dt * 60: scale per-frame feedback amounts by this to stay rate independent.
var<private> FR: f32;

// A preset may have a companion in Rust (src/gfx/driver.rs) that runs once a
// frame and works out what a shader cannot: these are the forty numbers
// it sends, and driveData() reads the table it keeps. All zero for a preset
// with no driver.
var<private> uDrive: array<vec4f, 10>;

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;
const HIST_ROWS: f32 = 256.0;

// The active colour palette, cyclic in t.
fn pal(t: f32) -> vec3f {
  return textureSampleLevel(tPal, sPal, vec2f(t, 0.25), 0.0).rgb;
}

// ---- The five voices --------------------------------------------------------
//
// buttercandy hears five voices in the music and tells them apart by their
// sound, not just their pitch: the KICK (a hit in the low end), the SNARE
// (or clap: a burst of noise through the mids), the HATS (a burst of noise
// up high with nothing under it), the BASS line (the low end that stays when
// the kicks are taken out) and the TUNE (melody and chords: the pitched part
// of the mids).
//
// Every preset answers each voice in the same recognisable way, so that a
// viewer can tell what the picture is reacting to, and learns it once for all
// of them. Three cues, always together:
//
//   colour   each voice owns one colour of the palette, voiceCol(VOICE).
//            Whatever a voice sets off is drawn in its colour, and nothing
//            else is drawn in a way that could be mistaken for it. pal() is
//            for the scenery: whatever is there regardless of who is playing.
//   motion   KICK   a slow, heavy push outward, then a slow release.
//            SNARE  a crack: sudden, bright, straight, gone in an instant.
//            HATS   tiny quick sparkles.
//            BASS   a swell: the size or weight of something, breathing.
//            TUNE   a glide: height follows the pitch, never a jolt.
//            The envelopes below are these motions; use them rather than
//            inventing others.
//   place    KICK   the biggest thing in the picture, from the centre or
//                   from below.
//            SNARE  cuts across the picture, at the scale of a limb.
//            HATS   the finest detail, in the upper part or round the edge.
//            BASS   the lowest or outermost mass.
//            TUNE   the middle, moving up and down with the pitch.
//
// A voice that is not playing leaves no trace: with only a kick drum
// sounding, nothing in the snare's colour moves.
const KICK: i32 = 0;
const SNARE: i32 = 1;
const HATS: i32 = 2;
const BASS: i32 = 3;
const TUNE: i32 = 4;

// The colour a voice owns in the active palette.
fn voiceCol(v: i32) -> vec3f {
  return textureSampleLevel(tPal, sPal, vec2f((f32(v) + 0.5) / 5.0, 0.75), 0.0).rgb;
}

// The drums. For each: a pulse (1 on a hit, decaying; peaks at uReact), how
// many hits there have been (hash it for a per-hit seed), seconds since each
// of the last four hits, newest first (10 = long ago or never), and how hard
// each of those four was, 0..1 (a ghost note is small, an accent is 1).
var<private> uKick: f32;
var<private> uKickN: f32;
var<private> uKickAges: vec4f;
var<private> uKickHits: vec4f;
var<private> uSnare: f32;
var<private> uSnareN: f32;
var<private> uSnareAges: vec4f;
var<private> uSnareHits: vec4f;
var<private> uHat: f32;
var<private> uHatN: f32;
var<private> uHatAges: vec4f;
var<private> uHatHits: vec4f;
// Notes starting in the tune (a pluck, a piano key, a sung syllable): the
// same four things.
var<private> uNote: f32;
var<private> uNoteN: f32;
var<private> uNoteAges: vec4f;
var<private> uNoteHits: vec4f;
// The bass line, 0..1: how much held bass there is, with the kicks taken out.
var<private> uBassLine: f32;
// The tune, 0..1: how much melody and harmony there is.
var<private> uTune: f32;
// How high the tune sits, 0..1: rises as a melody rises. Smooth.
var<private> uPitch: f32;

// The motions. Each returns 0..1 for hit k (0 is the newest, 3 the oldest)
// of its voice: how far into its gesture that hit is, already scaled by how
// hard it was and by uReact, with an attack that lengthens as uReact falls.
// Sum the four (see kickSum) and nothing pops when a new hit arrives.
// (taffy.wgsl and sprinkles.wgsl need the running total of these, and so
// repeat their shape and constants: change them together.)
fn hitEnv(age: f32, hit: f32, attack: f32, release: f32) -> f32 {
  let rise = attack * (1.0 + 2.5 * (1.0 - uReact));
  return sstep(0.0, rise, age) * exp(-max(age - rise, 0.0) / release) * hit * mix(0.35, 1.0, uReact);
}
// KICK: a slow, heavy push. In by about 60 ms, out over half a second.
fn kickEnv(k: i32) -> f32 {
  return hitEnv(uKickAges[k], uKickHits[k], 0.06, 0.20);
}
// SNARE: a crack. In at once, gone in a seventh of a second.
fn snareEnv(k: i32) -> f32 {
  return hitEnv(uSnareAges[k], uSnareHits[k], 0.012, 0.055);
}
// HATS: a tick. Shorter still.
fn hatEnv(k: i32) -> f32 {
  return hitEnv(uHatAges[k], uHatHits[k], 0.008, 0.03);
}
// A note of the TUNE: a pluck, ringing for a third of a second.
fn noteEnv(k: i32) -> f32 {
  return hitEnv(uNoteAges[k], uNoteHits[k], 0.02, 0.14);
}
// All four hits of a voice at once, for when the hits need not be told
// apart: a smooth 0..1 that never jumps down when a new hit arrives.
fn kickSum() -> f32 {
  return min(kickEnv(0) + kickEnv(1) + kickEnv(2) + kickEnv(3), 1.0);
}
fn snareSum() -> f32 {
  return min(snareEnv(0) + snareEnv(1) + snareEnv(2) + snareEnv(3), 1.0);
}
fn hatSum() -> f32 {
  return min(hatEnv(0) + hatEnv(1) + hatEnv(2) + hatEnv(3), 1.0);
}
fn noteSum() -> f32 {
  return min(noteEnv(0) + noteEnv(1) + noteEnv(2) + noteEnv(3), 1.0);
}

// Log-frequency spectrum, x in 0..1 from 30 Hz to 16 kHz.
fn spec(x: f32) -> f32 {
  return textureSampleLevel(tAudio, sClamp, vec2f(x, 0.1), 0.0).r;
}

// Oscilloscope waveform, -1..1.
fn wave(x: f32) -> f32 {
  return textureSampleLevel(tAudio, sClamp, vec2f(x, 0.3), 0.0).r * 2.0 - 1.0;
}

// Peak-hold spectrum: rises with spec(), then falls under gravity.
fn peak(x: f32) -> f32 {
  return textureSampleLevel(tAudio, sClamp, vec2f(x, 0.5), 0.0).r;
}

// Spectrum with a faster release, for twitchier motion.
fn fastSpec(x: f32) -> f32 {
  return textureSampleLevel(tAudio, sClamp, vec2f(x, 0.7), 0.0).r;
}

// Chroma: how strongly pitch class i is sounding, 0..1. 0 is C, then C#, D
// and so on up to 11, B; i wraps. Octaves are folded together, and what all
// twelve share (drums, noise) is taken out, so a chord lights its notes and
// a hi-hat lights none.
fn chroma(i: i32) -> f32 {
  let pc = f32(((i % 12) + 12) % 12);
  return textureSampleLevel(tAudio, sClamp, vec2f((pc * 42.0 + 21.0) / 512.0, 0.9), 0.0).r;
}

// Chroma at a continuous position around the pitch-class circle, blending
// smoothly between neighbouring notes. x is cyclic in 0..1 and pitch class i
// is centred on (i + 0.5) / 12.
fn chromaAt(x: f32) -> f32 {
  let p = fract(x) * 12.0 - 0.5;
  let i = i32(floor(p));
  let f = p - floor(p);
  return mix(chroma(i), chroma(i + 1), f * f * (3.0 - 2.0 * f));
}

// Spectrogram history: spectrum at frequency x, at an absolute row. Rows
// wrap; `uHistHead - n` is n rows (sixtieths of a second) ago.
fn histRow(x: f32, row: f32) -> f32 {
  return textureSampleLevel(tHist, sHist, vec2f(x, (row + 0.5) / HIST_ROWS), 0.0).r;
}

// Entry i of the driver's table: 32-bit floats, unfiltered, 16384 entries.
fn driveData(i: i32) -> vec4f {
  return textureLoad(tData, vec2i(i & 1023, i >> 10), 0);
}

// This preset's previous frame, for feedback effects.
fn prev(uv: vec2f) -> vec3f {
  return textureSampleLevel(tPrev, sClamp, uv, 0.0).rgb;
}

// A stateful preset's kept numbers, blended between pixels. (For one pixel's
// own, unblended: textureLoad(tPrev, vec2i(uv * uRes), 0).)
fn kept(uv: vec2f) -> vec4f {
  return textureSampleLevel(tPrev, sClamp, uv, 0.0);
}

// Floored modulo (the WGSL % operator truncates).
fn fmod(x: f32, y: f32) -> f32 {
  return x - y * floor(x / y);
}

fn rot(a: f32) -> mat2x2f {
  let c = cos(a);
  let s = sin(a);
  return mat2x2f(c, -s, s, c);
}

// Rotate a colour's hue by `a` radians.
fn hueRot(c: vec3f, a: f32) -> vec3f {
  let k = vec3f(0.57735);
  let ca = cos(a);
  return c * ca + cross(k, c) * sin(a) + k * dot(k, c) * (1.0 - ca);
}

fn hash21(p: vec2f) -> f32 {
  var p3 = fract(vec3f(p.x, p.y, p.x) * 0.1031);
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.x + p3.y) * p3.z);
}

fn hash22(p: vec2f) -> vec2f {
  var p3 = fract(vec3f(p.x, p.y, p.x) * vec3f(0.1031, 0.1030, 0.0973));
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.xx + p3.yz) * p3.zy);
}

fn vnoise(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  return mix(
    mix(hash21(i), hash21(i + vec2f(1.0, 0.0)), u.x),
    mix(hash21(i + vec2f(0.0, 1.0)), hash21(i + vec2f(1.0, 1.0)), u.x),
    u.y,
  );
}

fn fbm(p0: vec2f) -> f32 {
  var p = p0;
  var amp = 0.5;
  var sum = 0.0;
  for (var i = 0; i < 5; i++) {
    sum += amp * vnoise(p);
    p = p * 2.02 + vec2f(17.1, 9.3);
    amp *= 0.5;
  }
  return sum;
}

// uv -> aspect-correct coordinates: y up, -1..1 vertically, origin centred.
fn aspect(uv: vec2f) -> vec2f {
  return vec2f((uv.x - 0.5) * 2.0 * uRes.x / uRes.y, (0.5 - uv.y) * 2.0);
}

// The inverse of aspect().
fn unaspect(p: vec2f) -> vec2f {
  return vec2f(p.x * uRes.y / uRes.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
}
