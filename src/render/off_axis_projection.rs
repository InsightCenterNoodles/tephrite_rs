use bevy::math::{DMat4, DQuat, DVec3, Mat4, Vec4};

pub(crate) struct ScreenDesc {
    pub lower_left: DVec3,  // lower left corner of the screen in world space
    pub lower_right: DVec3, // lower right corner of the screen in world space
    pub upper_right: DVec3, // upper right corner of the screen in world space
}

#[allow(non_snake_case)]
pub(crate) fn compute_off_axis_projection(
    screen_desc: &ScreenDesc,
    head_pos: DVec3,
    head_rot: DQuat,
    left_eye: bool,
    near: f64,
    far: f64,
    camera_tf: &mut Mat4,
    camera_proj: &mut Mat4,
) {
    assert!(near > 0.0 && far > near);

    // 1) Head pose and stereo offset (world space)
    let H = DMat4::from_translation(head_pos);

    let ipd = 0.064f64;
    let half = if left_eye { 0.5 } else { -0.5 } * ipd;
    let rCam = head_rot.mul_vec3(H.x_axis.truncate().normalize()); // mat4f::project(mat4f(head_rot),normalize(float3 { H[0].x, H[0].y, H[0].z }));

    let H_eye = DMat4::from_translation(half * rCam) * H;
    let eyeW = H_eye.w_axis.truncate(); // float3 { H_eye[3].x, H_eye[3].y, H_eye[3].z };

    // 2) Screen plane in world
    let LL: DVec3 = screen_desc.lower_left;
    let LR: DVec3 = screen_desc.lower_right;
    let UR: DVec3 = screen_desc.upper_right;
    let UL = LL + (UR - LR); // planar

    let vr: DVec3 = (LR - LL).normalize(); // screen right
    let vu: DVec3 = (UL - LL).normalize(); // screen up
    let mut vn: DVec3 = vr.cross(vu).normalize(); // screen normal (points out of screen)

    // Ensure normal faces the eye
    if vn.dot(eyeW - LL) < 0.0 {
        vn = -vn;
    }

    // 3) Build a VIEW whose axes align to the screen
    let camX: DVec3 = -vr;
    let camY: DVec3 = -vu;
    let camZ: DVec3 = vn;

    // Column-major: columns are basis vectors and translation
    let V_worldFromEye = DMat4 {
        x_axis: camX.extend(0.0),
        y_axis: camY.extend(0.0),
        z_axis: camZ.extend(0.0),
        w_axis: eyeW.extend(1.0),
    };

    // Camera transform in world space (world_from_eye)
    *camera_tf = V_worldFromEye.as_mat4();
    //camera->setModelMatrix(V_worldFromEye);

    // 4) Compute asymmetric frustum in this eye space
    let va = LL - eyeW;
    let vb = LR - eyeW;
    let vc = UL - eyeW;

    let d = va.dot(vn); // distance along normal (>0)
    // Project to near plane using the screen basis
    let l = vr.dot(va) * near / d;
    let r = vr.dot(vb) * near / d;
    let b = vu.dot(va) * near / d;
    let t = vu.dot(vc) * near / d;

    // 5) Use a plain asymmetric projection (no extra S)
    *camera_proj = frustum_reverse_rh_bevy(
        l as f32,
        r as f32,
        b as f32,
        t as f32,
        near as f32,
        far as f32,
    );
    //camera->setCustomProjection(mat4(P), near, far);
}

#[inline]
pub(crate) fn frustum_reverse_rh_bevy(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    z_near: f32,
    z_far: f32,
) -> Mat4 {
    debug_assert!(z_near > 0.0);
    debug_assert!(z_far > z_near);

    let x = (2.0 * z_near) / (right - left);
    let y = (2.0 * z_near) / (top - bottom);

    let a = (right + left) / (right - left);
    let b = (top + bottom) / (top - bottom);

    // Reverse-Z, RH, depth in [0, 1]:
    // near -> 1, far -> 0
    let c = z_near / (z_far - z_near);
    let d = (z_far * z_near) / (z_far - z_near);

    Mat4::from_cols(
        Vec4::new(x, 0.0, 0.0, 0.0),
        Vec4::new(0.0, y, 0.0, 0.0),
        Vec4::new(a, b, c, -1.0),
        Vec4::new(0.0, 0.0, d, 0.0),
    )
}
