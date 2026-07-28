mod cpu;
use cpu::Cpu;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::io::{self, Read};
use std::os::unix::io::AsRawFd;

#[repr(C)]
#[derive(Clone, Copy)]
struct Termios {
    c_iflag: usize,
    c_oflag: usize,
    c_cflag: usize,
    c_lflag: usize,
    c_cc: [u8; 20],
    c_ispeed: usize,
    c_ospeed: usize,
}

const ICANON: usize = 0x00000100;
const ECHO: usize   = 0x00000008;
const TCSANOW: i32  = 0;        

unsafe extern "C" {
    fn tcgetattr(fd: i32, termios_ptr: *mut Termios) -> i32;
    fn tcsetattr(fd: i32, optional_actions: i32, termios_ptr: *const Termios) -> i32;
}


struct TerminalGuard {
    fd: i32,
    original_termios: Termios,
}

impl TerminalGuard {
    fn init(fd: i32) -> Self {
        unsafe {
            let mut termios_setup = std::mem::zeroed::<Termios>();
            tcgetattr(fd, &mut termios_setup);
            
            let original_termios = termios_setup;
            let mut raw_termios = original_termios;
            
            raw_termios.c_lflag &= !(ICANON | ECHO);
            tcsetattr(fd, TCSANOW, &raw_termios);
            
            TerminalGuard { fd, original_termios }
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        unsafe {
            tcsetattr(self.fd, TCSANOW, &self.original_termios);
            print!("\x1B[?25h"); // Cursor wieder einblenden
            let _ = io::Write::flush(&mut io::stdout());
        }
    }
}

fn main() {
    let mut cpu = Cpu::new();
    
    if let Err(e) = cpu.load_rom_file("SpaceInvaders.ch8") {
        eprintln!("Fehler beim Laden des ROMs: {}", e);
        return;
    }

    let stdin_fd = io::stdin().as_raw_fd();
    
    let _guard = TerminalGuard::init(stdin_fd);

    let keypad_shared = Arc::new(Mutex::new([false; 16]));
    let keypad_for_thread = Arc::clone(&keypad_shared);

    thread::spawn(move || {
        let mut buf = [0u8; 1];
        loop {
            if io::stdin().read(&mut buf).is_ok() {
                let ch = buf[0] as char;
                let key_index = match ch {
                    '1' => Some(0x1), '2' => Some(0x2), '3' => Some(0x3), '4' => Some(0xC),
                    'q' => Some(0x4), 'w' => Some(0x5), 'e' => Some(0x6), 'r' => Some(0xD),
                    'a' => Some(0x7), 's' => Some(0x8), 'd' => Some(0x9), 'f' => Some(0xE),
                    'y' => Some(0xA), 'x' => Some(0x0), 'c' => Some(0xB), 'v' => Some(0xF),
                    _ => None,
                };

                if let Some(idx) = key_index {
                    if let Ok(mut keys) = keypad_for_thread.lock() { keys[idx] = true; }
                    thread::sleep(Duration::from_millis(50));
                    if let Ok(mut keys) = keypad_for_thread.lock() { keys[idx] = false; }
                }
            }
        }
    });

    print!("\x1B[2J\x1B[?25l");

    loop {
        if let Ok(keys) = keypad_shared.lock() {
            cpu.keypad.copy_from_slice(&*keys);
        }

        for _ in 0..10 {
            cpu.step();
        }

        if cpu.delay_timer > 0 { cpu.delay_timer -= 1; }
        if cpu.sound_timer > 0 { cpu.sound_timer -= 1; }

        cpu.render_to_console();

        thread::sleep(Duration::from_millis(16));
    }
}
