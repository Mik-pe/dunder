#version 450
layout(location = 0) in vec4 position;
layout(location = 1) in vec4 color;
layout(location = 0) out vec4 vertex_color;
void main() {
    // Public geometry uses +Y upwards, like the Metal implementation.
    gl_Position = vec4(position.x, -position.y, position.z, position.w);
    vertex_color = color;
}
