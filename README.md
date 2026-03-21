# dbg

A Linux debugger built from scratch in Rust using `ptrace`.

Implements the core features of a real debugger — breakpoints, single stepping, register inspection, memory reads, stack walking, and symbol resolution — without any external debugger libraries.

## Features

- **Breakpoints** — set breakpoints at any address using `INT3` patching (`0xCC`)
- **Single stepping** — execute one instruction at a time
- **Register inspection** — dump all x86-64 registers
- **Memory reads** — inspect raw bytes at any address
- **String reads** — read null-terminated strings from process memory
- **Pointer dereferencing** — follow a pointer and print the string it points to
- **Stack walking** — walk the `rbp` chain to produce a full backtrace
- **Symbol resolution** — parse the target ELF binary's symbol table and resolve addresses to function names automatically

## Usage

```bash
cargo build
cargo run -- <program>
```

Example:
```bash
cargo run -- /bin/ls
```

## Commands

| Command | Description |
|---|---|
| `step` | Execute one instruction |
| `cont` | Run until next breakpoint or exit |
| `regs` | Print all x86-64 registers |
| `memory <addr> [len]` | Print `len` bytes (default 64) starting at `addr` |
| `string <addr>` | Read a null-terminated string at `addr` |
| `deref <addr>` | Follow a pointer at `addr` and print the string it points to |
| `break <addr>` | Set a breakpoint at `addr` |
| `backtrace` | Walk the call stack and print frame addresses with function names |

## How it works

### Attaching
The debugger forks a child process. The child calls `ptrace(TRACEME)` then `execvp` to become the target program. The kernel pauses the child before its first instruction and notifies the parent.

### Breakpoints
Breakpoints work by overwriting the target byte at the given address with `0xCC` (`INT3`). When the CPU executes `INT3` it raises `SIGTRAP`, pausing the child. On resuming, the debugger restores the original byte and rewinds `rip` by 1.

### Symbol resolution
On startup the debugger parses the target ELF binary's symbol table and reads `/proc/<pid>/maps` to determine the load base address (accounting for ASLR). All symbol addresses are shifted by the load base so they match runtime addresses.

### Stack walking
The backtrace walks the `rbp` chain. At each frame, `rbp+8` holds the return address and `rbp+0` holds the caller's saved `rbp`. Following this chain gives the full call history.

## Dependencies

- [`nix`](https://crates.io/crates/nix) — safe Rust bindings for Linux syscalls (`ptrace`, `fork`, `waitpid`)
- [`elf`](https://crates.io/crates/elf) — ELF binary parsing
