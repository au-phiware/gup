struct ParamsX_naga_oil_mod_XM52XAOR2ONRWC3DFHI5GY2LOMVQXEX {
    k: f32,
    range_start: f32,
}

struct ParamsX_naga_oil_mod_XM52XAOR2ONRWC3DFHI5GY33HX {
    log_lo: f32,
    inv_log_span: f32,
    range_start: f32,
    r_span: f32,
}

struct ParamsX_naga_oil_mod_XM52XAOR2MNXWY33SHI5HGZLROVSW45DJMFWAX {
    lo: f32,
    inv_span: f32,
    reverse: u32,
}

struct ViewX_naga_oil_mod_XM52XAOR2OZUWK5YX {
    size: vec2<f32>,
    dpr: f32,
    padding: f32,
}

struct CircleInX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX {
    x: f32,
    y: f32,
    radius: f32,
    fill: vec4<f32>,
}

struct VaryingsX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX {
    @builtin(position) clip: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) radius: f32,
    @location(2) fill: vec4<f32>,
    @location(3) @interpolate(flat) row: u32,
}

struct Encodings {
    x: ParamsX_naga_oil_mod_XM52XAOR2ONRWC3DFHI5GY2LOMVQXEX,
    y: ParamsX_naga_oil_mod_XM52XAOR2ONRWC3DFHI5GY33HX,
    radius: f32,
    fill: ParamsX_naga_oil_mod_XM52XAOR2MNXWY33SHI5HGZLROVSW45DJMFWAX,
}

struct Chunk {
    row_base: u32,
    x_base: f32,
}

struct Columns {
    @location(0) x: f32,
    @location(1) y: f32,
    @location(2) fill: f32,
}

@group(0) @binding(0) 
var<uniform> u_view: ViewX_naga_oil_mod_XM52XAOR2OZUWK5YX;
@group(1) @binding(0) 
var<uniform> enc: Encodings;
@group(1) @binding(1) 
var fill_lut: texture_2d<f32>;
@group(1) @binding(2) 
var fill_smp: sampler;
@group(2) @binding(0) 
var<uniform> chunk: Chunk;

fn map_relX_naga_oil_mod_XM52XAOR2ONRWC3DFHI5GY2LOMVQXEX(v_1: f32, base: f32, p: ParamsX_naga_oil_mod_XM52XAOR2ONRWC3DFHI5GY2LOMVQXEX) -> f32 {
    return (p.range_start + ((v_1 + base) * p.k));
}

fn mapX_naga_oil_mod_XM52XAOR2ONRWC3DFHI5GY33HX(v_2: f32, p_1: ParamsX_naga_oil_mod_XM52XAOR2ONRWC3DFHI5GY33HX) -> f32 {
    return (p_1.range_start + (((log2(v_2) - p_1.log_lo) * p_1.inv_log_span) * p_1.r_span));
}

fn mapX_naga_oil_mod_XM52XAOR2MNXWY33SHI5HGZLROVSW45DJMFWAX(v_3: f32, p_2: ParamsX_naga_oil_mod_XM52XAOR2MNXWY33SHI5HGZLROVSW45DJMFWAX, lut: texture_2d<f32>, smp: sampler) -> vec4<f32> {
    var t: f32;

    t = clamp(((v_3 - p_2.lo) * p_2.inv_span), 0f, 1f);
    if (p_2.reverse != 0u) {
        let _e13 = t;
        t = (1f - _e13);
    }
    let _e17 = textureDimensions(lut);
    let n = f32(_e17.x);
    let _e20 = t;
    let u = (((_e20 * (n - 1f)) + 0.5f) / n);
    let _e31 = textureSampleLevel(lut, smp, vec2<f32>(u, 0.5f), 0f);
    return _e31;
}

fn px_to_clipX_naga_oil_mod_XM52XAOR2OZUWK5YX(p_3: vec2<f32>, view: ViewX_naga_oil_mod_XM52XAOR2OZUWK5YX) -> vec4<f32> {
    let ndc = vec2<f32>((((p_3.x / view.size.x) * 2f) - 1f), (1f - ((p_3.y / view.size.y) * 2f)));
    return vec4<f32>(ndc, 0f, 1f);
}

fn cornerX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX(vertex_index_1: u32) -> vec2<f32> {
    var corners: array<vec2<f32>, 6> = array<vec2<f32>, 6>(vec2<f32>(-1f, -1f), vec2<f32>(1f, -1f), vec2<f32>(-1f, 1f), vec2<f32>(-1f, 1f), vec2<f32>(1f, -1f), vec2<f32>(1f, 1f));

    let _e10 = corners[vertex_index_1];
    return _e10;
}

fn vertexX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX(m_1: CircleInX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX, vertex_index_2: u32, row: u32, view_1: ViewX_naga_oil_mod_XM52XAOR2OZUWK5YX) -> VaryingsX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX {
    var out: VaryingsX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX;

    let half = (m_1.radius + (1f / view_1.dpr));
    let _e8 = cornerX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX(vertex_index_2);
    let offset = (_e8 * half);
    let _e16 = px_to_clipX_naga_oil_mod_XM52XAOR2OZUWK5YX((vec2<f32>(m_1.x, m_1.y) + offset), view_1);
    out.clip = _e16;
    out.local = (offset * view_1.dpr);
    out.radius = (m_1.radius * view_1.dpr);
    out.fill = m_1.fill;
    out.row = row;
    let _e28 = out;
    return _e28;
}

fn shadeX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX(v_4: VaryingsX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX) -> vec4<f32> {
    let distance = (length(v_4.local) - v_4.radius);
    let coverage = clamp((0.5f - distance), 0f, 1f);
    let alpha = (v_4.fill.w * coverage);
    if (alpha <= 0f) {
        discard;
    }
    return vec4<f32>((v_4.fill.xyz * alpha), alpha);
}

@vertex 
fn vs_main(@builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32, col: Columns) -> VaryingsX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX {
    var m: CircleInX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX;

    let _e6 = chunk.x_base;
    let _e9 = enc.x;
    let _e10 = map_relX_naga_oil_mod_XM52XAOR2ONRWC3DFHI5GY2LOMVQXEX(col.x, _e6, _e9);
    m.x = _e10;
    let _e15 = enc.y;
    let _e16 = mapX_naga_oil_mod_XM52XAOR2ONRWC3DFHI5GY33HX(col.y, _e15);
    m.y = _e16;
    let _e20 = enc.radius;
    m.radius = _e20;
    let _e25 = enc.fill;
    let _e28 = mapX_naga_oil_mod_XM52XAOR2MNXWY33SHI5HGZLROVSW45DJMFWAX(col.fill, _e25, fill_lut, fill_smp);
    m.fill = _e28;
    let _e29 = m;
    let _e33 = chunk.row_base;
    let _e36 = u_view;
    let _e38 = vertexX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX(_e29, vertex_index, (_e33 + instance_index), _e36);
    return _e38;
}

@fragment 
fn fs_main(v: VaryingsX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX) -> @location(0) vec4<f32> {
    let _e1 = shadeX_naga_oil_mod_XM52XAOR2NVQXE23THI5GG2LSMNWGKX(v);
    return _e1;
}
