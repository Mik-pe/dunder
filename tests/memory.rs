#![cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]

#[cfg(target_os = "linux")]
#[path = "support/graphics.rs"]
mod graphics;

use dunder::target::Target;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dunder-memory-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn build(&self, source: &str, target: Target) -> PathBuf {
        let source_path = self.0.join("main.ddr");
        fs::write(&source_path, source).unwrap();
        let checked = dunder::modules::check_path(source_path)
            .unwrap_or_else(|error| panic!("{}", error.render()));
        let image = checked
            .compile_for(target)
            .unwrap_or_else(|error| panic!("{}", checked.render(&error)));
        let binary = self.0.join("program");
        fs::write(&binary, image).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        binary
    }

    fn run(&self, binary: &Path, emulator: Option<&Path>, bounded_rss: bool) -> Output {
        let mut command = if let Some(emulator) = emulator {
            let mut command = Command::new(emulator);
            command.arg(binary);
            command
        } else {
            Command::new(binary)
        };
        let stdout = self.0.join("stdout");
        let stderr = self.0.join("stderr");
        command
            .current_dir(&self.0)
            .env_clear()
            .env("PATH", "/no-external-tools")
            .stdout(Stdio::from(fs::File::create(&stdout).unwrap()))
            .stderr(Stdio::from(fs::File::create(&stderr).unwrap()));
        for name in [
            "VK_ICD_FILENAMES",
            "VK_DRIVER_FILES",
            "VK_LAYER_PATH",
            "VK_INSTANCE_LAYERS",
            "LD_LIBRARY_PATH",
            "XDG_RUNTIME_DIR",
        ] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        let mut child = command.spawn().unwrap();
        let started = Instant::now();
        let mut rss_kib = 0u64;
        let status = loop {
            // A raised arena ceiling alone must not make these tests pass.
            // Sampling is an additional bound; successful cumulative allocation
            // above 512 MiB remains the cross-platform reclamation assertion.
            if let Ok(status) = fs::read_to_string(format!("/proc/{}/status", child.id())) {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") || line.starts_with("VmHWM:") {
                        if let Some(value) = line.split_whitespace().nth(1) {
                            rss_kib = rss_kib.max(value.parse().unwrap());
                        }
                    }
                }
            }
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if started.elapsed() > Duration::from_secs(30) {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "bounded-live-memory program exceeded 30 seconds\n{}",
                    fs::read_to_string(&stderr).unwrap()
                );
            }
            thread::sleep(Duration::from_millis(5));
        };
        let output = Output {
            status,
            stdout: fs::read(stdout).unwrap(),
            stderr: fs::read(stderr).unwrap(),
        };
        assert!(
            output.status.success(),
            "status {:?}; sampled RSS {rss_kib} KiB\nstdout: {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if bounded_rss && emulator.is_none() {
            assert!(
                rss_kib < 96 * 1024,
                "a small live payload retained {rss_kib} KiB; dead buffers must be reclaimed"
            );
        }
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn native(&self, source: &str) {
        let binary = self.build(source, Target::host().unwrap());
        let result = self.run(&binary, None, true);
        assert_eq!(result.stdout, b"memory ok\n");
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn repeated_array_copies_and_assignments_reclaim_dead_buffers() {
    Project::new().native(
        r#"
fn last(values:[]i64)->i64{values[values.len-1]}
fn main(){
 var source:[]i64=[]
 source.reserve(1024)
 for i in 0..1024{source.push(i64(i))}
 var current:[]i64=[]
 for i in 0..200000{
  var copy:=source
  copy[0]=i64(i)
  assert(source[0]==0 && copy[1023]==1023)
  current=copy
  current=current
  assert(current[0]==i64(i))
  assert(last(current)==1023)
 }
 assert(source[0]==0 && current[0]==199999)
 print("memory ok")
}
"#,
    );
}

#[test]
fn unused_capacity_and_control_flow_release_owners() {
    Project::new().native(
        r#"
fn early(n:i32)->i32{
 var allocation:[]i64=[]
 allocation.reserve(65536)
 if n%2==0{return n}
 var i:=0
 while i<4{
  var temporary:[]i64=[]
  temporary.reserve(65536)
  i=i+1
  if i<3{continue}
  break
 }
 n
}
fn main(){
 var total:i64=0
 for i in 0..4096{
  var empty:[]i64=[]
  empty.reserve(65536)
  assert(empty.len==0 && empty.cap()>=65536)
  total=total+i64(early(i))
  if i%2==0{continue}
  var tail:[]i64=[]
  tail.reserve(65536)
 }
 assert(total==8386560)
 print("memory ok")
}
"#,
    );
}

#[test]
fn array_returns_callbacks_and_expression_temporaries_release_storage() {
    Project::new().native(
        r#"
fn make(seed:i32)->[]i64{
 var values:[]i64=[]
 values.reserve(1024)
 for i in 0..1024{values.push(i64(seed+i))}
 values
}
fn invoke(f:fn(i32)->[]i64,seed:i32)->[]i64{f(seed)}
fn recurse(depth:i32,values:[]i64)->[]i64{
 if depth==0{return values}
 recurse(depth-1,values)
}
fn main(){
 for i in 0..100000{
  values:=invoke(make,i)
  assert(values[0]==i64(i) && values[1023]==i64(i+1023))
 }
 for i in 0..10000{
  assert(make(i).len==1024)
  returned:=recurse(3,make(i))
  assert(returned[1023]==i64(i+1023))
 }
 print("memory ok")
}
"#,
    );
}

#[test]
fn nested_structs_enum_variants_and_borrows_preserve_value_semantics() {
    Project::new().native(
        r#"
struct Box{items:[]i64,other:[]i64,labels:[]string}
enum Value{Empty,Items([]i64),Record(Box)}
fn self_assign(values:&mut []i64){values=values}
fn replace(box:&mut Box){
 var copy:=box.items
 copy[0]=99
 box.items=copy
 self_assign(&mut box.items)
}
fn main(){
 label:="retained "+42.str()
 var base:[]i64=[]
 base.reserve(1024)
 for i in 0..1024{base.push(i64(i))}
 original:=Box{items:base,other:base,labels:[label,"second"]}
 var value:Value=.Empty
 for i in 0..40000{
  if i%2==0{value=.Items(base)}
  else{value=.Record(original)}
  value=value
  snapshot:=value
  value=.Empty
  match snapshot{
   .Items(items)=>{
    var copy:=items
    self_assign(&mut copy)
    copy[0]=99
    assert(items[0]==0 && copy[0]==99)
   },
   .Record(record)=>{
    var copy:=record
    replace(&mut copy)
    assert(record.items[0]==0 && copy.items[0]==99)
    assert(copy.other[1023]==1023)
    copy.labels.clear()
    assert(record.labels[0]=="retained 42")
   },
   .Empty=>{assert(false)},
  }
 }
 assert(original.items[0]==0 && original.other[1023]==1023)
 assert(original.labels[0]==label)
 print("memory ok")
}
"#,
    );
}

#[test]
fn iteration_snapshots_survive_source_mutation_and_release_at_loop_exit() {
    Project::new().native(
        r#"
fn main(){
 var source:[]i64=[]
 for i in 0..1024{source.push(i64(i))}
 for round in 0..100000{
  var checksum:i64=0
  for value in source{
   source[0]=99
   checksum=checksum+value
   if value==5{break}
  }
  assert(checksum==15)
  source[0]=0
 }
 print("memory ok")
}
"#,
    );
}

#[test]
#[cfg(target_os = "linux")]
fn arm64_reclaims_arrays_and_strings_when_native_or_qemu_execution_is_available() {
    let emulator = if cfg!(target_arch = "aarch64") {
        None
    } else {
        let explicit = std::env::var_os("DUNDER_QEMU_AARCH64").map(PathBuf::from);
        if let Some(ref path) = explicit {
            assert!(
                path.is_file(),
                "DUNDER_QEMU_AARCH64 must name an executable"
            );
        }
        let candidates = explicit.into_iter().chain([
            PathBuf::from("/usr/bin/qemu-aarch64"),
            PathBuf::from("/usr/bin/qemu-aarch64-static"),
            PathBuf::from("/workspace/.tools/qemu/usr/bin/qemu-aarch64"),
        ]);
        let Some(path) = candidates.into_iter().find(|path| path.is_file()) else {
            eprintln!("ARM64 memory execution requires DUNDER_QEMU_AARCH64 or qemu-aarch64");
            return;
        };
        Some(fs::canonicalize(path).unwrap())
    };
    let project = Project::new();
    let binary = project.build(
        r#"
fn change(text:&mut string)->string{text="changed"+"!";"tail"+"!"}
fn main(){
 for i in 0..4096{var empty:[]i64=[];empty.reserve(65536)}
 var source:[]i64=[]
 for i in 0..4096{source.push(i64(i))}
 for i in 0..20000{
  var copy:=source
  copy[0]=99
  assert(source[0]==0 && copy[4095]==4095)
 }
 prefix:="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
 var saved:=""
 for i in 0..200000{
  var text:=prefix+i.str()
  var words:[]string=[text,"",text[0..64]]
  var copy:=words
  words.clear()
  assert(copy.pop()==prefix)
  assert(copy.pop()=="")
  value:=copy.pop()
  if i==12345{saved=value}
  assert(text+change(&mut text)==value+"tail!")
  var repeated:[string;3]=[value;3]
  repeated[0]=text
  assert(repeated[1]==value && repeated[2]==value)
 }
 assert(saved==prefix+"12345")
 print("memory ok")
}
"#,
        Target::Arm64Linux,
    );
    let result = project.run(&binary, emulator.as_deref(), false);
    assert_eq!(result.stdout, b"memory ok\n");
}

#[test]
#[cfg(target_os = "linux")]
fn repeated_vulkan_readback_snapshots_reclaim_owned_arrays() {
    if std::env::var_os("DUNDER_VULKAN_TEST").is_none() {
        eprintln!("set DUNDER_VULKAN_TEST=1 to exercise repeated Vulkan readback ownership");
        return;
    }
    let project = Project::new();
    graphics::install(&project.0);
    let binary = project.build(
        r#"
import graphics.vulkan as vk
fn main(){
 opened:=vk.open_headless(64,48)
 assert(opened.ok)
 empty:[]f32=[]
 drawn:=vk.draw(opened.window,&empty,0.0,0.0,1.0)
 assert(drawn.ok)
 original:=vk.pixels(opened.window)
 for i in 0..4000{
  var pixels:=vk.pixels(opened.window)
  assert(pixels.len==12288 && pixels[2]==255 && pixels[3]==255)
  pixels[2]=0
  assert(original[2]==255)
 }
 vk.close(opened.window)
 assert(original[2]==255)
 print("memory ok")
}
"#,
        Target::host().unwrap(),
    );
    let result = project.run(&binary, None, false);
    assert_eq!(result.stdout, b"memory ok\n");
}

#[test]
fn separate_lexical_blocks_reuse_large_buffers_before_function_return() {
    Project::new().native(
        r#"
fn main(){
 {
  var first:[]i64=[]
  first.reserve(8388608)
  for i in 0..8388608{first.push(i64(i))}
  assert(first[8388607]==8388607)
 }
 {
  var second:[]i64=[]
  second.reserve(8388608)
  for i in 0..8388608{second.push(i64(i+1))}
  assert(second[8388607]==8388608)
 }
 print("memory ok")
}
"#,
    );
}

#[test]
fn a_million_string_returns_slices_and_field_replacements_keep_memory_bounded() {
    Project::new().native(r#"
struct Label{text:string}
fn describe(n:i32)->string{"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"+n.str()}
fn invoke(f:fn(i32)->string,n:i32)->string{f(n)}
fn main(){
 prefix:="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
 var label:=Label{text:""}
 var current:string=""
 var saved:string=""
 for i in 0..1000000{
  label.text=invoke(describe,i)
  if i==12345{saved=label.text}
  part:=label.text[0..64]
  assert(part==prefix)
  current=part
  label.text=label.text
 }
 assert(current==prefix)
 assert(saved==prefix+"12345")
 assert(label.text==prefix+"999999")
 print("memory ok")
}
"#);
}

#[test]
fn strings_survive_sibling_mutations_and_iterator_source_replacement() {
    Project::new().native(
        r#"
fn change(s:&mut string)->string{s="mutated"+"!";"tail"+"!"}
fn index(s:&mut string)->i32{s="UVWXYZ"+"!";1}
fn low(s:&mut string)->i32{s="UVWXYZ"+"!";1}
fn high(s:&mut string)->i32{s="123456"+"!";5}
fn combine(a:string,b:string)->string{a+":"+b}
fn main(){
 for i in 0..100000{
  var s:="abc"+"def"
  joined:=s+change(&mut s)
  assert(joined=="abcdeftail!" && s=="mutated!")
  s="abc"+"def"
  called:=combine(s,change(&mut s))
  assert(called=="abcdef:tail!")
  s="abc"+"def"
  assert(s[index(&mut s)]==98)
  s="abc"+"def"
  assert(s[low(&mut s)..high(&mut s)]=="bcde")
  s="abc"+"def"
  var sum:=0
  for byte in s.bytes(){s="xxxxx"+"xx";sum=sum+byte}
  assert(sum==597)
  s="åβ"+"🌍"
  assert(s.runes()[index(&mut s)]==`β`)
  s="åβ"+"🌍"
  var runes:=0
  for scalar in s.runes(){s="xxxxx"+"xx";runes=runes+i32(scalar)}
  assert(runes==128932)
 }
 print("memory ok")
}
"#,
    );
}

#[test]
fn strings_in_fixed_arrays_dynamic_arrays_and_pop_results_have_independent_owners() {
    Project::new().native(r#"
fn produce(n:i32)->string{
 var values:[]string=["0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"+n.str(),"literal",""]
 copy:=values
 selected:=copy[0]
 values.clear()
 selected
}
fn main(){
 var saved:string=""
 for i in 0..500000{
  var text:=produce(i)
  fixed:[string;4]=[text;4]
  var fixed_copy:=fixed
  fixed_copy[0]="replacement"+i.str()
  assert(fixed_copy[1]==text && fixed[0]==text)
  empty_fixed:[string;0]=["discarded"+i.str();0]
  assert(empty_fixed.len==0)
  var values:[]string=[text,"literal",""]
  var copy:=values
  copy.reserve(32)
  empty:=copy.pop()
  assert(empty=="")
  literal:=copy.pop()
  assert(literal=="literal")
  popped:=copy.pop()
  copy.push("replacement"+i.str())
  copy.clear()
  values[0]="new"+i.str()
  text="changed"+i.str()
  for entry in fixed{assert(entry==popped)}
  assert(values[0]=="new"+i.str())
  assert(popped.len>=65)
  if i==12345{saved=popped}
 }
 assert(saved=="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef12345")
 print("memory ok")
}
"#);
}

#[test]
fn nested_string_fields_enum_payloads_and_string_array_snapshots_survive_cleanup() {
    Project::new().native(
        r#"
struct Box{label:string,items:[]string}
enum Value{Empty,Text(string),Record(Box)}
fn make(n:i32)->Value{
 text:="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"+n.str()
 .Record(Box{label:text,items:[text,"tail",""]})
}
fn main(){
 var current:Value=.Empty
 var saved:string=""
 for i in 0..200000{
  current=make(i)
  current=current
  snapshot:=current
  current=.Text("replaced"+i.str())
  match snapshot{
   .Record(box)=>{
    var copy:=box
    copy.label="new"+i.str()
    copy.items[0]=copy.label
    assert(box.items[0]==box.label)
    var words:=box.items
    var count:=0
    for word in words{
     words.clear()
     if count==0{assert(word==box.label);saved=word}
     else if count==1{assert(word=="tail")}
     else{assert(word=="")}
     count=count+1
    }
    assert(count==3 && saved==box.label)
   },
   _=>{assert(false)},
  }
  current=.Empty
 }
 assert(saved=="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef199999")
 print("memory ok")
}
"#,
    );
}

#[test]
fn repeated_system_string_results_and_error_paths_release_temporary_storage() {
    let project = Project::new();
    let text = "0123456789abcdef".repeat(8);
    fs::write(project.0.join("input.txt"), &text).unwrap();
    fs::write(project.0.join("invalid.txt"), vec![255; 128]).unwrap();
    project.native(r#"
import sys
fn main(){
 original:=sys.arg(0)
 var retained:string=""
 for i in 0..200000{
  argument:=sys.arg(0)
  assert(argument==original)
  input:=sys.read_text("input.txt")
  assert(input.ok && input.value.len==128)
  retained=input.value
  invalid:=sys.read_text("invalid.txt")
  assert(!invalid.ok && invalid.error==92 && invalid.value=="")
  missing:=sys.read_text("missing.txt")
  assert(!missing.ok && missing.error==2 && missing.value=="")
 }
 assert(retained=="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
 print("memory ok")
}

"#);
}

#[test]
fn repeated_byte_packing_and_utf8_error_paths_release_internal_strings() {
    let project = Project::new();
    fs::write(project.0.join("untouched"), b"previous contents").unwrap();
    project.native(
        r#"
import sys
fn main(){
 var valid:[]i32=[]
 for i in 0..4096{valid.push(65)}
 var out_of_range:=valid
 out_of_range[4095]=256
 var invalid_utf8:=valid
 invalid_utf8[4095]=255
 var retained:=""
 for i in 0..5000{
  written:=sys.write_bytes("output",&valid)
  assert(written.ok)
  decoded:=sys.decode_utf8(&valid)
  assert(decoded.ok && decoded.value.len==4096)
  assert(decoded.value[0]==65 && decoded.value[4095]==65)
  retained=decoded.value
  rejected_write:=sys.write_bytes("untouched",&out_of_range)
  assert(!rejected_write.ok && rejected_write.error==22)
  rejected_range:=sys.decode_utf8(&out_of_range)
  assert(!rejected_range.ok && rejected_range.error==22 && rejected_range.value=="")
  rejected_utf8:=sys.decode_utf8(&invalid_utf8)
  assert(!rejected_utf8.ok && rejected_utf8.error==92 && rejected_utf8.value=="")
 }
 assert(retained.len==4096 && retained[4095]==65)
 print("memory ok")
}
"#,
    );
    assert_eq!(fs::read(project.0.join("output")).unwrap(), vec![65; 4096]);
    assert_eq!(
        fs::read(project.0.join("untouched")).unwrap(),
        b"previous contents"
    );
}
