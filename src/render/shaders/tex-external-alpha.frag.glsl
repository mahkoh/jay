#extension GL_OES_EGL_image_external : require

precision mediump float;
varying vec2 v_texcoord;
uniform samplerExternalOES tex;

void main() {
	gl_FragColor = texture2D(tex, v_texcoord);
}
