Texture2D<float4> sourceTexture : register(t0);
SamplerState sourceSampler : register(s0);

cbuffer VisibilityConstants : register(b0)
{
    float exposure;
    float shadowLift;
    float gammaValue;
    float highlightKnee;
    float saturationValue;
    float detailGain;
    float sceneLuma;
    float forceBright;
    float2 texelSize;
    float2 padding;
};

struct VertexOutput
{
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
};

VertexOutput VSMain(uint vertexId : SV_VertexID)
{
    VertexOutput output;
    float2 uv = float2((vertexId << 1) & 2, vertexId & 2);
    output.uv = uv;
    output.position = float4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    return output;
}

float compressHighlight(float value)
{
    if (value <= highlightKnee)
    {
        return value;
    }
    float room = max(1.0 - highlightKnee, 0.001);
    return highlightKnee + room * (1.0 - exp(-(value - highlightKnee) / room));
}

float3 transformVisibility(float3 color, float3 localAverage)
{
    color = saturate(color);
    localAverage = saturate(localAverage);
    float sourceLuma = dot(color, float3(0.2126, 0.7152, 0.0722));
    float shadowWeight = 1.0 - smoothstep(0.30, 0.88, sourceLuma);
    float autoAmount = saturate((0.45 - sceneLuma) * 3.0);
    float activeAmount = lerp(autoAmount, 1.0, saturate(forceBright));
    float activeExposure = lerp(1.0, exposure, activeAmount);
    float activeLift = shadowLift * activeAmount;
    float activeDetail = detailGain * activeAmount;

    float3 exposed = saturate(color * activeExposure);
    float3 curved = pow(max(exposed, 0.000001), gammaValue);
    float3 lifted = curved + activeLift * (1.0 - curved) * shadowWeight;
    float3 compressed = float3(
        compressHighlight(lifted.r),
        compressHighlight(lifted.g),
        compressHighlight(lifted.b)
    );
    float mappedLuma = dot(compressed, float3(0.2126, 0.7152, 0.0722));
    float3 saturatedColor = mappedLuma + (compressed - mappedLuma) * saturationValue;
    float3 detailed = saturatedColor + (color - localAverage) * activeDetail * shadowWeight;
    return saturate(detailed);
}

float4 PSMain(VertexOutput input) : SV_Target
{
    float4 center = sourceTexture.Sample(sourceSampler, input.uv);
    float3 north = sourceTexture.Sample(sourceSampler, input.uv + float2(0.0, -texelSize.y)).rgb;
    float3 south = sourceTexture.Sample(sourceSampler, input.uv + float2(0.0, texelSize.y)).rgb;
    float3 east = sourceTexture.Sample(sourceSampler, input.uv + float2(texelSize.x, 0.0)).rgb;
    float3 west = sourceTexture.Sample(sourceSampler, input.uv + float2(-texelSize.x, 0.0)).rgb;
    float3 localAverage = (north + south + east + west) * 0.25;
    return float4(transformVisibility(center.rgb, localAverage), center.a);
}

float LumaAt(float2 uv)
{
    return dot(sourceTexture.SampleLevel(sourceSampler, uv, 0).rgb, float3(0.2126, 0.7152, 0.0722));
}

float LumaPS(VertexOutput input) : SV_Target
{
    // A fixed 4x4 grid gives a deterministic GPU aggregate without copying the
    // captured frame to CPU memory. Only the resulting one-float summary is read.
    float sum = 0.0;
    sum += LumaAt(float2(0.125, 0.125));
    sum += LumaAt(float2(0.375, 0.125));
    sum += LumaAt(float2(0.625, 0.125));
    sum += LumaAt(float2(0.875, 0.125));
    sum += LumaAt(float2(0.125, 0.375));
    sum += LumaAt(float2(0.375, 0.375));
    sum += LumaAt(float2(0.625, 0.375));
    sum += LumaAt(float2(0.875, 0.375));
    sum += LumaAt(float2(0.125, 0.625));
    sum += LumaAt(float2(0.375, 0.625));
    sum += LumaAt(float2(0.625, 0.625));
    sum += LumaAt(float2(0.875, 0.625));
    sum += LumaAt(float2(0.125, 0.875));
    sum += LumaAt(float2(0.375, 0.875));
    sum += LumaAt(float2(0.625, 0.875));
    sum += LumaAt(float2(0.875, 0.875));
    return saturate(sum / 16.0);
}
