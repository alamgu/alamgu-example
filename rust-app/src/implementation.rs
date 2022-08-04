use crate::interface::*;
use arrayvec::ArrayVec;
use core::fmt::Write;
use ledger_crypto_helpers::hasher::{Hasher, Blake2b};
use ledger_crypto_helpers::common::{with_public_keys, PKH, public_key_bytes};
use ledger_crypto_helpers::eddsa::eddsa_sign;
use ledger_parser_combinators::interp_parser::{
    Action, DefaultInterp, DropInterp, ObserveBytes, SubInterp,
};
use pin_project::pin_project;
use core::future::Future;
use ledger_parser_combinators::async_parser::*;

use ledger_prompts_ui::{write_scroller, final_accept_prompt};

use core::convert::TryFrom;
use core::ops::Deref;

use core::task::*;
use ledger_async_block::*;
use core::pin::Pin;

// A couple type ascription functions to help the compiler along.
const fn mkfn<A,B,C>(q: fn(&A,&mut B)->C) -> fn(&A,&mut B)->C {
    q
}
const fn mkmvfn<A,B,C>(q: fn(A,&mut B)->Option<C>) -> fn(A,&mut B)->Option<C> {
    q
}
const fn mkvfn<A>(q: fn(&A,&mut Option<()>)->Option<()>) -> fn(&A,&mut Option<()>)->Option<()> {
    q
}
const fn mkfinfun(q: fn(ArrayVec<u32, 10>) -> Option<ArrayVec<u8, 128>>) -> fn(ArrayVec<u32, 10>) -> Option<ArrayVec<u8, 128>> {
    q
}

pub type GetAddressImplT = impl AsyncParser<Bip32Key, ByteStream> + HasOutput<Bip32Key, Output=ArrayVec<u8, 128>>; // Returning = ArrayVec<u8, 260_usize>>;

pub const GET_ADDRESS_IMPL: GetAddressImplT =
    Action(SubInterp(DefaultInterp), mkfinfun(|path: ArrayVec<u32, 10>| -> Option<ArrayVec<u8, 128>> {
        with_public_keys(&path, |key: &_, pkh: &PKH| {

            // At this point we have the value to send to the host; but there's a bit more to do to
            // ask permission from the user.
            write_scroller("Provide Public Key", |w| Ok(write!(w, "For Address     {}", pkh)?))?;

            final_accept_prompt(&[])?;

            let key_bytes = public_key_bytes(key);
            let mut rv = ArrayVec::new();
            rv.try_push(u8::try_from(key_bytes.len()).ok()?).ok()?;
            rv.try_extend_from_slice(key_bytes).ok()?;
            rv.try_push(u8::try_from(pkh.0.len()).ok()?).ok()?;
            rv.try_extend_from_slice(&pkh.0).ok()?;
            Ok(rv)
        }).ok()
    }));

#[derive(Copy, Clone)]
pub struct GetAddress; // (pub GetAddressImplT);

impl AsyncAPDU for GetAddress {
    // const MAX_PARAMS : usize = 1;
    type State<'c> = impl Future<Output = ()>;

    fn run<'c>(self, io: HostIO, input: ArrayVec<ByteStream, MAX_PARAMS >) -> Self::State<'c> {
        let mut param = input[0].clone();
        async move {
            let address = GET_ADDRESS_IMPL.parse(&mut param).await;
            io.result_final(&address).await;
        }
    }
}

impl<'d> AsyncAPDUStated<ParsersStateCtr> for GetAddress {
    #[inline(never)]
    fn init<'a, 'b: 'a>(
        self,
        s: &mut core::pin::Pin<&'a mut ParsersState<'a>>,
        io: HostIO,
        input: ArrayVec<ByteStream, MAX_PARAMS>
    ) -> () {
        s.set(ParsersState::GetAddressState(self.run(io, input)));
    }

    /*
    #[inline(never)]
    fn get<'a, 'b>(self, s: &'b mut core::pin::Pin<&'a mut ParsersState<'a>>) -> Option<&'b mut core::pin::Pin<&'a mut Self::State<'a>>> {
        match s.as_mut().project() {
            ParsersStateProjection::GetAddressState(ref mut s) => Some(s),
            _ => panic!("Oops"),
        }
    }*/

    #[inline(never)]
    fn poll<'a, 'b>(self, s: &mut core::pin::Pin<&'a mut ParsersState>) -> core::task::Poll<()> {
        let waker = unsafe { Waker::from_raw(RawWaker::new(&(), &RAW_WAKER_VTABLE)) };
        let mut ctxd = Context::from_waker(&waker);
        match s.as_mut().project() {
            ParsersStateProjection::GetAddressState(ref mut s) => s.as_mut().poll(&mut ctxd),
            _ => panic!("Ooops"),
        }
    }
}

#[derive(Copy, Clone)]
pub struct Sign;

// Transaction parser; this should prompt the user a lot more than this.

const TXN_PARSER : impl AsyncParser<Transaction, ByteStream> + HasOutput<Transaction, Output = [u8; 32]> =
    Action(
        // Calculate the hash of the transaction
        ObserveBytes(Blake2b::new, Blake2b::update, DropInterp),
        // Ask the user if they accept the transaction body's hash
        mkfn(|(hash, _): &(Blake2b, Option<() /*ArrayVec<(), { usize::MAX }>*/>), destination: &mut _| {
            let the_hash = hash.clone().finalize();

            write_scroller("Transaction hash?", |w| Ok(write!(w, "{}", the_hash.deref())?))?;

            *destination = Some(the_hash.0.into());
            Some(())
        })
    );

const PATH_PARSER : impl AsyncParser<Bip32Key, ByteStream> + HasOutput<Bip32Key, Output=ArrayVec<u32, 10>> =
    SubInterp(DefaultInterp);

impl AsyncAPDU for Sign {
    // const MAX_PARAMS : usize = 2;

    type State<'c> = impl Future<Output = ()>;

    fn run<'c>(self, io: HostIO, mut input: ArrayVec<ByteStream, MAX_PARAMS>) -> Self::State<'c> {
        async move {
            let hash = TXN_PARSER.parse(&mut input[0]).await;

            let path = PATH_PARSER.parse(&mut input[1]).await;

            with_public_keys(&path, |_, pkh : &PKH| {
                write_scroller("Sign for Address", |w| Ok(write!(w, "{}", pkh)?))?;
                Ok(())
            }).unwrap();

            let sig = eddsa_sign(&path, &hash[..]).unwrap();

            io.result_final(&sig.0).await;
        }
    }
}

impl<'d> AsyncAPDUStated<ParsersStateCtr> for Sign {
    #[inline(never)]
    fn init<'a, 'b: 'a>(
        self,
        s: &mut core::pin::Pin<&'a mut ParsersState<'a>>,
        io: HostIO,
        input: ArrayVec<ByteStream, MAX_PARAMS>
    ) -> () {
        s.set(ParsersState::SignState(self.run(io, input)));
    }

    #[inline(never)]
    fn poll<'a>(self, s: &mut core::pin::Pin<&'a mut ParsersState>) -> core::task::Poll<()> {
        let waker = unsafe { Waker::from_raw(RawWaker::new(&(), &RAW_WAKER_VTABLE)) };
        let mut ctxd = Context::from_waker(&waker);
        match s.as_mut().project() {
            ParsersStateProjection::SignState(ref mut s) => s.as_mut().poll(&mut ctxd),
            _ => panic!("Ooops"),
        }
    }
}

// The global parser state enum; any parser above that'll be used as the implementation for an APDU
// must have a field here.

// type GetAddressStateType = impl Future;
// type SignStateType = impl Future<Output = ()>;

#[pin_project(project = ParsersStateProjection)]
pub enum ParsersState<'a> {
    NoState,
    GetAddressState(#[pin] <GetAddress as AsyncAPDU>::State<'a>), // <GetAddressImplT<'a> as AsyncParser<Bip32Key, ByteStream<'a>>>::State<'a>),
    SignState(#[pin] <Sign as AsyncAPDU>::State<'a>),
    // SignState(#[pin] <SignImplT<'a> as AsyncParser<SignParameters, ByteStream<'a>>>::State<'a>),
}

impl Default for ParsersState<'_> {
    fn default() -> Self {
        ParsersState::NoState
    }
}

// we need to pass a type constructor for ParsersState to various places, so that we can give it
// the right lifetime; this is a bit convoluted, but works.

pub struct ParsersStateCtr;
impl StateHolderCtr for ParsersStateCtr {
    type StateCtr<'a> = ParsersState<'a>;
}

pub fn reset_parsers_state(state: &mut Pin<&mut ParsersState<'_>>) {
    state.set(ParsersState::default())
}

impl ParsersState<'_> {
    pub fn is_no_state(&self) -> bool {
        match self {
            ParsersState::NoState => true,
            _ => false,
        }
    }
}
