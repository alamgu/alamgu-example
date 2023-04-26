use crate::implementation::*;
use crate::interface::*;
use crate::settings::*;
use crate::test_parsers::*;

use ledger_log::info;
use ledger_parser_combinators::interp_parser::{DynParser, ParserCommon};

// The global parser state enum; any parser above that'll be used as the implementation for an APDU
// must have a field here.
#[allow(clippy::large_enum_variant)]
#[derive(enum_init::InPlaceInit)]
pub enum ParsersState {
    NoState,
    GetAddressState(<GetAddressImplT as ParserCommon<Bip32Key>>::State),
    SignState(<SignImplT as ParserCommon<SignParameters>>::State),
    TestParsersState(<TestParsersImplT as ParserCommon<TestParsersSchema>>::State),
}

pub fn reset_parsers_state(state: &mut ParsersState) {
    *state = ParsersState::NoState;
}

#[inline(never)]
pub fn get_get_address_state<const PROMPT: bool>(
    s: &mut ParsersState,
) -> &mut <GetAddressImplT as ParserCommon<Bip32Key>>::State {
    match s {
        ParsersState::GetAddressState(_) => {}
        _ => {
            info!("Non-same state found; initializing state.");
            *s = ParsersState::GetAddressState(<GetAddressImplT as ParserCommon<Bip32Key>>::init(
                &get_address_impl::<PROMPT>(),
            ));
        }
    }
    match s {
        ParsersState::GetAddressState(ref mut a) => a,
        _ => {
            panic!("")
        }
    }
}

#[inline(never)]
pub fn get_sign_state(
    s: &mut ParsersState,
    settings: Settings,
) -> &mut <SignImplT as ParserCommon<SignParameters>>::State {
    match s {
        ParsersState::SignState(_) => {}
        _ => {
            info!("Non-same state found; initializing state.");
            ParsersState::init_sign_state(s as *mut _ as *mut _, |state| {
                let s = state as *mut _;
                <SignImplT as DynParser<SignParameters>>::init_param(
                    &SIGN_IMPL,
                    settings,
                    unsafe { &mut *s },
                    &mut None,
                )
            });
        }
    }
    match s {
        ParsersState::SignState(ref mut a) => a,
        _ => {
            panic!("")
        }
    }
}

#[inline(never)]
pub fn get_test_parsers_state(
    s: &mut ParsersState,
) -> &mut <TestParsersImplT as ParserCommon<TestParsersSchema>>::State {
    match s {
        ParsersState::TestParsersState(_) => {}
        _ => {
            info!("Non-same state found; initializing state.");
            *s = ParsersState::TestParsersState(<TestParsersImplT as ParserCommon<
                TestParsersSchema,
            >>::init(&test_parsers_parser()));
        }
    }
    match s {
        ParsersState::TestParsersState(ref mut a) => a,
        _ => {
            panic!("")
        }
    }
}
