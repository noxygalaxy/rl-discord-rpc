// this is a code for heroic/game wrapper

use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn resolve_rpc_binary_path() -> PathBuf {
    if let Ok(exe_path) = env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let candidate = dir.join("rl_discord_rpc");
            if candidate.exists() {
                return candidate;
            }
        }
    }

    if let Ok(home) = env::var("HOME") {
        let candidate = PathBuf::from(home).join(".local/bin/rl_discord_rpc");
        if candidate.exists() {
            return candidate;
        }
    }

    PathBuf::from("rl_discord_rpc")
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("rl_rpc_wrapper: no game command supplied by Heroic");
        std::process::exit(1);
    }

    let rpc_binary_path = resolve_rpc_binary_path();
    eprintln!("rl_rpc_wrapper: resolved RPC binary path: {}", rpc_binary_path.display());

    let log_path = env::var("HOME")
        .map(|h| PathBuf::from(h).join(".local/share/rl_rpc_wrapper.log"))
        .unwrap_or_else(|_| PathBuf::from("/tmp/rl_rpc_wrapper.log"));

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok();

    let rpc_child = Command::new(&rpc_binary_path)
        .stdout(log_file.as_ref().map(|f| f.try_clone().unwrap()).map(Stdio::from).unwrap_or_else(Stdio::null))
        .stderr(log_file.map(Stdio::from).unwrap_or_else(Stdio::null))
        .spawn();

    match &rpc_child {
        Ok(child) => eprintln!("rl_rpc_wrapper: started rl_discord_rpc (pid {})", child.id()),
        Err(e) => eprintln!("rl_rpc_wrapper: FAILED to start rl_discord_rpc: {e} (path tried: {})", rpc_binary_path.display()),
    }

    let program = &args[0];
    let game_args = &args[1..];

    let status = Command::new(program)
        .args(game_args)
        .status()
        .expect("rl_rpc_wrapper: failed to launch game process");

    if let Ok(mut child) = rpc_child {
        let _ = child.kill();
        let _ = child.wait();
    }

    std::process::exit(status.code().unwrap_or(0));
}
