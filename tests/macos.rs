#[path = "support/graphics.rs"]
mod graphics;

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dunder-macos-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        graphics::install(&path);
        Self(path)
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }

    fn checked(&self, source: &str) -> dunder::modules::Checked {
        self.write("main.ddr", source);
        dunder::modules::check_path(self.0.join("main.ddr"))
            .unwrap_or_else(|error| panic!("{}", error.render()))
    }

    fn compile(&self, source: &str) -> Vec<u8> {
        let checked = self.checked(source);
        checked
            .compile_for(dunder::target::Target::Arm64Macos)
            .unwrap_or_else(|error| panic!("{}", checked.render(&error)))
    }

    fn rejects(&self, source: &str, code: &str) {
        self.write("main.ddr", source);
        let error = dunder::modules::check_path(self.0.join("main.ddr"))
            .err()
            .unwrap_or_else(|| panic!("incorrectly accepted {source}"));
        assert_eq!(error.error.code, code, "{}", error.render());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn execute(&self, source: &str) -> std::process::Output {
        use std::{os::unix::fs::PermissionsExt, process::Command};
        let path = self.0.join("program");
        let temporary = self.0.join("program.new");
        fs::write(&temporary, self.compile(source)).unwrap();
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o755)).unwrap();
        fs::rename(temporary, &path).unwrap();
        Command::new(path)
            .current_dir(&self.0)
            .env_clear()
            .env("PATH", "/no-external-tools")
            .env("DYLD_PRINT_LIBRARIES", "1")
            .output()
            .unwrap()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn commands(image: &[u8]) -> Vec<(u32, &[u8])> {
    assert_eq!(u32_at(image, 0), 0xfeedfacf);
    let count = u32_at(image, 16) as usize;
    let end = 32 + u32_at(image, 20) as usize;
    let mut position = 32;
    let mut result = Vec::new();
    for _ in 0..count {
        let kind = u32_at(image, position);
        let size = u32_at(image, position + 4) as usize;
        assert!(size >= 8 && position + size <= end);
        result.push((kind, &image[position..position + size]));
        position += size;
    }
    assert_eq!(position, end);
    result
}

fn dynamic_libraries(image: &[u8]) -> Vec<&str> {
    commands(image)
        .into_iter()
        .filter(|(kind, _)| *kind == 0xc) // LC_LOAD_DYLIB
        .map(|(_, command)| {
            let name = &command[u32_at(command, 8) as usize..];
            std::str::from_utf8(name.split(|byte| *byte == 0).next().unwrap()).unwrap()
        })
        .collect()
}

fn bind_stream(image: &[u8]) -> Option<&[u8]> {
    commands(image)
        .into_iter()
        .find(|(kind, _)| *kind == 0x80000022) // LC_DYLD_INFO_ONLY
        .map(|(_, command)| {
            let offset = u32_at(command, 16) as usize;
            let size = u32_at(command, 20) as usize;
            &image[offset..offset + size]
        })
}

#[test]
fn macos_public_api_keeps_type_and_module_boundaries() {
    let project = Project::new();
    project.write("macos.ddr", "pub fn answer()->i32{42}");
    let checked = project.checked(
        r#"
import graphics.macos as macos
import graphics.macos as other
import .macos as local
fn main(){
    color:other.Color=macos.rgb(1.0,0.0,0.0)
    window:other.Window=macos.Window{id:0}
    print(color.alpha==1.0);print(window.id);print(local.answer())
}
"#,
    );
    assert!(checked.program.main.is_some());

    for (source, code) in [
        (
            r#"import graphics.macos as macos
fn main(){macos.open("title",100,true)}"#,
            "E0201",
        ),
        (
            r#"import graphics.macos as macos
fn main(){macos.open("title",100)}"#,
            "E0207",
        ),
        (
            r#"import graphics.macos as macos
fn main(){macos.poll(0)}"#,
            "E0201",
        ),
        (
            r#"import graphics.macos as macos
fn main(){w:=macos.Window{id:0};var vertices:[]f64=[];macos.draw(w,&vertices,0.0,0.0,0.0)}"#,
            "E0201",
        ),
        (
            r#"import graphics.macos as macos
fn main(){w:=macos.Window{id:0};x:i32=macos.close(w)}"#,
            "E0201",
        ),
        (
            r#"import graphics.macos as macos
fn main(){__macos_open("title",100,100)}"#,
            "E0205",
        ),
        (
            r#"import graphics.macos as macos
fn main(){__macos_poll(macos.Window{id:0})}"#,
            "E0205",
        ),
        (
            r#"import graphics.macos as macos
fn main(){var vertices:[]f32=[];__macos_draw(macos.Window{id:0},&vertices,0.0,0.0,0.0)}"#,
            "E0205",
        ),
        ("fn main(){__macos_close(0)}", "E0205"),
        (
            r#"import graphics.macos as macos
fn main(){macos.__macos_open("title",100,100)}"#,
            "E0404",
        ),
    ] {
        project.rejects(source, code);
    }

    project.write(
        "macos.ddr",
        "pub fn attempt(){__macos_open(\"title\",100,100)}",
    );
    project.rejects("import macos\nfn main(){macos.attempt()}", "E0205");
}

#[test]
fn geometry_and_unreachable_window_calls_keep_the_existing_no_import_image() {
    let project = Project::new();
    let baseline = project.compile("fn main(){print(42)}");
    let unused_import = project.compile("import graphics.macos as macos\nfn main(){print(42)}");
    let unused_window = project.compile(
        r#"
import graphics.macos as macos
fn unused(){result:=macos.open("unused",100,100);macos.close(result.window)}
fn main(){print(42)}
"#,
    );
    assert_eq!(baseline, unused_import);
    assert_eq!(baseline, unused_window);

    let geometry = project.compile(
        r#"
import graphics.macos as macos
fn main(){
    var vertices:[]f32=[]
    macos.quad(&mut vertices,-1.0,-1.0,2.0,2.0,macos.rgba(0.2,0.3,0.4,0.5))
    assert(vertices.len==48);assert(vertices[0]==-1.0);assert(vertices[23]==0.5)
    print(vertices.len)
}
"#,
    );
    for image in [&baseline, &unused_import, &unused_window, &geometry] {
        assert!(dynamic_libraries(image).is_empty());
        assert!(bind_stream(image).is_none());
        assert!(!commands(image)
            .iter()
            .any(|(kind, command)| { *kind == 0x19 && command[8..24].starts_with(b"__DATA\0") }));
    }
}

#[test]
fn reachable_window_calls_add_only_the_required_dynamic_imports() {
    let project = Project::new();
    for source in [
        r#"import graphics.macos as macos
fn main(){result:=macos.open("invalid size",0,0);print(result.ok)}"#,
        r#"import graphics.macos as macos
fn poll()->bool{macos.poll(macos.Window{id:0}).closed}
fn main(){print(poll())}"#,
        r#"import graphics.macos as macos
fn main(){f:fn(macos.Window)=macos.close;f(macos.Window{id:0})}"#,
    ] {
        let image = project.compile(source);
        assert_eq!(
            dynamic_libraries(&image),
            ["/usr/lib/libSystem.B.dylib", "/usr/lib/libobjc.A.dylib"]
        );
        let binds = bind_stream(&image).expect("reachable native API needs dynamic bindings");
        for symbol in [
            b"_objc_msgSend\0".as_slice(),
            b"_sel_registerName\0".as_slice(),
        ] {
            assert!(binds.windows(symbol.len()).any(|window| window == symbol));
        }
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn assert_headless_success(output: &std::process::Output, expected: &[u8]) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "status {:?}\nstdout: {}\nstderr: {stderr}",
        output.status,
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(output.stdout, expected);
    // Ensure tracing was active, then prove these error paths never load AppKit.
    assert!(stderr.contains("libSystem.B.dylib"), "{stderr}");
    for framework in [
        "AppKit.framework",
        "QuartzCore.framework",
        "Metal.framework",
    ] {
        assert!(!stderr.contains(framework), "loaded {framework}: {stderr}");
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn invalid_handles_report_closed_and_no_key_and_close_is_idempotent_without_appkit() {
    let project = Project::new();
    let output = project.execute(
        r#"
import graphics.macos as macos
fn main(){
    handles:[i64;5]=[0,-1,1,65536,9223372036854775807]
    var vertices:[]f32=[]
    for id in handles{
        window:=macos.Window{id:id}
        copy:=window
        for i in 0..3{
            frame:=macos.poll(window)
            assert(frame.closed);assert(frame.key == -1)
            assert(frame.width==0&&frame.height==0)
            assert(frame.pixel_width==0&&frame.pixel_height==0)
            assert(frame.time==0.0&&frame.scale==0.0)
            assert(!frame.pointer_down&&!frame.resized)
            result:=macos.draw(copy,&vertices,0.1,0.2,0.3)
            assert(!result.ok&&result.frames==0)
            assert(result.error=="invalid or expired window handle")
            macos.close(window);macos.close(copy)
        }
    }
    print("invalid handles ok")
}
"#,
    );
    assert_headless_success(&output, b"invalid handles ok\n");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn invalid_dimensions_are_rejected_before_loading_appkit() {
    let project = Project::new();
    let output = project.execute(
        r#"
import graphics.macos as macos
fn reject(width:i32,height:i32){
    result:=macos.open("must not initialize AppKit",width,height)
    assert(!result.ok&&result.window.id==0)
    assert(result.error=="Window dimensions must be between 1 and 16384 points")
    frame:=macos.poll(result.window)
    assert(frame.closed&&frame.key == -1)
    macos.close(result.window);macos.close(result.window)
}
fn main(){
    reject(0,100);reject(100,0)
    reject(-1,100);reject(100,-1)
    reject(16385,100);reject(100,16385)
    reject(-2147483648,100);reject(100,2147483647)
    print("invalid dimensions ok")
}
"#,
    );
    assert_headless_success(&output, b"invalid dimensions ok\n");
}

#[test]
fn reachable_native_window_apis_reject_both_linux_targets_with_a_clear_error() {
    use dunder::target::Target;
    let project = Project::new();
    for source in [
        r#"import graphics.macos as macos
fn main(){result:=macos.open("unavailable",0,0);print(result.ok)}"#,
        r#"import graphics.macos as macos
fn poll()->bool{macos.poll(macos.Window{id:0}).closed}
fn main(){print(poll())}"#,
        r#"import graphics.macos as macos
fn main(){var vertices:[]f32=[];drawn:=macos.draw(macos.Window{id:0},&vertices,0.0,0.0,0.0);print(drawn.ok)}"#,
        r#"import graphics.macos as macos
fn main(){close:fn(macos.Window)=macos.close;close(macos.Window{id:0})}"#,
    ] {
        let checked = project.checked(source);
        for target in [Target::Arm64Linux, Target::X86_64Linux] {
            let error = checked.compile_for(target).unwrap_err();
            assert_eq!(error.code, "E0300");
            assert_eq!(
                error.message,
                format!("native module requires macOS; selected target is {target}")
            );
        }
    }
}

#[test]
fn pure_geometry_with_unreachable_window_apis_compiles_for_both_linux_targets() {
    use dunder::target::Target;
    let project = Project::new();
    let checked = project.checked(
        r#"
import graphics.macos as macos
fn unused_window(){
    opened:=macos.open("unreachable",100,100)
    frame:=macos.poll(opened.window)
    var vertices:[]f32=[]
    drawn:=macos.draw(opened.window,&vertices,0.0,0.0,0.0)
    macos.close(opened.window)
}
fn main(){
    var vertices:[]f32=[]
    macos.quad(&mut vertices,-1.0,-1.0,2.0,2.0,macos.rgba(0.2,0.3,0.4,0.5))
    assert(vertices.len==48);assert(vertices[0]==-1.0);assert(vertices[23]==0.5)
    print(vertices.len)
}
"#,
    );
    for (target, machine) in [(Target::Arm64Linux, 183u16), (Target::X86_64Linux, 62u16)] {
        let image = checked
            .compile_for(target)
            .unwrap_or_else(|error| panic!("{}", checked.render(&error)));
        assert_eq!(&image[..4], b"\x7fELF");
        assert_eq!(
            u16::from_le_bytes(image[18..20].try_into().unwrap()),
            machine
        );
        assert_eq!(image, checked.compile_for(target).unwrap());
    }
}
