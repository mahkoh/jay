#ifdef EXTERNAL
#extension GL_OES_EGL_image_external : require
#endif

precision mediump float;
varying vec2 v_texcoord;
#ifdef EXTERNAL
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif
#ifdef ALPHA_MULTIPLIER
uniform float alpha;
#endif

void main() {
#ifdef ALPHA

#ifdef ALPHA_MULTIPLIER
	gl_FragColor = texture2D(tex, v_texcoord) * alpha;
#else // !ALPHA_MULTIPLIER
	gl_FragColor = texture2D(tex, v_texcoord);
#endif // ALPHA_MULTIPLIER

#else // !ALPHA

#ifdef ALPHA_MULTIPLIER
	gl_FragColor = vec4(texture2D(tex, v_texcoord).rgb * alpha, alpha);
#else // !ALPHA_MULTIPLIER
	gl_FragColor = vec4(texture2D(tex, v_texcoord).rgb, 1.0);
#endif // ALPHA_MULTIPLIER

#endif // ALPHA
}
