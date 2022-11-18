use crate::implementation::*;

use ledger_prompts_ui::RootMenu;
use ledger_log::{info, trace};

use nanos_sdk::io;

#[allow(dead_code)]
pub fn app_main() {
    let mut comm = io::Comm::new();
    let mut states = ParsersState::NoState;

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
        match comm.next_event::<Ins>() {
            io::Event::Command(ins) => {
                trace!("Command received");
                match handle_apdu(&mut comm, ins, &mut states) {
                    Ok(()) => {
                        trace!("APDU accepted; sending response");
                        comm.reply_ok();
                        trace!("Replied");
                    }
                    Err(sw) => comm.reply(sw),
                };
                menu(&states, & mut idle_menu, & mut busy_menu);
                trace!("Command done");
            }
            io::Event::Button(btn) => {
                trace!("Button received");
                match states {
                    ParsersState::NoState => {match idle_menu.update(btn) {
                        Some(1) => { info!("Exiting app at user direction via root menu"); nanos_sdk::exit_app(0) },
                        _ => (),
                    } }
                    _ => { match busy_menu.update(btn) {
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
fn handle_apdu(comm: &mut io::Comm, ins: Ins, parser: &mut ParsersState) -> Result<(), Reply> {
    info!("entering handle_apdu with command {:?}", ins);
    if comm.rx == 0 {
        return Err(io::StatusWords::NothingReceived.into());
    }

    match ins {
        Ins::GetVersion => {
            comm.append(&[env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(), env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(), env!("CARGO_PKG_VERSION_PATCH").parse().unwrap()]);
            comm.append(b"rust app");
        }
        Ins::GetPubkey => {
            run_get_pubkey_apdu(comm)?
        }
        Ins::Sign => {
            run_sign_apdu(parser, comm)?
        }
        Ins::GetVersionStr => {
            comm.append(concat!("Rust App ", env!("CARGO_PKG_VERSION")).as_ref());
        }
        Ins::Exit => nanos_sdk::exit_app(0),
    }
    Ok(())
}
