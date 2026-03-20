use std::{ffi::CString, io::Write, collections::HashMap};
use nix::unistd::{ForkResult, execvp, fork};
use nix::sys::ptrace;
use nix::sys::wait::{waitpid, WaitStatus};

// Holds all debugger state
struct Debugger {
    pid: nix::unistd::Pid,
    breakpoints: HashMap<u64, u8>, // address -> original byte before we patched it with 0xCC
}

impl Debugger {
    fn new(pid: nix::unistd::Pid) -> Self {
        Debugger {
            pid,
            breakpoints: HashMap::new(),
        }
    }

    // Patch the byte at `addr` with INT3 (0xCC) so the CPU traps when it gets there
    fn set_breakpoint(&mut self, addr: u64) {
        let orig = ptrace::read(self.pid, addr as *mut _).expect("ptrace read failed");
        let orig_byte = (orig & 0xFF) as u8;
        self.breakpoints.insert(addr, orig_byte);

        // keep the top 7 bytes, replace only the lowest byte with 0xCC
        let patched = (orig & !0xFF) | 0xCC;
        ptrace::write(self.pid, addr as *mut _, patched).expect("ptrace write failed");
        println!("[debugger] breakpoint set at 0x{:x}", addr);
    }

    // When the CPU hits 0xCC it stops one byte past it — step back and restore the original byte
    fn restore_breakpoint(&mut self, addr: u64) {
        if let Some(&orig_byte) = self.breakpoints.get(&addr) {
            let current = ptrace::read(self.pid, addr as *mut _).expect("ptrace read failed");
            let restored = (current & !0xFF) | orig_byte as i64;
            ptrace::write(self.pid, addr as *mut _, restored).expect("ptrace write failed");

            // rewind rip by 1 because the CPU advanced past the 0xCC
            let mut regs = ptrace::getregs(self.pid).expect("getregs failed");
            regs.rip -= 1;
            ptrace::setregs(self.pid, regs).expect("setregs failed");
        }
    }

    fn run(&mut self) {
        loop {
            match waitpid(self.pid, None).expect("waitpid failed") {
                WaitStatus::Exited(_, code) => {
                    println!("[debugger] process exited with code {}", code);
                    break;
                }
                WaitStatus::Stopped(pid, _signal) => {
                    let regs = ptrace::getregs(pid).expect("getregs failed");
                    let rip = regs.rip;
                    println!("[debugger] rip = 0x{:x}", rip);

                    // if we stopped on a breakpoint address, restore the original byte
                    if self.breakpoints.contains_key(&(rip - 1)) {
                        self.restore_breakpoint(rip - 1);
                    }

                    // command loop — keep reading commands until the process is resumed
                    loop {
                        match get_command().as_str() {
                            "step" => {
                                ptrace::step(pid, None).expect("step failed");
                                break;
                            }
                            "cont" => {
                                ptrace::cont(pid, None).expect("cont failed");
                                break;
                            }
                            "regs" => {
                                let regs = ptrace::getregs(pid).expect("getregs failed");
                                print_regs(&regs);
                            }
                            cmd if cmd.starts_with("break ") => {
                                // expects: break 0x401234
                                let addr_str = cmd.trim_start_matches("break ").trim_start_matches("0x");
                                match u64::from_str_radix(addr_str, 16) {
                                    Ok(addr) => self.set_breakpoint(addr),
                                    Err(_) => println!("invalid address"),
                                }
                            }
                            _ => {
                                println!("commands: step, cont, regs, break <addr>");
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn get_command() -> String {
    print!("(dbg) ");
    std::io::stdout().flush().unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

fn print_regs(regs: &nix::libc::user_regs_struct) {
    println!("rip = 0x{:x}", regs.rip);
    println!("rax = 0x{:x}", regs.rax);
    println!("rbx = 0x{:x}", regs.rbx);
    println!("rcx = 0x{:x}", regs.rcx);
    println!("rdx = 0x{:x}", regs.rdx);
    println!("rsi = 0x{:x}", regs.rsi);
    println!("rdi = 0x{:x}", regs.rdi);
    println!("rsp = 0x{:x}", regs.rsp);
    println!("rbp = 0x{:x}", regs.rbp);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: debugger <program>");
        std::process::exit(1);
    }

    let target = CString::new(args[1].clone()).unwrap();

    match unsafe { fork() }.expect("fork failed") {
        ForkResult::Child => {
            ptrace::traceme().expect("traceme failed");
            execvp(&target, &[&target]).expect("execvp failed");
        }
        ForkResult::Parent { child } => {
            println!("[debugger] attached to pid {}", child);
            let mut dbg = Debugger::new(child);
            dbg.run();
        }
    }
}
