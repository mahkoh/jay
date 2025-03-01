#ifndef FRAG_SPEC_CONST_GLSL
#define FRAG_SPEC_CONST_GLSL

layout(constant_id = 0) const bool src_has_alpha = false;
layout(constant_id = 1) const bool has_alpha_multiplier = false;
layout(constant_id = 2) const uint eotf = 0;
layout(constant_id = 3) const uint oetf = 0;
layout(constant_id = 4) const bool has_matrix = false;

#endif
