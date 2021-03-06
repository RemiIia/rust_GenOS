use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

use super::{debug, error, print};
use super::{gdt, hlt_loop};
use lazy_static::lazy_static;

use pic8259_simple::ChainedPics;
use spin;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Breakpoint,
    DoubleFault,
    PageFault,
    Timer = PIC_1_OFFSET,
    Keyboard = PIC_1_OFFSET + 1,
}

impl InterruptIndex {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }

    pub fn send_bye_signal(i: InterruptIndex) {
        unsafe {
            PICS.lock().notify_end_of_interrupt(i.as_u8());
        }
    }

    extern "x86-interrupt" fn page_fault(
        stack_frame: &mut InterruptStackFrame,
        error_code: PageFaultErrorCode,
    ) {
        use x86_64::registers::control::Cr2;

        error!("PAGE FAULT");
        error!("Accessed Address: $0C{:?}", Cr2::read());
        error!("Error Code: $0C{:?}", error_code);
        error!(" $0C{:#?}", stack_frame);
        hlt_loop();
    }

    extern "x86-interrupt" fn timer(_stack_frame: &mut InterruptStackFrame) {
        //print!(".");
        InterruptIndex::send_bye_signal(InterruptIndex::Timer);
    }

    extern "x86-interrupt" fn keyboard(_stack_frame: &mut InterruptStackFrame) {
        use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
        use spin::Mutex;
        use x86_64::instructions::port::Port;

        lazy_static! {
            static ref KEYBOARD: Mutex<Keyboard<layouts::Azerty, ScancodeSet1>> = Mutex::new(
                Keyboard::new(layouts::Azerty, ScancodeSet1, HandleControl::Ignore)
            );
        }

        let mut keyboard = KEYBOARD.lock();
        let mut port = Port::new(0x60);

        let scancode: u8 = unsafe { port.read() };
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => print!("{}", character),
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                }
            }
        }

        InterruptIndex::send_bye_signal(InterruptIndex::Keyboard);
    }

    extern "x86-interrupt" fn breakpoint(stack_frame: &mut InterruptStackFrame) {
        error!("BREAKPOINT\n{:#?}", stack_frame);
    }

    extern "x86-interrupt" fn double_fault(
        stack_frame: &mut InterruptStackFrame,
        _error_code: u64,
    ) -> ! {
        error!("DOUBLE-FAULT:\n{:#?}", stack_frame);
        panic!("$0CCan't continue on double fault.");
    }
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // exeptions

        idt.breakpoint.set_handler_fn(InterruptIndex::breakpoint);
        unsafe {
            idt.double_fault
                .set_handler_fn(InterruptIndex::double_fault)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt.page_fault.set_handler_fn(InterruptIndex::page_fault);

        // interupts


        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(InterruptIndex::timer);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(InterruptIndex::keyboard);
        idt
    };
}

pub fn init_idt() {
    debug!("Initialisation of the IDT");
    IDT.load();
}

#[test_case]
fn test_breakpoint_exception() {
    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();
}
