// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister
// Copyright (C) 2026 ZephyrCodesStuff

use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_objcopy() -> Result<PathBuf, Box<dyn Error>> {
    let check = if cfg!(windows) {
        Command::new("where").arg("arm-none-eabi-objcopy").output()
    } else {
        Command::new("which").arg("arm-none-eabi-objcopy").output()
    };
    if let Ok(output) = check {
        if output.status.success() {
            return Ok(PathBuf::from("arm-none-eabi-objcopy"));
        }
    }

    if let Ok(sysroot_out) = Command::new("rustc").args(["--print", "sysroot"]).output() {
        if sysroot_out.status.success() {
            if let Ok(sysroot) = String::from_utf8(sysroot_out.stdout) {
                let sysroot = sysroot.trim();
                if let Ok(version_out) = Command::new("rustc").arg("-vV").output() {
                    if version_out.status.success() {
                        if let Ok(version_str) = String::from_utf8(version_out.stdout) {
                            if let Some(host_line) =
                                version_str.lines().find(|line| line.starts_with("host:"))
                            {
                                if let Some(host_triple) = host_line.split_whitespace().nth(1) {
                                    let llvm_objcopy = PathBuf::from(sysroot)
                                        .join("lib")
                                        .join("rustlib")
                                        .join(host_triple)
                                        .join("bin")
                                        .join(if cfg!(windows) {
                                            "llvm-objcopy.exe"
                                        } else {
                                            "llvm-objcopy"
                                        });
                                    if llvm_objcopy.exists() {
                                        return Ok(llvm_objcopy);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Err("Neither 'arm-none-eabi-objcopy' nor 'llvm-objcopy' (rustup component llvm-tools) was found.\n\
         Please install either the ARM GCC toolchain or add llvm-tools via:\n\
         rustup component add llvm-tools".into())
}

struct DeviceCfg {
    device: &'static str,
    package: &'static str,
    features: &'static [&'static str],
    elf_name: &'static str,
    target_triple: &'static str,
    syx_product: &'static str,
    default_version: &'static str,
    artifact_stem: &'static str,
    objcopy_pad_to: Option<&'static str>,
}

const ALL_DEVICES: &[&str] = &[
    "lpx",
    "mini",
    "minimk1",
    "lps",
    "mk2",
    "lpp",
    "lppmk3",
];

fn device_cfg(name: &str) -> Option<DeviceCfg> {
    match name {
        "lpx" => Some(DeviceCfg {
            device: "lpx",
            package: "launchpad-x",
            features: &[],
            elf_name: "launchpad-x",
            target_triple: "thumbv7em-none-eabihf",
            syx_product: "/x",
            default_version: "351",
            artifact_stem: "core-launchpad-x",
            objcopy_pad_to: None,
        }),
        "mini" => Some(DeviceCfg {
            device: "mini",
            package: "launchpad-mini-mk3",
            features: &[],
            elf_name: "launchpad-mini-mk3",
            target_triple: "thumbv7em-none-eabihf",
            syx_product: "/minimk3",
            default_version: "407",
            artifact_stem: "core-launchpad-mini-mk3",
            objcopy_pad_to: None,
        }),
        "lps" | "launchpad-s" => Some(DeviceCfg {
            device: "lps",
            package: "launchpad-s-and-mini",
            features: &["--no-default-features", "--features", "launchpad-s"],
            elf_name: "launchpad-s-and-mini",
            target_triple: "thumbv7m-none-eabi",
            syx_product: "/lps",
            default_version: "999",
            artifact_stem: "core-launchpad-s",
            objcopy_pad_to: None,
        }),
        "minimk1" => Some(DeviceCfg {
            device: "minimk1",
            package: "launchpad-s-and-mini",
            features: &["--no-default-features", "--features", "launchpad-mini-mk1"],
            elf_name: "launchpad-s-and-mini",
            target_triple: "thumbv7m-none-eabi",
            syx_product: "/minimk1",
            default_version: "999",
            artifact_stem: "core-launchpad-mini-mk1",
            objcopy_pad_to: None,
        }),
        "mk2" => Some(DeviceCfg {
            device: "mk2",
            package: "launchpad-mk2",
            features: &[],
            elf_name: "launchpad-mk2",
            target_triple: "thumbv7m-none-eabi",
            syx_product: "/mk2",
            default_version: "999",
            artifact_stem: "core-launchpad-mk2",
            objcopy_pad_to: None,
        }),
        "lpp" | "pro" => Some(DeviceCfg {
            device: "lpp",
            package: "launchpad-pro",
            features: &[],
            elf_name: "launchpad-pro",
            target_triple: "thumbv7m-none-eabi",
            syx_product: "/lpp",
            default_version: "154",
            artifact_stem: "core-launchpad-pro",
            objcopy_pad_to: None,
        }),
        "lppmk3" | "pro-mk3" => Some(DeviceCfg {
            device: "lppmk3",
            package: "launchpad-pro-mk3",
            features: &[],
            elf_name: "launchpad-pro-mk3",
            target_triple: "thumbv7em-none-eabihf",
            syx_product: "/lppmk3",

            default_version: "999",
            artifact_stem: "core-launchpad-pro-mk3",
            objcopy_pad_to: Some("0x08080000"),
        }),
        _ => None,
    }
}

fn run(cmd: &str, args: &[&str], cwd: &Path) -> Result<(), Box<dyn Error>> {
    eprintln!("+ {} {}", cmd, args.join(" "));
    let status = Command::new(cmd).args(args).current_dir(cwd).status()?;
    if !status.success() {
        return Err(format!("command failed: {} {:?}", cmd, args).into());
    }
    Ok(())
}

fn parse_package_args(args: &[String]) -> Result<(String, Option<String>, bool), Box<dyn Error>> {
    if args.len() < 2 {
        return Err(
            "usage: cargo xtask package <lpx|mini|minimk1|lps|mk2|lpp|lppmk3> [--version <hex3>] [--release]"
                .into(),
        );
    }
    let device = args[1].clone();
    let mut version: Option<String> = None;
    let mut release = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--version" => {
                i += 1;
                if i >= args.len() {
                    return Err("--version requires a value".into());
                }
                version = Some(args[i].clone());
            }
            "--release" => {
                release = true;
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
        i += 1;
    }
    Ok((device, version, release))
}

fn package(
    repo: &Path,
    device: &str,
    version: Option<&str>,
    release: bool,
) -> Result<(), Box<dyn Error>> {
    let cfg = device_cfg(device)
        .ok_or("device must be one of: lppmk3, lpx, mini, lpp, mk2, lps, minimk1")?;
    let release = release || cfg.device == "mk2" || cfg.device == "minimk1";
    let env_version = env::var("FW_VERSION").ok();
    let version = version
        .or(env_version.as_deref())
        .unwrap_or(cfg.default_version);
    let profile = if release { "release" } else { "debug" };

    let mut cargo_args = vec!["build", "-p", cfg.package, "--target", cfg.target_triple];
    for &feat in cfg.features {
        cargo_args.push(feat);
    }
    if release {
        cargo_args.push("--release");
    }
    run("cargo", &cargo_args, repo)?;

    let elf = repo
        .join("target")
        .join(cfg.target_triple)
        .join(profile)
        .join(cfg.elf_name);
    if !elf.exists() {
        return Err(format!("ELF not found: {}", elf.display()).into());
    }

    let out_dir = repo.join("build").join(cfg.device);
    fs::create_dir_all(&out_dir)?;
    let bin = out_dir.join("fw.bin");
    let syx = out_dir.join("fw.syx");
    let final_syx = repo
        .join("build")
        .join(format!("{}.syx", cfg.artifact_stem));

    let objcopy_bin = find_objcopy()?;
    let elf_arg = elf.display().to_string();
    let bin_arg = bin.display().to_string();
    if let Some(pad_to) = cfg.objcopy_pad_to {
        run(
            &objcopy_bin.to_string_lossy(),
            &[
                "-O",
                "binary",
                "--pad-to",
                pad_to,
                "--gap-fill",
                "0xFF",
                &elf_arg,
                &bin_arg,
            ],
            repo,
        )?;
    } else {
        run(
            &objcopy_bin.to_string_lossy(),
            &["-O", "binary", &elf_arg, &bin_arg],
            repo,
        )?;
    }
    run(
        "python3",
        &[
            "tools/syxtool.py",
            "--to-syx",
            cfg.syx_product,
            version,
            &bin.display().to_string(),
            &syx.display().to_string(),
        ],
        repo,
    )?;
    fs::copy(&syx, &final_syx)?;

    println!("{}", final_syx.display());
    Ok(())
}

fn package_all(repo: &Path) -> Result<(), Box<dyn Error>> {
    for device in ALL_DEVICES {
        eprintln!("==> packaging {device} --release");
        package(repo, device, None, true)?;
    }
    Ok(())
}

fn repo_root() -> Result<PathBuf, Box<dyn Error>> {
    let cwd = env::current_dir()?;
    Ok(cwd)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        return Err("usage: cargo xtask <package>".into());
    }

    let repo = repo_root()?;
    match args[0].as_str() {
        "all" => package_all(&repo),
        "package" => {
            let (device, version, release) = parse_package_args(&args)?;
            package(&repo, &device, version.as_deref(), release)
        }
        cmd => Err(format!("unknown xtask command: {cmd}").into()),
    }
}
