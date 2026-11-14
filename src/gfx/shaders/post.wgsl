// Post-processing: bloom prefilter and the final composite. Appended to
// fullscreen.wgsl.

struct Post {
  a: vec4f, // transition mix, bloom threshold, bloom knee, bloom amount
  b: vec4f, // beat, time, aberration, grain
  c: vec4f, // vignette, exposure, bloom normalisation, unused
  d: vec4f, // output size in pixels, unused
}

@group(0) @binding(0) var<uniform> P: Post;
@group(0) @binding(1) var tA: texture_2d<f32>;
@group(0) @binding(2) var tB: texture_2d<f32>;
@group(0) @binding(3) var tBloom: texture_2d<f32>;
@group(0) @binding(4) var samp: sampler;

fn hash(p: vec2f) -> f32 {
  var p3 = fract(vec3f(p.x, p.y, p.x) * 0.1031);
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.x + p3.y) * p3.z);
}

fn noise(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  return mix(
    mix(hash(i), hash(i + vec2f(1.0, 0.0)), u.x),
    mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), u.x),
    u.y,
  );
}

// The scene: the current preset, dissolving into the next during a transition.
fn scene_at(uv: vec2f) -> vec3f {
  let a = textureSampleLevel(tA, samp, uv, 0.0).rgb;
  let m = P.a.x;
  if (m <= 0.0) {
    return a;
  }
  let b = textureSampleLevel(tB, samp, uv, 0.0).rgb;
  let n = noise(uv * vec2f(5.0, 3.0)) * 0.6 + noise(uv * vec2f(13.0, 8.0)) * 0.4;
  return mix(a, b, sstep(0.0, 1.0, m * 1.5 - 0.25 + (n - 0.5) * 0.5));
}

// Downsample to half resolution, keeping only what is bright enough to bloom.
@fragment
fn fs_prefilter(in: VsOut) -> @location(0) vec4f {
  let ts = 1.0 / vec2f(textureDimensions(tA));
  var c = scene_at(in.uv + ts * vec2f(-1.0, -1.0));
  c += scene_at(in.uv + ts * vec2f(1.0, -1.0));
  c += scene_at(in.uv + ts * vec2f(-1.0, 1.0));
  c += scene_at(in.uv + ts * vec2f(1.0, 1.0));
  c *= 0.25;
  let threshold = P.a.y;
  let knee = P.a.z;
  let br = max(c.r, max(c.g, c.b));
  var soft = clamp(br - threshold + knee, 0.0, 2.0 * knee);
  soft = soft * soft / (4.0 * knee + 1e-4);
  let keep = max(soft, br - threshold) / max(br, 1e-4);
  return vec4f(c * keep, 1.0);
}

// Narkowicz's ACES fit.
fn aces(x: vec3f) -> vec3f {
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3f(0.0), vec3f(1.0));
}

// Scene + bloom, lens effects, tonemap and gamma. Writes display-referred colour.
@fragment
fn fs_composite(in: VsOut) -> @location(0) vec4f {
  let uv = in.uv;
  let d = uv - 0.5;
  let beat = P.b.x;

  // Chromatic aberration that grows towards the corners and kicks on beats.
  let ca = P.b.z * (1.0 + 2.0 * beat) * dot(d, d);
  var col = vec3f(scene_at(uv + d * ca).r, scene_at(uv).g, scene_at(uv - d * ca).b);

  col += textureSampleLevel(tBloom, samp, uv, 0.0).rgb * P.c.z * P.a.w;
  col *= 1.0 - P.c.x * dot(d, d) * 1.4;
  col = aces(col * P.c.y * (1.0 + 0.06 * beat));
  col = pow(col, vec3f(1.0 / 2.2));

  // Film grain, which doubles as dithering against banding.
  let n = hash(uv * P.d.xy + fract(P.b.y) * 100.0);
  col += (n - 0.5) * P.b.w;
  return vec4f(col, 1.0);
}
