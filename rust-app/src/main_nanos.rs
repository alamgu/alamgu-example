use crate::implementation::*;
use crate::interface::*;

use alamgu_async_block::*;

use ledger_log::{info, trace};
use ledger_parser_combinators::interp_parser::OOB;
use ledger_prompts_ui::RootMenu;

use nanos_sdk::io;
use pin_cell::*;
use core::cell::RefCell;
use core::pin::Pin;


// We are single-threaded in fact, albeit with nontrivial code flow. We don't need to worry about
// full atomicity of the below globals.
struct SingleThreaded<T>(T);
unsafe impl<T> Send for SingleThreaded<T> { }
unsafe impl<T> Sync for SingleThreaded<T> { }
impl<T> core::ops::Deref for SingleThreaded<T> {
    type Target = T;
    fn deref(&self) -> &T { &self.0 }
}
impl<T> core::ops::DerefMut for SingleThreaded<T> {
    fn deref_mut(&mut self) -> &mut T { &mut self.0 }
}

/*static COMM : SingleThreaded<RefCell<io::Comm>> = SingleThreaded(RefCell::new(io::Comm::new()));
static HOSTIO_STATE : SingleThreaded<RefCell<HostIOState>> = SingleThreaded(RefCell::new(HostIOState::new(&COMM.0)));
static HOSTIO : SingleThreaded<HostIO> = SingleThreaded(HostIO(&HOSTIO_STATE.0));
static STATES_BACKING : SingleThreaded<PinCell<Option<APDUsFuture>>> = SingleThreaded(PinCell::new(None));
static STATES : SingleThreaded<Pin<&PinCell<Option<APDUsFuture>>>> = SingleThreaded(Pin::static_ref(&STATES_BACKING.0));
*/

#[allow(dead_code)]
pub fn app_main() {
let COMM : SingleThreaded<RefCell<io::Comm>> = SingleThreaded(RefCell::new(io::Comm::new()));
let HOSTIO_STATE : SingleThreaded<RefCell<HostIOState>> = SingleThreaded(RefCell::new(HostIOState::new(unsafe { core::mem::transmute(&COMM.0) })));
let HOSTIO : SingleThreaded<HostIO> = SingleThreaded(HostIO(unsafe { core::mem::transmute(&HOSTIO_STATE.0) }));
let STATES_BACKING : SingleThreaded<PinCell<Option<APDUsFuture>>> = SingleThreaded(PinCell::new(None));
let STATES : SingleThreaded<Pin<&PinCell<Option<APDUsFuture>>>> = SingleThreaded(Pin::static_ref(unsafe { core::mem::transmute(&STATES_BACKING.0) } ));
    /*unsafe {

        core::mem::forget(COMM.0.replace(io::Comm::new()));
        core::mem::forget(HOSTIO_STATE.0.replace(HostIOState::new(&COMM.0)));
        core::mem::transmute::<_, *mut SingleThreaded<HostIO>>(&HOSTIO as *const SingleThreaded<HostIO>).write(SingleThreaded(HostIO(&HOSTIO_STATE.0)));
        core::mem::transmute::<_, *mut SingleThreaded<PinCell<Option<APDUsFuture>>>>(&STATES_BACKING as *const SingleThreaded<PinCell<Option<APDUsFuture>>>).write(SingleThreaded(PinCell::new(None)));
        core::mem::transmute::<_, *mut SingleThreaded<Pin<&PinCell<Option<APDUsFuture>>>>>(&STATES as *const SingleThreaded<Pin<&PinCell<Option<APDUsFuture>>>>).write(SingleThreaded(Pin::static_ref(&STATES_BACKING.0)));
    }*/


    let mut idle_menu = RootMenu::new([concat!("Rust App ", env!("CARGO_PKG_VERSION")), "Exit"]);
    let mut busy_menu = RootMenu::new(["Working...", "Cancel"]);

    info!("Rust App {}", env!("CARGO_PKG_VERSION"));
    info!(
        "State sizes\ncomm: {}\nstates: {}",
        core::mem::size_of::<io::Comm>(),
        core::mem::size_of::<Option<APDUsFuture>>()
    );

    let // Draw some 'welcome' screen
        menu = |states : core::cell::Ref<'_, Option<APDUsFuture>>, idle : & mut RootMenu<2>, busy : & mut RootMenu<2>| {
            match states.is_none() {
                true => idle.show(),
                _ => busy.show(),
            }
        };

    menu(STATES.borrow(), &mut idle_menu, &mut busy_menu);
    loop {
        // Wait for either a specific button push to exit the app
        // or an APDU command
        let evt = COMM.borrow_mut().next_event::<Ins>();
        match evt {
            io::Event::Command(ins) => {
                trace!("Command received");
                let poll_rv = poll_apdu_handlers(PinMut::as_mut(&mut STATES.0.borrow_mut()), ins, *HOSTIO, (), handle_apdu_async);
                trace!("Poll complete");
                match poll_rv {
//                    handle_apdu(&mut comm, ins, &mut states) {
                    Ok(()) => {
                        trace!("APDU accepted; sending response");
                        COMM.borrow_mut().reply_ok();
                        trace!("Replied");
                    }
                    Err(sw) => {
                        trace!("Replying");
                        COMM.borrow_mut().reply(sw);
                    }
                };
                menu(STATES.borrow(), &mut idle_menu, &mut busy_menu);
                trace!("Command done");
            }
            io::Event::Button(btn) => {
                trace!("Button received");
                match STATES.borrow().is_none() {
                    true => {
                        if let Some(1) = idle_menu.update(btn) {
                            info!("Exiting app at user direction via root menu");
                            nanos_sdk::exit_app(0)
                        }
                    }
                    _ => {
                        if let Some(1) = idle_menu.update(btn) {
                            info!("Resetting at user direction via busy menu");
                            PinMut::as_mut(&mut STATES.borrow_mut()).set(None);
                        }
                    }
                };
                menu(STATES.borrow(), &mut idle_menu, &mut busy_menu);
                trace!("Button done");
            }
            io::Event::Ticker => {
                //trace!("Ignoring ticker event");
            }
        }
    }
}

/*
#[repr(u8)]
#[derive(Debug)]
enum Ins {
    GetVersion,
    GetPubkey,
    Sign,
    GetVersionStr,
    Exit,
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

use arrayvec::ArrayVec;
use nanos_sdk::io::Reply;

use ledger_parser_combinators::interp_parser::{InterpParser, ParserCommon};
fn run_parser_apdu<P: InterpParser<A, Returning = ArrayVec<u8, 128>>, A>(
    states: &mut ParsersState,
    get_state: fn(&mut ParsersState) -> &mut <P as ParserCommon<A>>::State,
    parser: &P,
    comm: &mut io::Comm,
) -> Result<(), Reply> {
    let cursor = comm.get_data()?;

    trace!("Parsing APDU input: {:?}\n", cursor);
    let mut parse_destination = None;
    let parse_rv =
        <P as InterpParser<A>>::parse(parser, get_state(states), cursor, &mut parse_destination);
    trace!("Parser result: {:?}\n", parse_rv);
    match parse_rv {
        // Explicit rejection; reset the parser. Possibly send error message to host?
        Err((Some(OOB::Reject), _)) => {
            reset_parsers_state(states);
            Err(io::StatusWords::Unknown.into())
        }
        // Deliberately no catch-all on the Err((Some case; we'll get error messages if we
        // add to OOB's out-of-band actions and forget to implement them.
        //
        // Finished the chunk with no further actions pending, but not done.
        Err((None, [])) => {
            trace!("Parser needs more; continuing");
            Ok(())
        }
        // Didn't consume the whole chunk; reset and error message.
        Err((None, _)) => {
            reset_parsers_state(states);
            Err(io::StatusWords::Unknown.into())
        }
        // Consumed the whole chunk and parser finished; send response.
        Ok([]) => {
            trace!("Parser finished, resetting state\n");
            match parse_destination.as_ref() {
                Some(rv) => comm.append(&rv[..]),
                None => return Err(io::StatusWords::Unknown.into()),
            }
            // Parse finished; reset.
            reset_parsers_state(states);
            Ok(())
        }
        // Parse ended before the chunk did; reset.
        Ok(_) => {
            reset_parsers_state(states);
            Err(io::StatusWords::Unknown.into())
        }
    }
}

#[inline(never)]
fn handle_apdu(comm: &mut io::Comm, ins: Ins, parser: &mut ParsersState) -> Result<(), Reply> {
    info!("entering handle_apdu with command {:?}", ins);
    if comm.rx == 0 {
        return Err(io::StatusWords::NothingReceived.into());
    }

    match ins {
        Ins::GetVersion => {
            comm.append(&[
                env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
                env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
                env!("CARGO_PKG_VERSION_PATCH").parse().unwrap(),
            ]);
            comm.append(b"rust app");
        }
        Ins::GetPubkey => {
            run_parser_apdu::<_, Bip32Key>(parser, get_get_address_state, &GET_ADDRESS_IMPL, comm)?
        }
        Ins::Sign => {
            run_parser_apdu::<_, SignParameters>(parser, get_sign_state, &SIGN_IMPL, comm)?
        }
        Ins::GetVersionStr => {
            comm.append(concat!("Rust App ", env!("CARGO_PKG_VERSION")).as_ref());
        }
        Ins::Exit => nanos_sdk::exit_app(0),
    }
    Ok(())
}*/
