//! Output targets are independent of the machine running the compiler.
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Arm64Macos,
    Arm64Linux,
    X86_64Linux,
}

impl Target {
    pub const ALL: [Self; 3] = [Self::Arm64Macos, Self::Arm64Linux, Self::X86_64Linux];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Arm64Macos => "arm64-macos",
            Self::Arm64Linux => "arm64-linux",
            Self::X86_64Linux => "x86_64-linux",
        }
    }

    pub fn host() -> Result<Self, String> {
        match (std::env::consts::ARCH, std::env::consts::OS) {
            ("aarch64", "macos") => Ok(Self::Arm64Macos),
            ("aarch64", "linux") => Ok(Self::Arm64Linux),
            ("x86_64", "linux") => Ok(Self::X86_64Linux),
            (arch, os) => Err(format!(
                "unsupported host {arch}-{os}; select an output target with --target (see --list-targets)"
            )),
        }
    }

    pub fn can_run_on_host(self) -> bool {
        Self::host() == Ok(self)
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Target {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "arm64-macos" | "aarch64-apple-darwin" => Ok(Self::Arm64Macos),
            "arm64-linux" | "aarch64-unknown-linux-gnu" => Ok(Self::Arm64Linux),
            "x86_64-linux" | "x86_64-unknown-linux-gnu" => Ok(Self::X86_64Linux),
            _ => Err(format!(
                "unsupported target '{value}'; supported targets: arm64-macos, arm64-linux, x86_64-linux"
            )),
        }
    }
}
