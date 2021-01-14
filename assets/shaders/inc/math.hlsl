#ifndef MATH_HLSL
#define MATH_HLSL

#define M_PI 3.14159265358979323846264338327950288
#define M_TAU 6.28318530717958647692528676655900577
#define M_FRAC_PI_2 1.57079632679489661923132169163975144
#define M_FRAC_PI_3 1.04719755119659774615421446109316763
#define M_FRAC_PI_4 0.785398163397448309615660845819875721
#define M_FRAC_PI_6 0.52359877559829887307710723054658381
#define M_FRAC_PI_8 0.39269908169872415480783042290993786
#define M_FRAC_1_PI 0.318309886183790671537767526745028724
#define M_FRAC_2_PI 0.636619772367581343075535053490057448
#define M_FRAC_2_SQRT_PI 1.12837916709551257389615890312154517
#define M_SQRT_2 1.41421356237309504880168872420969808
#define M_FRAC_1_SQRT_2 0.707106781186547524400844362104849039
#define M_E 2.71828182845904523536028747135266250
#define M_LOG2_E 1.44269504088896340735992468100189214
#define M_LOG2_10 3.32192809488736234787031942948939018
#define M_LOG10_E 0.434294481903251827651128918916605082
#define M_LOG10_2 0.301029995663981195213738894724493027
#define M_LN_2 0.693147180559945309417232121458176568
#define M_LN_10 2.30258509299404568401799145468436421
#define M_PHI 1.61803398874989484820

// Building an Orthonormal Basis, Revisited
// http://jcgt.org/published/0006/01/01/
float3x3 build_orthonormal_basis(float3 n) {
    float3 b1;
    float3 b2;

    if (n.z < 0.0) {
        const float a = 1.0 / (1.0 - n.z);
        const float b = n.x * n.y * a;
        b1 = float3(1.0 - n.x * n.x * a, -b, n.x);
        b2 = float3(b, n.y * n.y * a - 1.0, -n.y);
    } else {
        const float a = 1.0 / (1.0 + n.z);
        const float b = -n.x * n.y * a;
        b1 = float3(1.0 - n.x * n.x * a, b, -n.x);
        b2 = float3(b, 1.0 - n.y * n.y * a, -n.y);
    }

    return float3x3(
        b1.x, b2.x, n.x,
        b1.y, b2.y, n.y,
        b1.z, b2.z, n.z
    );
}

#endif
