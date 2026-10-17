#include <metal_stdlib>
using namespace metal;

constant uint SHAPE_ARC = 0;
constant uint SHAPE_RECT = 1;
constant float TAU = 6.28318530718;
constant float DASH = 3.0;

struct FrameUniforms {
    float2 viewport;
    float scale;
    float progress;
};

struct ShapeInstance {
    float4 geometry;
    float4 previous;
    float4 fill;
    float4 stroke;
    float2 center;
    float corner;
    float stroke_width;
    uint kind;
    uint dashed;
    uint2 padding;
};

struct GlyphInstance {
    float4 rect;
    float4 texture;
    float4 color;
};

struct ShapeVarying {
    float4 position [[position]];
    float2 point;
    float4 geometry [[flat]];
    uint instance [[flat]];
};

struct GlyphVarying {
    float4 position [[position]];
    float2 texture;
    float4 color [[flat]];
};

static float2 corner_of(uint vertex_id) {
    return float2(float(vertex_id & 1u), float(vertex_id >> 1u));
}

static float2 polar(float2 center, float radius, float angle) {
    return center + radius * float2(sin(angle), -cos(angle));
}

static void include(thread float2 &low, thread float2 &high, float2 point) {
    low = min(low, point);
    high = max(high, point);
}

static float4 to_clip(float2 point, constant FrameUniforms &frame) {
    float2 unit = point / frame.viewport;
    return float4(unit.x * 2.0 - 1.0, 1.0 - unit.y * 2.0, 0.0, 1.0);
}

vertex ShapeVarying shape_vertex(uint vertex_id [[vertex_id]],
                                 uint instance_id [[instance_id]],
                                 constant ShapeInstance *shapes [[buffer(0)]],
                                 constant FrameUniforms &frame [[buffer(1)]]) {
    ShapeInstance shape = shapes[instance_id];
    float4 geometry = mix(shape.previous, shape.geometry, frame.progress);
    float2 low;
    float2 high;
    if (shape.kind == SHAPE_RECT) {
        low = geometry.xy - 1.0;
        high = geometry.xy + geometry.zw + 1.0;
    } else if (shape.kind == SHAPE_ARC) {
        low = polar(shape.center, geometry.z, geometry.x);
        high = low;
        include(low, high, polar(shape.center, geometry.w, geometry.x));
        include(low, high, polar(shape.center, geometry.z, geometry.y));
        include(low, high, polar(shape.center, geometry.w, geometry.y));
        for (uint quarter = 0u; quarter <= 4u; quarter++) {
            float axis = float(quarter) * TAU * 0.25;
            if (axis >= geometry.x && axis <= geometry.y) {
                include(low, high, polar(shape.center, geometry.w, axis));
            }
        }
        low -= 1.0;
        high += 1.0;
    } else {
        low = shape.center - geometry.w - 1.0;
        high = shape.center + geometry.w + 1.0;
    }
    float2 point = mix(low, high, corner_of(vertex_id));
    ShapeVarying out;
    out.position = to_clip(point, frame);
    out.point = point;
    out.geometry = geometry;
    out.instance = instance_id;
    return out;
}

static float rounded_rect_distance(float2 point, float4 rect, float corner) {
    float2 half_size = rect.zw * 0.5;
    float2 q = abs(point - (rect.xy + half_size)) - (half_size - corner);
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - corner;
}

static float arc_distance(float2 offset, float4 geometry, uint kind) {
    float radius = length(offset);
    if (kind != SHAPE_ARC) {
        return radius - geometry.w;
    }
    float angle = atan2(offset.x, -offset.y);
    angle = angle < 0.0 ? angle + TAU : angle;
    float radial = max(geometry.z - radius, radius - geometry.w);
    float angular = max(geometry.x - angle, angle - geometry.y) * radius;
    return max(radial, angular);
}

fragment float4 shape_fragment(ShapeVarying in [[stage_in]],
                               constant ShapeInstance *shapes [[buffer(0)]],
                               constant FrameUniforms &frame [[buffer(1)]]) {
    ShapeInstance shape = shapes[in.instance];
    float distance;
    float dash_parameter;
    if (shape.kind == SHAPE_RECT) {
        distance = rounded_rect_distance(in.point, in.geometry, shape.corner);
        dash_parameter = in.point.x + in.point.y;
    } else {
        float2 offset = in.point - shape.center;
        distance = arc_distance(offset, in.geometry, shape.kind);
        dash_parameter = atan2(offset.x, -offset.y) * length(offset) + length(offset);
    }
    float coverage = saturate(0.5 - distance * frame.scale);
    float4 color = shape.fill * coverage;
    if (shape.stroke.a > 0.0) {
        float band = abs(distance + shape.stroke_width * 0.5) - shape.stroke_width * 0.5;
        float stroke_coverage = saturate(0.5 - band * frame.scale) * coverage;
        if (shape.dashed != 0u && fmod(abs(dash_parameter), 2.0 * DASH) >= DASH) {
            stroke_coverage = 0.0;
        }
        color = shape.stroke * stroke_coverage + color * (1.0 - shape.stroke.a * stroke_coverage);
    }
    return color;
}

vertex GlyphVarying glyph_vertex(uint vertex_id [[vertex_id]],
                                 uint instance_id [[instance_id]],
                                 constant GlyphInstance *glyphs [[buffer(0)]],
                                 constant FrameUniforms &frame [[buffer(1)]]) {
    GlyphInstance glyph = glyphs[instance_id];
    float2 corner = corner_of(vertex_id);
    GlyphVarying out;
    out.position = to_clip(glyph.rect.xy + glyph.rect.zw * corner, frame);
    out.texture = mix(glyph.texture.xy, glyph.texture.zw, corner);
    out.color = glyph.color;
    return out;
}

fragment float4 glyph_fragment(GlyphVarying in [[stage_in]], texture2d<float> atlas [[texture(0)]]) {
    constexpr sampler nearest(filter::nearest, address::clamp_to_edge);
    float coverage = atlas.sample(nearest, in.texture).r;
    return in.color * coverage;
}
