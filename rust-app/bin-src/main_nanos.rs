use rust_app::implementation::*;
use core::pin::Pin;
use core::cell::RefCell;

use ledger_async_block::*;

use ledger_prompts_ui::RootMenu;

use nanos_sdk::io;
nanos_sdk::set_panic!(nanos_sdk::exiting_panic);

use rust_app::*;

static mut COMM_CELL : Option<RefCell<io::Comm>> = None;

static mut HOST_IO_STATE : Option<RefCell<HostIOState>> = None;
static mut STATES_BACKING : ParsersState<'static> = ParsersState::NoState;

#[inline(never)]
unsafe fn initialize() {
    STATES_BACKING = ParsersState::NoState;
    COMM_CELL = Some(RefCell::new(io::Comm::new()));
    let comm = COMM_CELL.as_ref().unwrap();
    HOST_IO_STATE = Some(RefCell::new(HostIOState {
        comm: comm,
        requested_block: None,
        sent_command: None,
    }));

}

#[cfg(not(test))]
#[no_mangle]
extern "C" fn sample_main() {
    unsafe { initialize(); }
    let comm = unsafe { COMM_CELL.as_ref().unwrap() };
    let host_io = HostIO(unsafe { HOST_IO_STATE.as_ref().unwrap() });
    let mut states = unsafe { Pin::new_unchecked( &mut STATES_BACKING ) };

    let mut idle_menu = RootMenu::new([ concat!("Rust App ", env!("CARGO_PKG_VERSION")), "Exit" ]);
    let mut busy_menu = RootMenu::new([ "Working...", "Cancel" ]);

    info!("Rust App {}", env!("CARGO_PKG_VERSION"));
    info!("State sizes\ncomm: {}\nstates: {}"
          , core::mem::size_of::<io::Comm>()
          , core::mem::size_of::<ParsersState>());

    let // Draw some 'welcome' screen
        menu = |states : &ParsersState, idle : & mut RootMenu<2>, busy : & mut RootMenu<2>| {
            match states {
                ParsersState::NoState => idle.show(),
                _ => busy.show(),
            }
        };

    menu(&states, & mut idle_menu, & mut busy_menu);
    loop {
        // Wait for either a specific button push to exit the app
        // or an APDU command
        let evt = comm.borrow_mut().next_event();
        match evt {
            io::Event::Command(ins) => {
                trace!("Command received");
                match handle_apdu(host_io, ins, &mut states) {
                    Ok(()) => {
                        trace!("APDU accepted; sending response");
                        comm.borrow_mut().reply_ok();
                        trace!("Replied");
                    }
                    Err(sw) => comm.borrow_mut().reply(sw),
                };
                menu(&states, & mut idle_menu, & mut busy_menu);
                trace!("Command done");
            }
            io::Event::Button(btn) => {
                trace!("Button received");
                match states.is_no_state() {
                    true => {match idle_menu.update(btn) {
                        Some(1) => { info!("Exiting app at user direction via root menu"); nanos_sdk::exit_app(0) },
                        _ => (),
                    } }
                    false => { match busy_menu.update(btn) {
                        Some(1) => { info!("Resetting at user direction via busy menu"); reset_parsers_state(&mut states) }
                        _ => (),
                    } }
                };
                menu(&states, & mut idle_menu, & mut busy_menu);
                trace!("Button done");
            }
            io::Event::Ticker => {
                //trace!("Ignoring ticker event");
            },
        }
    }
}

#[repr(u8)]
#[derive(Debug)]
enum Ins {
    GetVersion,
    GetPubkey,
    Sign,
    GetVersionStr,
    Exit
}

impl From<u8> for Ins {
    fn from(ins: u8) -> Ins {
        match ins {
            0 => Ins::GetVersion,
            2 => Ins::GetPubkey,
            3 => Ins::Sign,
            0xfe => Ins::GetVersionStr,
            0xff => Ins::Exit,
            _ => panic!(),
        }
    }
}

use nanos_sdk::io::Reply;

#[inline(never)]
fn handle_apdu<'a: 'b, 'b>(io: HostIO, ins: Ins, state: &'b mut Pin<&'a mut ParsersState<'a>>) -> Result<(), Reply> {

    let comm = io.get_comm();
    if comm?.rx == 0 {
        return Err(io::StatusWords::NothingReceived.into());
    }

    match ins {
        Ins::GetVersion => {

        }
        Ins::GetPubkey => {
            poll_apdu_handler(state, io, GetAddress)?
        }
        Ins::Sign => {
            poll_apdu_handler(state, io, Sign)?
        }
        Ins::GetVersionStr => {
        }
        Ins::Exit => nanos_sdk::exit_app(0),
    }
    Ok(())
}
