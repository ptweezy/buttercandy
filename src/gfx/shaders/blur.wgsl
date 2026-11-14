// Dual-filter (Kawase) blur passes and a plain blit. Appended to
// fullscreen.wgsl. Used for both bloom and the frosted-glass UI backdrop.

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_down(in: VsOut) -> @location(0) vec4f {
  let o = 1.0 / vec2f(textureDimensions(src));
  var c = textureSampleLevel(src, samp, in.uv, 0.0) * 4.0;
  c += textureSampleLevel(src, samp, in.uv - o, 0.0);
  c += textureSampleLevel(src, samp, in.uv + o, 0.0);
  c += textureSampleLevel(src, samp, in.uv + vec2f(o.x, -o.y), 0.0);
  c += textureSampleLevel(src, samp, in.uv - vec2f(o.x, -o.y), 0.0);
  return c / 8.0;
}

@fragment
fn fs_up(in: VsOut) -> @location(0) vec4f {
  let o = 0.5 / vec2f(textureDimensions(src));
  var c = textureSampleLevel(src, samp, in.uv + vec2f(-o.x * 2.0, 0.0), 0.0);
  c += textureSampleLevel(src, samp, in.uv + vec2f(-o.x, o.y), 0.0) * 2.0;
  c += textureSampleLevel(src, samp, in.uv + vec2f(0.0, o.y * 2.0), 0.0);
  c += textureSampleLevel(src, samp, in.uv + vec2f(o.x, o.y), 0.0) * 2.0;
  c += textureSampleLevel(src, samp, in.uv + vec2f(o.x * 2.0, 0.0), 0.0);
  c += textureSampleLevel(src, samp, in.uv + vec2f(o.x, -o.y), 0.0) * 2.0;
  c += textureSampleLevel(src, samp, in.uv + vec2f(0.0, -o.y * 2.0), 0.0);
  c += textureSampleLevel(src, samp, in.uv + vec2f(-o.x, -o.y), 0.0) * 2.0;
  return c / 12.0;
}

// All four channels as they are, at whatever size the target is.
@fragment
fn fs_copy(in: VsOut) -> @location(0) vec4f {
  return textureSampleLevel(src, samp, in.uv, 0.0);
}

@fragment
fn fs_blit(in: VsOut) -> @location(0) vec4f {
  return vec4f(textureSampleLevel(src, samp, in.uv, 0.0).rgb, 1.0);
}

// For sRGB-encoded targets: the frame is already gamma encoded, so undo that
// before the target encodes it again.
@fragment
fn fs_blit_linear(in: VsOut) -> @location(0) vec4f {
  let c = textureSampleLevel(src, samp, in.uv, 0.0).rgb;
  return vec4f(pow(c, vec3f(2.2)), 1.0);
}
