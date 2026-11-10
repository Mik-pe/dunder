#[path = "support/graphics.rs"]
mod graphics;

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use dunder::target::Target;

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dunder-vulkan-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        graphics::install(&path);
        Self(path)
    }
    fn checked(&self, source: &str) -> dunder::modules::Checked {
        fs::write(self.0.join("main.ddr"), source).unwrap();
        dunder::modules::check_path(self.0.join("main.ddr"))
            .unwrap_or_else(|error| panic!("{}", error.render()))
    }
    fn compile(&self, source: &str, target: Target) -> Vec<u8> {
        let checked = self.checked(source);
        checked
            .compile_for(target)
            .unwrap_or_else(|error| panic!("{}", checked.render(&error)))
    }
    fn rejects(&self, source: &str, code: &str) {
        fs::write(self.0.join("main.ddr"), source).unwrap();
        let error = dunder::modules::check_path(self.0.join("main.ddr"))
            .err()
            .unwrap_or_else(|| panic!("incorrectly accepted {source}"));
        assert_eq!(error.error.code, code, "{}", error.render());
    }

    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    fn execute(&self, source: &str) -> std::process::Output {
        self.execute_with(source, &[])
    }

    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    fn execute_with(&self, source: &str, environment: &[(&str, &str)]) -> std::process::Output {
        self.execute_status(source, environment, 0, 45)
    }

    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    fn execute_status(
        &self,
        source: &str,
        environment: &[(&str, &str)],
        expected_status: i32,
        timeout_seconds: u64,
    ) -> std::process::Output {
        use std::{
            os::unix::fs::PermissionsExt,
            process::{Command, Stdio},
            thread,
            time::{Duration, Instant},
        };
        let binary = self.0.join("program");
        fs::write(&binary, self.compile(source, Target::host().unwrap())).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        let mut command = Command::new(binary);
        command
            .current_dir(&self.0)
            .env_clear()
            .env("PATH", "/no-external-tools")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for name in [
            "VK_ICD_FILENAMES",
            "VK_DRIVER_FILES",
            "VK_LAYER_PATH",
            "VK_INSTANCE_LAYERS",
            "LD_LIBRARY_PATH",
            "XDG_RUNTIME_DIR",
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "SDL_VIDEODRIVER",
        ] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        for (name, value) in environment {
            command.env(name, value);
        }
        let mut child = command.spawn().unwrap();
        let started = Instant::now();
        while child.try_wait().unwrap().is_none() {
            if started.elapsed() > Duration::from_secs(timeout_seconds) {
                let _ = child.kill();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "Vulkan program timed out: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            thread::sleep(Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.code() == Some(expected_status),
            "{:?}\nstdout: {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("VUID-") && !stderr.contains("Validation Error"),
            "{stderr}"
        );
        output
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn elf_segments(image: &[u8]) -> Vec<(u32, u32, &[u8])> {
    assert_eq!(&image[..4], b"\x7fELF");
    let u16_at = |offset| u16::from_le_bytes(image[offset..offset + 2].try_into().unwrap());
    let u32_at = |offset| u32::from_le_bytes(image[offset..offset + 4].try_into().unwrap());
    let u64_at = |offset| u64::from_le_bytes(image[offset..offset + 8].try_into().unwrap());
    let table = u64_at(32) as usize;
    let stride = u16_at(54) as usize;
    let count = u16_at(56) as usize;
    assert!(stride >= 56 && table + count * stride <= image.len());
    (0..count)
        .map(|i| {
            let at = table + i * stride;
            let offset = u64_at(at + 8) as usize;
            let size = u64_at(at + 32) as usize;
            assert!(offset <= image.len() && size <= image.len() - offset);
            (u32_at(at), u32_at(at + 4), &image[offset..offset + size])
        })
        .collect()
}

#[test]
fn vulkan_types_are_nominal_and_legacy_intrinsics_are_unavailable() {
    let project = Project::new();
    fs::write(project.0.join("vulkan.ddr"), "pub fn answer()->i32{42}").unwrap();
    let checked = project.checked(r#"
import graphics.vulkan as vk
import graphics.vulkan as other
import .vulkan as local
fn main(){color:other.Color=vk.rgb(1.0,0.0,0.0);window:other.Window=vk.Window{id:0};print(color.alpha==1.0);print(window.id);print(local.answer())}
"#);
    assert_eq!(
        checked
            .sources
            .files
            .iter()
            .filter(|s| s.path
                == project
                    .0
                    .join("graphics/vulkan.ddr")
                    .canonicalize()
                    .unwrap())
            .count(),
        1
    );
    for (source, code) in [
        ("import graphics.vulkan as vk\nfn main(){r:=vk.open_headless(1,true)}", "E0201"),
        ("import graphics.vulkan as vk\nfn main(){r:=vk.open_headless(1)}", "E0207"),
        ("import graphics.vulkan as vk\nfn main(){r:=vk.poll(0)}", "E0201"),
        ("import graphics.vulkan as vk\nfn main(){w:=vk.Window{id:0};var v:[]f64=[];r:=vk.draw(w,&v,0.0,0.0,0.0)}", "E0201"),
        ("import graphics.vulkan as vk\nfn main(){r:i32=vk.close(vk.Window{id:0})}", "E0201"),
        ("fn main(){__vulkan_open_headless(16,16)}", "E0205"),
        ("import graphics.vulkan as vk\nfn main(){__vulkan_pixels(vk.Window{id:0})}", "E0205"),
    ] { project.rejects(source, code); }
    fs::write(
        project.0.join("vulkan.ddr"),
        "pub fn fake(){__vulkan_open_headless(16,16)}",
    )
    .unwrap();
    project.rejects("import .vulkan as local\nfn main(){local.fake()}", "E0205");
}

#[test]
fn geometry_and_unreachable_vulkan_calls_remain_portable_and_static() {
    let project = Project::new();
    let geometry = r#"import graphics.vulkan as vk
fn main(){var v:[]f32=[];vk.quad(&mut v,-1.0,-1.0,2.0,2.0,vk.rgb(1.0,0.0,0.0));print(v.len)}"#;
    for target in Target::NATIVE {
        let geometry_image = project.compile(geometry, target);
        let plain = project.compile("fn main(){print(42)}", target);
        let unused = project.compile("import graphics.vulkan as vk\nfn unused(){r:=vk.open_headless(16,16)}\nfn main(){print(42)}", target);
        assert_eq!(unused, plain, "unreachable Vulkan changed {target}");
        if target != Target::Arm64Macos {
            assert!(!elf_segments(&geometry_image)
                .iter()
                .any(|(kind, _, _)| *kind == 2 || *kind == 3));
        }
    }
}

#[test]
fn linux_vulkan_images_resolve_native_imports_without_a_build_toolchain() {
    let project = Project::new();
    let source =
        "import graphics.vulkan as vk\nfn main(){r:=vk.open_headless(16,16);vk.close(r.window)}";
    for (target, machine, interpreter) in [
        (Target::X86_64Linux, 62, "/lib64/ld-linux-x86-64.so.2"),
        (Target::Arm64Linux, 183, "/lib/ld-linux-aarch64.so.1"),
    ] {
        let image = project.compile(source, target);
        assert_eq!(
            u16::from_le_bytes(image[18..20].try_into().unwrap()),
            machine
        );
        let segments = elf_segments(&image);
        let loader = segments.iter().find(|(kind, _, _)| *kind == 3).unwrap().2;
        assert_eq!(loader.strip_suffix(&[0]).unwrap(), interpreter.as_bytes());
        assert!(segments.iter().any(|(kind, _, _)| *kind == 2));
        assert!(segments
            .iter()
            .any(|(kind, flags, _)| *kind == 0x6474e551 && *flags == 6));
        assert!(
            !segments
                .iter()
                .any(|(kind, flags, _)| *kind == 1 && *flags & 3 == 3),
            "writable executable segment"
        );
        assert_eq!(
            image,
            project.compile(source, target),
            "nondeterministic image"
        );
        let output = project.0.join(target.name());
        let built = std::process::Command::new(env!("CARGO_BIN_EXE_dunder"))
            .args(["build", "main.ddr", "--target", target.name(), "-o"])
            .arg(&output)
            .current_dir(&project.0)
            .env("PATH", "/no-external-tools")
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        assert_eq!(fs::read(output).unwrap(), image);
    }
    let checked = project.checked(source);
    let error = checked.compile_for(Target::Arm64Macos).unwrap_err();
    assert!(checked.render(&error).contains("Linux"));
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn native_requested() -> bool {
    if std::env::var_os("DUNDER_VULKAN_TEST").is_some_and(|value| value == "1") {
        true
    } else {
        eprintln!("Vulkan execution not requested; set DUNDER_VULKAN_TEST=1 with a Vulkan ICD (lavapipe is supported)");
        false
    }
}

#[test]
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn missing_vulkan_driver_is_a_recoverable_error_without_publishing_a_handle() {
    let output = Project::new().execute_with(
        r#"import graphics.vulkan as vk
fn main(){
    for i in 0..2 {
        result:=vk.open_headless(16,16)
        assert(!result.ok&&result.error.len>0)
        assert(vk.poll(result.window).closed)
        assert(vk.pixels(result.window).len==0)
        vk.close(result.window)
    }
    print("missing driver handled")
}"#,
        &[
            ("VK_ICD_FILENAMES", "/dunder-no-such-icd.json"),
            ("VK_DRIVER_FILES", "/dunder-no-such-icd.json"),
            ("VK_INSTANCE_LAYERS", ""),
        ],
    );
    assert_eq!(output.stdout, b"missing driver handled\n");
}

#[test]
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn lavapipe_pixels_prove_clear_geometry_orientation_interpolation_and_alpha() {
    if !native_requested() {
        return;
    }
    let output = Project::new().execute(r#"
import graphics.vulkan as vk
fn main(){
    opened:=vk.open_headless(64,48)
    if !opened.ok {print(opened.error);assert(false)}
    window:=opened.window
    name:=vk.device_name(window);assert(name.len>0);print(name)
    info:=vk.poll(window);assert(!info.closed);assert(info.width==64&&info.height==48);assert(info.pixel_width==64&&info.pixel_height==48);assert(info.scale==1.0)
    var vertices:[]f32=[]
    first:=vk.draw(window,&vertices,0.0,0.0,1.0);assert(first.ok&&first.frames==1)
    before:=vk.pixels(window);assert(before.len==64*48*4)
    for i in 0..64*48 {assert(before[i*4]==0&&before[i*4+1]==0&&before[i*4+2]==255&&before[i*4+3]==255)}
    vk.quad(&mut vertices,-1.0,0.0,2.0,1.0,vk.rgba(1.0,0.0,0.0,0.5))
    second:=vk.draw(window,&vertices,0.0,0.0,1.0);assert(second.ok&&second.frames==2);assert(vertices.len==48&&vertices[0]==-1.0)
    var rgba:=vk.pixels(window)
    top:=(8*64+32)*4;bottom:=(40*64+32)*4
    assert(rgba[top]>=127&&rgba[top]<=128);assert(rgba[top+1]==0);assert(rgba[top+2]>=127&&rgba[top+2]<=128)
    assert(rgba[top+3]==255)
    assert(rgba[bottom]==0&&rgba[bottom+1]==0&&rgba[bottom+2]==255)
    assert(before[top]==0&&before[top+2]==255)
    rgba[bottom]=17;fresh:=vk.pixels(window);assert(fresh[bottom]==0)
    vertices.clear()
    vk.gradient_triangle(&mut vertices,vk.Point{x:-1.0,y:-1.0},vk.Point{x:1.0,y:-1.0},vk.Point{x:0.0,y:1.0},vk.rgb(1.0,0.0,0.0),vk.rgb(0.0,1.0,0.0),vk.rgb(0.0,0.0,1.0))
    third:=vk.draw(window,&vertices,0.0,0.0,0.0);assert(third.ok&&third.frames==3)
    gradient:=vk.pixels(window);center:=(24*64+32)*4
    assert(gradient[center]>=50&&gradient[center]<=80);assert(gradient[center+1]>=50&&gradient[center+1]<=80);assert(gradient[center+2]>=110&&gradient[center+2]<=145)
    assert(gradient[0]==0&&gradient[1]==0&&gradient[2]==0)
    vk.close(window);print("pixels ok")
}
"#);
    assert!(String::from_utf8_lossy(&output.stdout).ends_with("\npixels ok\n"));
}

#[test]
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn lavapipe_lifecycle_rejects_invalid_handles_sizes_geometry_and_nonfinite_values() {
    if !native_requested() {
        return;
    }
    let output = Project::new().execute(r#"
import graphics.vulkan as vk
fn main(){
    invalid0:=vk.open_headless(0,16);assert(!invalid0.ok&&invalid0.error.len>0)
    invalid1:=vk.open_headless(16,-1);assert(!invalid1.ok)
    invalid2:=vk.open_headless(4097,16);assert(!invalid2.ok)
    var old:=vk.Window{id:0}
    var vertices:[]f32=[]
    zero:f32=0.0
    for cycle in 0..3 {
        opened:=vk.open_headless(32,24);if !opened.ok{print(opened.error);assert(false)}
        window:=opened.window;assert(window.id!=old.id)
        duplicate:=vk.open_headless(32,24);assert(!duplicate.ok)
        vk.close(old);assert(!vk.poll(window).closed)
        cleared:=vk.draw(window,&vertices,0.0,0.0,0.0);assert(cleared.ok&&cleared.frames==1)
        vertices.push(0.0);bad_length:=vk.draw(window,&vertices,0.0,0.0,0.0);assert(!bad_length.ok&&bad_length.error.len>0);vertices.clear()
        if cycle==0 {
            vertices.reserve(800016)
            for i in 0..800016 {vertices.push(0.0)}
            too_many:=vk.draw(window,&vertices,0.0,0.0,0.0);assert(!too_many.ok)
            vertices.clear()
        }
        vk.triangle(&mut vertices,vk.Point{x:-0.5,y:-0.5},vk.Point{x:0.5,y:-0.5},vk.Point{x:0.0,y:0.5},vk.rgb(1.0,0.0,0.0))
        vertices[0]=zero/zero;bad_nan:=vk.draw(window,&vertices,0.0,0.0,0.0);assert(!bad_nan.ok)
        vertices[0]=1.0/zero;bad_inf:=vk.draw(window,&vertices,0.0,0.0,0.0);assert(!bad_inf.ok)
        vertices.clear();zero64:f64=0.0
        bad_clear:=vk.draw(window,&vertices,zero64/zero64,0.0,0.0);assert(!bad_clear.ok)
        again:=vk.draw(window,&vertices,0.0,0.0,0.0);assert(again.ok&&again.frames==2)
        vk.close(window);vk.close(window);assert(vk.poll(window).closed);assert(vk.pixels(window).len==0);assert(vk.device_name(window)=="")
        stale:=vk.draw(window,&vertices,0.0,0.0,0.0);assert(!stale.ok)
        old=window
    }
    forged:=vk.Window{id:9223372036854775807};assert(vk.poll(forged).closed);vk.close(forged)
    print("lifecycle cycles=3")
}
"#);
    assert_eq!(output.stdout, b"lifecycle cycles=3\n");
}

#[test]
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn active_lavapipe_threads_exit_with_main_and_with_runtime_traps() {
    if !native_requested() {
        return;
    }
    for (ending, status) in [("return 7", 7), ("assert(false); 0", 104)] {
        let source = format!(
            r#"import graphics.vulkan as vk
fn main()->i32 {{
    opened:=vk.open_headless(32,24)
    if !opened.ok {{print(opened.error);return 1}}
    print("opened")
    {ending}
}}"#
        );
        // Intentionally leave the device open: a thread-only SYS_exit would leave
        // lavapipe worker threads alive and hang both cases until this deadline.
        let output = Project::new().execute_status(&source, &[], status, 5);
        assert_eq!(output.stdout, b"opened\n");
    }
}

#[test]
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn sdl_surface_and_events_round_trip_through_the_public_window_api() {
    if !std::env::var_os("DUNDER_SDL_TEST").is_some_and(|value| value == "1") {
        return;
    }
    let result = Project::new().execute_with(
        r#"capability unsafe
import graphics.vulkan as vk

@extern("libSDL2-2.0.so.0", "SDL_GetWindowFromID")
unsafe fn find_window(id: u32) -> ptr
@extern("libSDL2-2.0.so.0", "SDL_GetWindowSurface")
unsafe fn surface(window: ptr) -> ptr
@extern("libSDL2-2.0.so.0", "SDL_ConvertSurfaceFormat")
unsafe fn convert(surface: ptr, format: u32, flags: u32) -> ptr
@extern("libSDL2-2.0.so.0", "SDL_FreeSurface")
unsafe fn free_surface(surface: ptr)
@extern("libSDL2-2.0.so.0", "SDL_PushEvent")
unsafe fn push(event: ptr) -> i32

@c_layout
struct Surface { flags: u32, format: ptr, width: i32, height: i32, pitch: i32, pixels: ptr }
unsafe fn first_pixel(s: &Surface) -> u32 {
    assert(s.width == 64 && s.height == 48 && s.pitch >= s.width * 4)
    load_u32(s.pixels)
}

fn main() {
    r := vk.open("SDL pixel test", 64, 48)
    if !r.ok { print(r.error); assert(false) }
    w := r.window
    var vertices: []f32 = []
    assert(vk.draw(w, &vertices, 1.0, 0.0, 0.0).ok)
    unsafe {
        var id: u32 = 1
        while find_window(id) == ptr(0) && id < 256 { id = id + 1 }
        native := find_window(id)
        assert(native != ptr(0))
        // Convert the SDL window surface to ABGR8888 (RGBA bytes on Linux).
        pixels := convert(surface(native), 376840196, 0)
        assert(pixels != ptr(0))
        assert(first_pixel(ref_at(pixels)) == 4278190335)
        free_surface(pixels)

        var event: [u32; 14] = [0; 14]
        event[0] = 768
        event[2] = id
        event[5] = 99
        assert(push(addr(&event[0])) == 1)
        var seen := false
        for i in 0..16 {
            frame := vk.poll(w)
            if frame.key == 99 { seen = true; break }
        }
        assert(seen)
        event[0] = 256
        assert(push(addr(&event[0])) == 1)
        for i in 0..16 { if vk.poll(w).closed { break } }
        assert(vk.poll(w).closed)
        assert(!vk.draw(w, &vertices, 0.0, 0.0, 0.0).ok)
    }
    vk.close(w)
    print("SDL surface and events ok")
}
"#,
        &[("SDL_VIDEODRIVER", "dummy")],
    );
    assert_eq!(result.stdout, b"SDL surface and events ok\n");
}
