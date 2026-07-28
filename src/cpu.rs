use std::fs::File;
use std::io::Read;
use std::path::Path;


pub struct Cpu {
    ram: [u8; 4096],
    v: [u8; 16],
    i: u16,
    pc: u16,
    stack: [u16; 16],
    sp: u16,
    pub delay_timer: u8,
    pub sound_timer: u8,
    pub keypad: [bool; 16],
    display: [bool; 64 * 32],
    rng_state: u32,
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            ram: [0; 4096],
            v: [0; 16],
            i: 0,
            pc: 0x200,
            stack: [0; 16],
            sp: 0,
            delay_timer: 0,
            sound_timer: 0,
            keypad: [false; 16],
            display: [false; 64 * 32],
            rng_state: 0xACE1,
        }
    }

    pub fn load_rom_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        let mut file = File::open(path)?;
        
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let start_address = 0x200;
        if start_address + buffer.len() > 4096 {
            panic!("Fehler: Das ROM ist zu groß für den CHIP-8 Arbeitsspeicher!");
        }

        for (i, &byte) in buffer.iter().enumerate() {
            self.ram[start_address + i] = byte;
        }

        println!("ROM erfolgreich geladen! ({} Bytes)", buffer.len());
        Ok(())
    }

    pub fn render_to_console(&self) {
        print!("\x1B[H");

        let mut output = String::with_capacity(64 * 32 + 32);

        for y in 0..32 {
            for x in 0..64 {
                let index = y * 64 + x;
                if self.display[index] {
                    output.push_str("██");
                } else {
                    output.push_str("  ");
                }
            }
            output.push('\n');
        }

        print!("{}", output);
        
        use std::io::Write;
        std::io::stdout().flush().unwrap();
    }

    pub fn step(&mut self) {
        let opcode = self.fetch();
        self.execute(opcode);
    }

    pub fn fetch(&mut self) -> u16 {
        let first_bit = self.ram[self.pc as usize] as u16;
        let second_bit = self.ram[self.pc as usize + 1] as u16;
        self.pc += 2;
        first_bit << 8 | second_bit
    }

    fn next_random_byte(&mut self) -> u8 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x << 17;
        x ^= x << 5;

        self.rng_state = x;

        (x & 0xFF) as u8
    }

    pub fn execute(&mut self, opcode: u16) {
        if let Some(instruction) = Instruction::decode(opcode) {
            match instruction {
                Instruction::ClearDisplay => {
                    self.display.fill(false);
                }
                Instruction::Return => {
                    if self.sp > 0 {
                        self.sp -= 1;
                        self.pc = self.stack[self.sp as usize];
                    }
                }
                Instruction::Jump(nnn) => {
                    self.pc = nnn;
                }
                Instruction::Call(nnn) => {
                    self.stack[self.sp as usize] = self.pc;
                    self.sp += 1;
                    self.pc = nnn;
                }
                Instruction::SkipIfEqConst(x, kk) => {
                    if self.v[x] == kk {
                        self.pc += 2;
                    }
                }
                Instruction::SkipIfNeqConst(x, kk) => {
                    if self.v[x] != kk {
                        self.pc += 2;
                    }
                }
                Instruction::SkipIfEqReg(x, y) => {
                    if self.v[x] == self.v[y] {
                        self.pc += 2;
                    }
                }
                Instruction::SetRegConst(x, kk) => {
                    self.v[x] = kk;
                }
                Instruction::AddRegConst(x, kk) => {
                    self.v[x] = self.v[x].wrapping_add(kk);
                }
                Instruction::MoveReg(x, y) => {
                    self.v[x] = self.v[y];
                }
                Instruction::OrReg(x, y) => {
                    self.v[x] |= self.v[y];
                }
                Instruction::AndReg(x, y) => {
                    self.v[x] &= self.v[y];
                }
                Instruction::XorReg(x, y) => {
                    self.v[x] ^= self.v[y];
                }
                Instruction::AddRegReg(x, y) => {
                    let sum = self.v[x] as u16 + self.v[y] as u16;
                    self.v[0xF] = if sum > 255 { 1 } else { 0 };
                    self.v[x] = sum as u8;
                }
                Instruction::SubRegReg(x, y) => {
                    let borrow = self.v[x] >= self.v[y];
                    self.v[x] = self.v[x].wrapping_sub(self.v[y]);
                    self.v[0xF] = if borrow { 1 } else { 0 };
                }
                Instruction::ShiftRight(x, y) => {
                    self.v[0xF] = if self.v[x] & 0x01 == 1 { 1 } else { 0 };
                    self.v[x] = self.v[x] >> 1;
                }
                Instruction::SubNRegReg(x, y) => {
                    self.v[0xF] = if self.v[y] > self.v[x] { 1 } else { 0 };
                    self.v[x] = self.v[y].wrapping_sub(self.v[x]);
                }
                Instruction::ShiftLeft(x, y) => {
                    self.v[0xF] = { if (self.v[x] & 0x80) >> 7 == 1 { 1 } else { 0 } };
                    self.v[x] = self.v[x].wrapping_shl(1);
                }
                Instruction::SkipIfNeqReg(x, y) => {
                    if self.v[x] != self.v[y] {
                        self.pc += 2;
                    }
                }
                Instruction::SetIndexConst(nnn) => {
                    self.i = nnn;
                }
                Instruction::JumpWithOffset(nnn) => {
                    self.pc = self.v[0] as u16 + nnn;
                }
                Instruction::Random(x, kk) => {
                    let rng_byte = self.next_random_byte();
                    self.v[x] = rng_byte & kk;
                }
                Instruction::Draw(x, y, n) => {
                    let start_x = self.v[x] as usize;
                    let start_y = self.v[y] as usize;

                    let start = self.i as usize;
                    let end = start + n as usize;
                    let sprite = &self.ram[start..end];

                    self.v[0xF] = 0;

                    for (row_index, sprite_byte) in sprite.iter().enumerate() {
                        for bit_index in 0..8 {
                            if (sprite_byte & (0x80 >> bit_index)) != 0 {
                                let pixel_x = (start_x + bit_index) % 64;
                                let pixel_y = (start_y + row_index) % 32;
                                let screen_index = pixel_y * 64 + pixel_x;

                                if self.display[screen_index] == true {
                                    self.v[0xF] = 1;
                                }
                                self.display[screen_index] ^= true;
                            }
                        }
                    }
                }
                Instruction::SkipIfKeyPressed(x) => {
                    if self.keypad[self.v[x] as usize] {
                        self.pc += 2;
                    }
                }
                Instruction::SkipIfKeyNotPressed(x) => {
                    if !self.keypad[self.v[x] as usize] {
                        self.pc += 2;
                    }
                }
                Instruction::GetDelayTimer(x) => {
                    self.v[x] = self.delay_timer;
                }
                Instruction::WaitForKeypress(x) => {
                    let mut key_pressed = false;
                    for i in 0..16 {
                        if self.keypad[i] {
                            self.v[x] = i as u8;
                            key_pressed = true;
                            break;
                        }
                    }

                    if !key_pressed {
                        self.pc -= 2;
                    }
                }
                Instruction::SetDelayTimer(x) => {
                    self.delay_timer = self.v[x];
                }
                Instruction::SetSoundTimer(x) => {
                    self.sound_timer = self.v[x];
                }
                Instruction::AddIndexReg(x) => {
                    self.i = self.i + self.v[x] as u16;
                }
                Instruction::SetIndexToSprite(x) => {
                    self.i = (self.v[x] as u16) * 5;
                }
                Instruction::StoreBCD(x) => {
                    let value = self.v[x];
                    self.ram[self.i as usize] = value / 100;
                    self.ram[self.i as usize + 1] = (value / 10) % 10;
                    self.ram[self.i as usize + 2] = value % 10;
                }
                Instruction::StoreRegisters(x) => {
                    for i in 0..=x {
                        let pos = self.i as usize + i;
                        self.ram[pos] = self.v[i];
                    }
                    self.i += x as u16 + 1;
                }
                Instruction::LoadRegisters(x) => {
                    for i in 0..=x {
                        let pos = self.i as usize + i;
                        self.v[i] = self.ram[pos];
                    }
                    self.i += x as u16 + 1;
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Instruction {
    ClearDisplay,               // 00E0 - CLS
    Return,                     // 00EE - RET
    Jump(u16),                  // 1NNN - JP addr
    Call(u16),                  // 2NNN - CALL addr
    SkipIfEqConst(usize, u8),   // 3XKK - SE Vx, byte
    SkipIfNeqConst(usize, u8),  // 4XKK - SNE Vx, byte
    SkipIfEqReg(usize, usize),  // 5XY0 - SE Vx, Vy
    SetRegConst(usize, u8),     // 6XKK - LD Vx, byte
    AddRegConst(usize, u8),     // 7XKK - ADD Vx, byte
    MoveReg(usize, usize),      // 8XY0 - LD Vx, Vy
    OrReg(usize, usize),        // 8XY1 - OR Vx, Vy
    AndReg(usize, usize),       // 8XY2 - AND Vx, Vy
    XorReg(usize, usize),       // 8XY3 - XOR Vx, Vy
    AddRegReg(usize, usize),    // 8XY4 - ADD Vx, Vy (Sets VF carry)
    SubRegReg(usize, usize),    // 8XY5 - SUB Vx, Vy (Sets VF NOT borrow)
    ShiftRight(usize, usize),   // 8XY6 - SHR Vx {, Vy}
    SubNRegReg(usize, usize),   // 8XY7 - SUBN Vx, Vy
    ShiftLeft(usize, usize),    // 8XYE - SHL Vx {, Vy}
    SkipIfNeqReg(usize, usize), // 9XY0 - SNE Vx, Vy
    SetIndexConst(u16),         // ANNN - LD I, addr
    JumpWithOffset(u16),        // BNNN - JP V0, addr
    Random(usize, u8),          // CXKK - RND Vx, byte
    Draw(usize, usize, u8),     // DXYN - DRW Vx, Vy, nibble
    SkipIfKeyPressed(usize),    // EX9E - SKP Vx
    SkipIfKeyNotPressed(usize), // EXA1 - SKNP Vx
    GetDelayTimer(usize),       // FX07 - LD Vx, DT
    WaitForKeypress(usize),     // FX0A - LD Vx, K
    SetDelayTimer(usize),       // FX15 - LD DT, Vx
    SetSoundTimer(usize),       // FX18 - LD ST, Vx
    AddIndexReg(usize),         // FX1E - ADD I, Vx
    SetIndexToSprite(usize),    // FX29 - LD F, Vx
    StoreBCD(usize),            // FX33 - LD B, Vx
    StoreRegisters(usize),      // FX55 - LD [I], Vx
    LoadRegisters(usize),       // FX65 - LD Vx, [I]
}

impl Instruction {
    pub fn decode(opcode: u16) -> Option<Self> {
        let n1 = ((opcode & 0xF000) >> 12) as u8;
        let n2 = ((opcode & 0x0F00) >> 8) as usize;
        let n3 = ((opcode & 0x00F0) >> 4) as usize;
        let n4 = opcode & 0x000f;

        let nnn = opcode & 0x0FFF;
        let kk = (opcode & 0x00FF) as u8;

        match (n1, n2, n3, n4) {
            (0x0, 0x0, 0xE, 0x0) => Some(Instruction::ClearDisplay), // 00E0 - CLS
            (0x0, 0x0, 0xE, 0xE) => Some(Instruction::Return),       // 00EE - RET
            (0x1, _, _, _) => Some(Instruction::Jump(nnn)),          // 1NNN - JP addr
            (0x2, _, _, _) => Some(Instruction::Call(nnn)),          // 2NNN - CALL addr
            (0x3, x, _, _) => Some(Instruction::SkipIfEqConst(x, kk)), // 3XKK - SE Vx, byte
            (0x4, x, _, _) => Some(Instruction::SkipIfNeqConst(x, kk)), // 4XKK - SNE Vx, byte
            (0x5, x, y, 0x0) => Some(Instruction::SkipIfEqReg(x, y)), // 5XY0 - SE Vx, Vy
            (0x6, x, _, _) => Some(Instruction::SetRegConst(x, kk)), // 6XKK - LD Vx, byte
            (0x7, x, _, _) => Some(Instruction::AddRegConst(x, kk)), // 7XKK - ADD Vx, byte
            (0x8, x, y, 0x0) => Some(Instruction::MoveReg(x, y)),    // 8XY0 - LD Vx, Vy
            (0x8, x, y, 0x1) => Some(Instruction::OrReg(x, y)),      // 8XY1 - OR Vx, Vy
            (0x8, x, y, 0x2) => Some(Instruction::AndReg(x, y)),     // 8XY2 - AND Vx, Vy
            (0x8, x, y, 0x3) => Some(Instruction::XorReg(x, y)),     // 8XY3 - XOR Vx, Vy
            (0x8, x, y, 0x4) => Some(Instruction::AddRegReg(x, y)), // 8XY4 - ADD Vx, Vy (Sets VF carry)
            (0x8, x, y, 0x5) => Some(Instruction::SubRegReg(x, y)), // 8XY5 - SUB Vx, Vy (Sets VF NOT borrow)
            (0x8, x, y, 0x6) => Some(Instruction::ShiftRight(x, y)), // 8XY6 - SHR Vx {, Vy}
            (0x8, x, y, 0x7) => Some(Instruction::SubNRegReg(x, y)), // 8XY7 - SUBN Vx, Vy
            (0x8, x, y, 0xE) => Some(Instruction::ShiftLeft(x, y)), // 8XYE - SHL Vx {, Vy}
            (0x9, x, y, 0x0) => Some(Instruction::SkipIfNeqReg(x, y)), // 9XY0 - SNE Vx, Vy
            (0xA, _, _, _) => Some(Instruction::SetIndexConst(nnn)), // ANNN - LD I, addr
            (0xB, _, _, _) => Some(Instruction::JumpWithOffset(nnn)), // BNNN - JP V0, addr
            (0xC, x, _, _) => Some(Instruction::Random(x, kk)),     // CXKK - RND Vx, byte
            (0xD, x, y, n) => Some(Instruction::Draw(x, y, n as u8)), // DXYN - DRW Vx, Vy, nibble
            (0xE, x, 0x9, 0xE) => Some(Instruction::SkipIfKeyPressed(x)), // EX9E - SKP Vx
            (0xE, x, 0xA, 0x1) => Some(Instruction::SkipIfKeyNotPressed(x)), // EXA1 - SKNP Vx
            (0xF, x, 0x0, 0x7) => Some(Instruction::GetDelayTimer(x)), // FX07 - LD Vx, DT
            (0xF, x, 0x0, 0xA) => Some(Instruction::WaitForKeypress(x)), // FX0A - LD Vx, K
            (0xF, x, 0x1, 0x5) => Some(Instruction::SetDelayTimer(x)), // FX15 - LD DT, Vx
            (0xF, x, 0x1, 0x8) => Some(Instruction::SetSoundTimer(x)), // FX18 - LD ST, Vx
            (0xF, x, 0x1, 0xE) => Some(Instruction::AddIndexReg(x)), // FX1E - ADD I, Vx
            (0xF, x, 0x2, 0x9) => Some(Instruction::SetIndexToSprite(x)), // FX29 - LD F, Vx
            (0xF, x, 0x3, 0x3) => Some(Instruction::StoreBCD(x)),   // FX33 - LD B, Vx
            (0xF, x, 0x5, 0x5) => Some(Instruction::StoreRegisters(x)), // FX55 - LD [I], Vx
            (0xF, x, 0x6, 0x5) => Some(Instruction::LoadRegisters(x)), // FX65 - LD Vx, [I]
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0x3A;
        cpu.ram[0x201] = 0x4B;

        let result = cpu.fetch();
        assert_eq!(result, 0x3A4B);
        assert_eq!(cpu.pc, 0x202);
    }

    #[test]
    fn test_new_cpu() {
        let cpu = Cpu::new();
        assert_eq!(cpu.ram, [0; 4096]);
        assert_eq!(cpu.v, [0; 16]);
        assert_eq!(cpu.i, 0);
        assert_eq!(cpu.pc, 0x200);
        assert_eq!(cpu.stack, [0; 16]);
        assert_eq!(cpu.sp, 0);
        assert_eq!(cpu.delay_timer, 0);
        assert_eq!(cpu.sound_timer, 0);
        assert_eq!(cpu.keypad, [false; 16]);
        assert_eq!(cpu.display, [false; 64 * 32]);
    }

    #[test]
    fn test_opcode_00_e0() {
        let mut cpu = Cpu::new();
        cpu.display[1] = true;
        cpu.display[5] = true;
        cpu.display[10] = true;
        cpu.display[11] = true;

        cpu.execute(0x00E0);
        assert_eq!(cpu.display, [false; 64 * 32]);
    }

    #[test]
    fn test_opcode_00_e0_full() {
        let mut cpu = Cpu::new();
        cpu.display[1] = true;
        cpu.display[5] = true;
        cpu.display[10] = true;
        cpu.display[11] = true;

        cpu.ram[0x200] = 0x00;
        cpu.ram[0x201] = 0xE0;

        cpu.step();
        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.display, [false; 64 * 32]);
    }

    #[test]
    fn test_opcode_00_ee() {
        let mut cpu = Cpu::new();
        cpu.stack[1] = 0x123;
        cpu.sp = 2;

        cpu.execute(0x00EE);

        assert_eq!(cpu.pc, 0x123);
        assert_eq!(cpu.sp, 1);
    }

    #[test]
    fn test_opcode_00_ee_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0x00;
        cpu.ram[0x201] = 0xEE;

        cpu.stack[1] = 0x123;
        cpu.sp = 2;

        cpu.step();

        assert_eq!(cpu.sp, 1);
        assert_eq!(cpu.pc, 0x123);
    }

    #[test]
    fn test_opcode_1n_nn() {
        let mut cpu = Cpu::new();
        cpu.execute(0x1123);

        assert_eq!(cpu.pc, 0x123);
    }

    #[test]
    fn test_opcode_1n_nn_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0x11;
        cpu.ram[0x201] = 0x23;

        cpu.step();

        assert_eq!(cpu.pc, 0x123);
    }

    #[test]
    fn test_opcode_2n_nn() {
        let mut cpu = Cpu::new();
        cpu.execute(0x2123);

        assert_eq!(cpu.stack[0], 0x200);
        assert_eq!(cpu.sp, 1);
        assert_eq!(cpu.pc, 0x123);
    }

    #[test]
    fn test_opcode_2n_nn_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0x21;
        cpu.ram[0x201] = 0x23;

        cpu.step();

        assert_eq!(cpu.stack[0], 0x202);
        assert_eq!(cpu.sp, 1);
        assert_eq!(cpu.pc, 0x123);
    }

    #[test]
    fn test_opcode_return_from_subroutine() {
        let mut cpu = Cpu::new();
        cpu.stack[0] = 0x401;
        cpu.stack[1] = 0x402;
        cpu.sp = 2;

        cpu.execute(0x00EE);
        assert_eq!(cpu.pc, 0x402);
        assert_eq!(cpu.sp, 1);

        //check sp does not go lower than 0
        cpu.execute(0x00EE);
        assert_eq!(cpu.pc, 0x401);
        assert_eq!(cpu.sp, 0);
    }

    #[test]
    fn test_opcode_return_from_subroutine_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0x00;
        cpu.ram[0x201] = 0xEE;
        cpu.stack[0] = 0x401;
        cpu.stack[1] = 0x402;
        cpu.sp = 1;

        cpu.step();

        assert_eq!(cpu.pc, 0x401);
        assert_eq!(cpu.sp, 0);
    }

    #[test]
    fn test_opcode_skip_next() {
        let mut cpu = Cpu::new();
        cpu.v[2] = 0x22;
        cpu.execute(0x3222);

        assert_eq!(cpu.pc, 0x202);
    }

    #[test]
    fn test_opcode_skip_next_negative() {
        let mut cpu = Cpu::new();
        cpu.v[2] = 0x24;
        cpu.execute(0x3222);

        assert_eq!(cpu.pc, 0x200);
    }

    #[test]
    fn test_opcode_skip_next_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0x32;
        cpu.ram[0x201] = 0x22;
        cpu.v[2] = 0x22;
        cpu.step();
        assert_eq!(cpu.pc, 0x204);
    }

    #[test]
    fn test_opcode_skip_next_if_not_equal_negative() {
        let mut cpu = Cpu::new();
        cpu.v[2] = 0x22;
        cpu.execute(0x4222);
        assert_eq!(cpu.pc, 0x200);
    }

    #[test]
    fn test_opcode_skip_next_if_not_equal() {
        let mut cpu = Cpu::new();
        cpu.v[2] = 0x24;
        cpu.execute(0x4222);
        assert_eq!(cpu.pc, 0x202);
    }

    #[test]
    fn test_opcode_skip_next_if_not_equal_full() {
        let mut cpu = Cpu::new();
        cpu.v[2] = 0x24;
        cpu.ram[0x200] = 0x42;
        cpu.ram[0x202] = 0x22;

        cpu.step();

        assert_eq!(cpu.pc, 0x204);
    }

    #[test]
    fn test_opcode_0x5() {
        let mut cpu = Cpu::new();
        cpu.v[1] = 0x1;
        cpu.v[2] = 0x1;

        cpu.execute(0x5120);
        assert_eq!(cpu.pc, 0x202);
    }

    #[test]
    fn test_opcode_0x5_negative() {
        let mut cpu = Cpu::new();
        cpu.v[1] = 0x1;
        cpu.v[2] = 0x2;

        cpu.execute(0x5120);
        assert_eq!(cpu.pc, 0x200);
    }

    #[test]
    fn test_opcode_0x5_full() {
        let mut cpu = Cpu::new();
        cpu.v[1] = 0x2;
        cpu.v[2] = 0x2;
        cpu.ram[0x200] = 0x51;
        cpu.ram[0x201] = 0x20;

        cpu.step();

        assert_eq!(cpu.pc, 0x204);
    }

    #[test]
    fn test_opcode_6xkk() {
        let mut cpu = Cpu::new();
        let opcode = 0x6342;

        cpu.execute(opcode);
        assert_eq!(cpu.v[3], 0x42);
    }

    #[test]
    fn test_opcode_6xkk_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0x63;
        cpu.ram[0x201] = 0x42;

        cpu.step();
        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[3], 0x42);
    }

    #[test]
    fn test_opcode_0x7() {
        let mut cpu = Cpu::new();
        cpu.v[1] = 0x2;
        cpu.execute(0x7122);

        assert_eq!(cpu.v[1], 0x2 + 0x22);
    }

    #[test]
    fn test_opcode_0x7_full() {
        let mut cpu = Cpu::new();
        cpu.v[1] = 0x2;
        cpu.ram[0x200] = 0x71;
        cpu.ram[0x201] = 0x22;

        cpu.step();

        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[1], 0x2 + 0x22);
    }

    #[test]
    fn test_opcode_0x8_00() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x1;
        cpu.v[1] = 0x0;

        cpu.execute(0x8010);

        assert_eq!(cpu.v[0], 0x0);
    }

    #[test]
    fn test_opcode_0x8_00_full() {
        let mut cpu = Cpu::new();

        cpu.ram[0x200] = 0x80;
        cpu.ram[0x201] = 0x10;

        cpu.v[0] = 0x1;
        cpu.v[1] = 0x0;

        cpu.step();

        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[0], 0x0);
    }

    #[test]
    fn test_opcode_0x8_01() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x1;
        cpu.v[1] = 0x0;

        cpu.execute(0x8011);

        assert_eq!(cpu.v[0], 0x1);
    }

    #[test]
    fn test_opcode_0x8_01_full() {
        let mut cpu = Cpu::new();

        cpu.ram[0x200] = 0x80;
        cpu.ram[0x201] = 0x11;

        cpu.v[0] = 0x1;
        cpu.v[1] = 0x0;

        cpu.step();

        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[0], 0x1);
    }

    #[test]
    fn test_opcode_0x8_02() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x1;
        cpu.v[1] = 0x0;

        cpu.execute(0x8012);

        assert_eq!(cpu.v[0], 0x0);
    }

    #[test]
    fn test_opcode_0x8_02_full() {
        let mut cpu = Cpu::new();

        cpu.ram[0x200] = 0x80;
        cpu.ram[0x201] = 0x12;

        cpu.v[0] = 0x1;
        cpu.v[1] = 0x0;

        cpu.step();

        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[0], 0x0);
    }

    #[test]
    fn test_opcode_0x8_03() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x1;
        cpu.v[1] = 0x0;

        cpu.execute(0x8013);

        assert_eq!(cpu.v[0], 0x1);
    }

    #[test]
    fn test_opcode_0x8_03_full() {
        let mut cpu = Cpu::new();

        cpu.ram[0x200] = 0x80;
        cpu.ram[0x201] = 0x13;

        cpu.v[0] = 0x1;
        cpu.v[1] = 0x0;

        cpu.step();

        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[0], 0x1);
    }

    #[test]
    fn test_opcode_0x8_04() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x1;
        cpu.v[1] = 0x3;

        cpu.execute(0x8014);

        assert_eq!(cpu.v[0], 0x4);
    }

    #[test]
    fn test_opcode_0x8_04_full() {
        let mut cpu = Cpu::new();

        cpu.ram[0x200] = 0x80;
        cpu.ram[0x201] = 0x14;

        cpu.v[0] = 0x1;
        cpu.v[1] = 0x4;

        cpu.step();

        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[0], 0x5);
    }

    #[test]
    fn test_opcode_0x8_05() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x2;
        cpu.v[1] = 0x1;

        cpu.execute(0x8015);

        assert_eq!(cpu.v[0], 0x1);
    }

    #[test]
    fn test_opcode_0x8_05_full() {
        let mut cpu = Cpu::new();

        cpu.ram[0x200] = 0x80;
        cpu.ram[0x201] = 0x15;

        cpu.v[0] = 0x2;
        cpu.v[1] = 0x1;

        cpu.step();

        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[0], 0x1);
    }

    #[test]
    fn test_opcode_0x8_06() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x1;
        cpu.v[1] = 0x0;

        cpu.execute(0x8016);

        assert_eq!(cpu.v[0], 0x1 >> 1);
    }

    #[test]
    fn test_opcode_0x8_06_full() {
        let mut cpu = Cpu::new();

        cpu.ram[0x200] = 0x80;
        cpu.ram[0x201] = 0x16;

        cpu.v[0] = 0x1;
        cpu.v[1] = 0x0;

        cpu.step();

        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[0], 0x1 >> 1);
    }

    #[test]
    fn test_opcode_0x8_07() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x1;
        cpu.v[1] = 0x2;

        cpu.execute(0x8017);

        assert_eq!(cpu.v[0], 0x1);
    }

    #[test]
    fn test_opcode_0x8_07_full() {
        let mut cpu = Cpu::new();

        cpu.ram[0x200] = 0x80;
        cpu.ram[0x201] = 0x17;

        cpu.v[0] = 0x1;
        cpu.v[1] = 0x2;

        cpu.step();

        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[0], 0x1);
    }

    #[test]
    fn test_opcode_0x8_0e() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x1;
        cpu.v[1] = 0x0;

        cpu.execute(0x801e);

        assert_eq!(cpu.v[0], 0x1 << 1);
    }

    #[test]
    fn test_opcode_0x8_0e_full() {
        let mut cpu = Cpu::new();

        cpu.ram[0x200] = 0x80;
        cpu.ram[0x201] = 0x1e;

        cpu.v[0] = 0x1;
        cpu.v[1] = 0x0;

        cpu.step();

        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[0], 0x1 << 1);
    }

    #[test]
    fn test_opcode_0x9() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x1;
        cpu.v[1] = 0x0;

        cpu.execute(0x9010);

        assert_eq!(cpu.pc, 0x202);
    }

    #[test]
    fn test_opcode_0x9_full() {
        let mut cpu = Cpu::new();

        cpu.ram[0x200] = 0x90;
        cpu.ram[0x201] = 0x10;

        cpu.v[0] = 0x1;
        cpu.v[1] = 0x0;

        cpu.step();

        assert_eq!(cpu.pc, 0x204);
    }

    #[test]
    fn test_opcode_0x_a() {
        let mut cpu = Cpu::new();

        cpu.execute(0xA123);

        assert_eq!(cpu.i, 0x123);
    }

    #[test]
    fn test_opcode_0x_a_full() {
        let mut cpu = Cpu::new();

        cpu.ram[0x200] = 0xA1;
        cpu.ram[0x201] = 0x23;

        cpu.step();

        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.i, 0x123);
    }

    #[test]
    fn test_opcode_0x_b() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x1;

        cpu.execute(0xB123);

        assert_eq!(cpu.pc, 0x124);
    }

    #[test]
    fn test_opcode_0x_b_full() {
        let mut cpu = Cpu::new();

        cpu.ram[0x200] = 0xB1;
        cpu.ram[0x201] = 0x23;

        cpu.v[0] = 0x1;

        cpu.step();

        assert_eq!(cpu.pc, 0x124);
    }

    #[test]
    fn test_opcode_0x_c() {
        let mut cpu = Cpu::new();
        cpu.execute(0xC000);
        assert_eq!(cpu.v[0], 0);
    }

    #[test]
    fn test_opcode_0x_c_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0xC0;
        cpu.ram[0x201] = 0x00;
        cpu.step();
        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[0], 0);
    }

    #[test]
    fn test_opcode_0x_d() {
        let mut cpu = Cpu::new();
        cpu.i = 0x300;
        cpu.ram[0x300] = 0b10000000; // Ein Pixel oben links
        cpu.v[0] = 0; // X
        cpu.v[1] = 0; // Y

        cpu.execute(0xD011); // Zeichne 1 Byte aus RAM an V0, V1
        assert_eq!(cpu.display[0], true);
        assert_eq!(cpu.v[0xF], 0); // Kein Kollisions-Flag (da vorher leer)
    }

    #[test]
    fn test_opcode_0x_d_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0xD0;
        cpu.ram[0x201] = 0x11;

        cpu.i = 0x300;
        cpu.ram[0x300] = 0b10000000;
        cpu.v[0] = 0;
        cpu.v[1] = 0;

        cpu.step();
        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.display[0], true);
    }

    #[test]
    fn test_opcode_0x_e() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 5;
        cpu.keypad[5] = true; // Taste gedrückt

        cpu.execute(0xE09E);
        assert_eq!(cpu.pc, 0x202); // Skip!
    }

    #[test]
    fn test_opcode_0x_e_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0xE0;
        cpu.ram[0x201] = 0x9E;
        cpu.v[0] = 5;
        cpu.keypad[5] = true;

        cpu.step();
        assert_eq!(cpu.pc, 0x204); // Step (2) + Skip (2) = 204
    }

    #[test]
    fn test_opcode_0x_ex_a1() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 5;
        cpu.keypad[5] = false; // Taste NICHT gedrückt

        cpu.execute(0xE0A1);
        assert_eq!(cpu.pc, 0x202); // Skip!
    }

    #[test]
    fn test_opcode_0x_ex_a1_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0xE0;
        cpu.ram[0x201] = 0xA1;
        cpu.v[0] = 5;
        cpu.keypad[5] = false;

        cpu.step();
        assert_eq!(cpu.pc, 0x204);
    }

    #[test]
    fn test_opcode_0x_fx_07() {
        let mut cpu = Cpu::new();
        cpu.delay_timer = 0x42;
        cpu.execute(0xF007);
        assert_eq!(cpu.v[0], 0x42);
    }

    #[test]
    fn test_opcode_0x_fx_07_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0xF0;
        cpu.ram[0x201] = 0x07;
        cpu.delay_timer = 0x42;

        cpu.step();
        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[0], 0x42);
    }

    #[test]
    fn test_opcode_0x_fx_0a() {
        let mut cpu = Cpu::new();
        // Fall 1: Keine Taste gedrückt -> PC wird dekrementiert (bzw. blockiert)
        cpu.execute(0xF00A);
        assert_eq!(cpu.pc, 0x1FE); // 0x200 - 2

        // Fall 2: Taste gedrückt -> Speichern und weitergehen
        let mut cpu2 = Cpu::new();
        cpu2.keypad[7] = true;
        cpu2.execute(0xF00A);
        assert_eq!(cpu2.v[0], 7);
        assert_eq!(cpu2.pc, 0x200); // bleibt normal
    }

    #[test]
    fn test_opcode_0x_fx_0a_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0xF0;
        cpu.ram[0x201] = 0x0A;
        cpu.keypad[7] = true;

        cpu.step();
        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[0], 7);
    }

    #[test]
    fn test_opcode_0x_fx_15() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x25;
        cpu.execute(0xF015);
        assert_eq!(cpu.delay_timer, 0x25);
    }

    #[test]
    fn test_opcode_0x_fx_15_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0xF0;
        cpu.ram[0x201] = 0x15;
        cpu.v[0] = 0x25;

        cpu.step();
        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.delay_timer, 0x25);
    }

    #[test]
    fn test_opcode_0x_fx_18() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x12;
        cpu.execute(0xF018);
        assert_eq!(cpu.sound_timer, 0x12);
    }

    #[test]
    fn test_opcode_0x_fx_18_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0xF0;
        cpu.ram[0x201] = 0x18;
        cpu.v[0] = 0x12;

        cpu.step();
        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.sound_timer, 0x12);
    }

    #[test]
    fn test_opcode_0x_fx_1e() {
        let mut cpu = Cpu::new();
        cpu.i = 0x100;
        cpu.v[0] = 0x20;
        cpu.execute(0xF01E);
        assert_eq!(cpu.i, 0x120);
    }

    #[test]
    fn test_opcode_0x_fx_1e_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0xF0;
        cpu.ram[0x201] = 0x1E;
        cpu.i = 0x100;
        cpu.v[0] = 0x20;

        cpu.step();
        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.i, 0x120);
    }

    #[test]
    fn test_opcode_0x_fx_29() {
        let mut cpu = Cpu::new();
        cpu.v[5] = 2; // Sprite für '2'
        cpu.execute(0xF529);
        assert_eq!(cpu.i, 10); // 2 * 5
    }

    #[test]
    fn test_opcode_0x_fx_29_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0xF5;
        cpu.ram[0x201] = 0x29;
        cpu.v[5] = 2;

        cpu.step();
        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.i, 10);
    }

    #[test]
    fn test_opcode_0x_fx_33() {
        let mut cpu = Cpu::new();
        cpu.i = 0x400;
        cpu.v[2] = 142;

        cpu.execute(0xF233);
        assert_eq!(cpu.ram[0x400], 1);
        assert_eq!(cpu.ram[0x401], 4);
        assert_eq!(cpu.ram[0x402], 2);
    }

    #[test]
    fn test_opcode_0x_fx_33_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0xF2;
        cpu.ram[0x201] = 0x33;
        cpu.i = 0x400;
        cpu.v[2] = 142;

        cpu.step();
        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.ram[0x400], 1);
        assert_eq!(cpu.ram[0x401], 4);
        assert_eq!(cpu.ram[0x402], 2);
    }

    #[test]
    fn test_opcode_0x_fx_55() {
        let mut cpu = Cpu::new();
        cpu.i = 0x500;
        cpu.v[0] = 11;
        cpu.v[1] = 22;
        cpu.v[2] = 33;

        cpu.execute(0xF255); // Speichere V0 bis V2
        assert_eq!(cpu.ram[0x500], 11);
        assert_eq!(cpu.ram[0x501], 22);
        assert_eq!(cpu.ram[0x502], 33);
        assert_eq!(cpu.i, 0x503); // i wurde erhöht um x + 1
    }

    #[test]
    fn test_opcode_0x_fx_55_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0xF2;
        cpu.ram[0x201] = 0x55;
        cpu.i = 0x500;
        cpu.v[0] = 11;
        cpu.v[1] = 22;
        cpu.v[2] = 33;

        cpu.step();
        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.ram[0x500], 11);
        assert_eq!(cpu.i, 0x503);
    }

    #[test]
    fn test_opcode_0x_fx_65() {
        let mut cpu = Cpu::new();
        cpu.i = 0x600;
        cpu.ram[0x600] = 55;
        cpu.ram[0x601] = 66;

        cpu.execute(0xF165); // Lade V0 und V1 aus RAM
        assert_eq!(cpu.v[0], 55);
        assert_eq!(cpu.v[1], 66);
        assert_eq!(cpu.i, 0x602);
    }

    #[test]
    fn test_opcode_0x_fx_65_full() {
        let mut cpu = Cpu::new();
        cpu.ram[0x200] = 0xF1;
        cpu.ram[0x201] = 0x65;
        cpu.i = 0x600;
        cpu.ram[0x600] = 55;
        cpu.ram[0x601] = 66;

        cpu.step();
        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.v[0], 55);
        assert_eq!(cpu.v[1], 66);
        assert_eq!(cpu.i, 0x602);
    }
}
