use crate::interface::*;
use arrayvec::ArrayVec;
use core::fmt::Write;
use ledger_crypto_helpers::common::{try_option, Address, PubKey, CryptographyError};
use ledger_crypto_helpers::hasher::{Blake2b, Hash, Hasher, SHA256};
use ledger_log::info;
use nanos_sdk::ecc::{ECPublicKey, Secp256k1};
/* use ledger_parser_combinators::interp_parser::{
    Action, DefaultInterp, DropInterp, InterpParser, MoveAction, ObserveBytes, ParserCommon,
    SubInterp,
}; */
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::interp::*;
use alamgu_async_block::*;
use ledger_prompts_ui::{final_accept_prompt, PromptWrite, ScrollerError};

use core::convert::TryFrom;
use core::ops::Deref;
use zeroize::Zeroizing;
use ledger_log::*;
use core::future::Future;

#[allow(clippy::upper_case_acronyms)]
type Addr = PubKey<65, 'W'>;


pub type BipParserImplT = impl AsyncParser<Bip32Key, ByteStream> + HasOutput<Bip32Key, Output = ArrayVec<u32, 10>>;
pub const BIP_PATH_PARSER: BipParserImplT = SubInterp(DefaultInterp);


pub fn get_address_apdu(io: HostIO) -> impl Future<Output = ()> {
    async move {
        let input = io.get_params::<1>().unwrap();
        io.result_accumulating(&[]).await;
        error!("Doing getAddress");

        let path = BIP_PATH_PARSER.parse(&mut input[0].clone()).await;

        error!("Got path");

        let mut rv = ArrayVec::<u8, 220>::new();
        
        (|| -> Option<()> {
            let pubkey = Secp256k1::from_bip32(&path).public_key().ok()?;
            let address = PubKey::get_address(&pubkey).ok()?;
            rv.push((pubkey.keylength) as u8);
            let _ = rv.try_extend_from_slice(&pubkey.pubkey[0..pubkey.keylength]).unwrap();
            let mut temp_fmt = arrayvec::ArrayString::<128>::new();
            write!(temp_fmt, "{}", address).unwrap();
            rv.push(temp_fmt.as_bytes().len() as u8);
            rv.try_extend_from_slice(temp_fmt.as_bytes()).unwrap();
            scroller("With PKH", |w| Ok(write!(w, "{}", address)?))?;
            Some(())
        })();

        trace!("Stashing result");
        let hash = io.put_chunk(&rv).await;
        trace!("Sending result back");
        scroller("With PKH", |w| Ok(write!(w, "noodle")?));
        let mut rv2 = ArrayVec::<u8, 220>::new();
        rv2.try_extend_from_slice(&io.get_chunk(hash).await.unwrap());
        io.result_accumulating(&rv2).await;

        trace!("Accumulated result");
        io.result_final(&[]).await;
        trace!("Sent");
    }
}

// A couple type ascription functions to help the compiler along.
const fn mkfn<A, B, C>(q: fn(&A, &mut B) -> C) -> fn(&A, &mut B) -> C {
    q
}
const fn mkmvfn<A, B, C>(q: fn(A, &mut B) -> Option<C>) -> fn(A, &mut B) -> Option<C> {
    q
}
/*
const fn mkvfn<A>(q: fn(&A,&mut Option<()>)->Option<()>) -> fn(&A,&mut Option<()>)->Option<()> {
    q
}
*/

#[cfg(not(target_os = "nanos"))]
#[inline(never)]
fn scroller<F: for<'b> Fn(&mut PromptWrite<'b, 16>) -> Result<(), ScrollerError>>(
    title: &str,
    prompt_function: F,
) -> Option<()> {
    ledger_prompts_ui::write_scroller_three_rows(title, prompt_function)
}

#[cfg(target_os = "nanos")]
#[inline(never)]
fn scroller<F: for<'b> Fn(&mut PromptWrite<'b, 16>) -> Result<(), ScrollerError>>(
    title: &str,
    prompt_function: F,
) -> Option<()> {
    ledger_prompts_ui::write_scroller(title, prompt_function)
}

type HasherParser = impl AsyncParser<Transaction, ByteStream> + HasOutput<Transaction, Output=(SHA256, Option<()>)>;
const fn hasher_parser() -> HasherParser { ObserveBytes(SHA256::new, SHA256::update, DropInterp) }

pub fn sign_apdu(io: HostIO) -> impl Future<Output = ()> {
    async move {
        let mut input = io.get_params::<2>().unwrap();
        io.result_accumulating(&[]).await; // Trick to display the "Working..." message; we should have a
                                     // real way to do this.
        let hash;

        {
            let mut txn = input[0].clone();
            hash = hasher_parser().parse(&mut txn).await.0.finalize();
            trace!("Hashed txn");
        }
        
        if scroller("Sign Transaction", |w| Ok(write!(w, "Hash: {}", *hash)?)).is_none()
            { reject::<()>().await; }

        let path = BIP_PATH_PARSER.parse(&mut input[1].clone()).await;

        if let Some((sig, sig_len)) = {
            let sk = Secp256k1::from_bip32(&path);
            let prompt_fn = || {
                let pkh = PubKey::get_address(&sk.public_key().ok()?).ok()?;
                scroller("With PKH", |w| Ok(write!(w, "{}", pkh)?))?;
                final_accept_prompt(&[])
            };
            if prompt_fn().is_none() { reject::<()>().await; }
            sk.deterministic_sign(&hash.0[..]).ok()
        } {
            // io.result_final(&sig[0..sig_len as usize]).await;
            io.result_final(&[]).await;
        } else {
            reject::<()>().await;
        }
    }
}

/*
pub type SignImplT = impl InterpParser<SignParameters, Returning = ArrayVec<u8, 128>>;

pub static SIGN_IMPL: SignImplT = Action(
    (
        Action(
            // Calculate the hash of the transaction
            ObserveBytes(Hasher::new, Hasher::update, SubInterp(DropInterp)),
            // Ask the user if they accept the transaction body's hash
            mkfn(
                |(mut hasher, _): &(Blake2b, _), destination: &mut Option<Zeroizing<Hash<32>>>| {
                    let the_hash = hasher.finalize();
                    scroller("Transaction hash", |w| {
                        Ok(write!(w, "{}", the_hash.deref())?)
                    })?;
                    *destination = Some(the_hash);
                    Some(())
                },
            ),
        ),
        MoveAction(
            SubInterp(DefaultInterp),
            // And ask the user if this is the key the meant to sign with:
            mkmvfn(
                |path: ArrayVec<u32, 10>, destination: &mut Option<ArrayVec<u32, 10>>| {
                    with_public_keys(&path, |_, pkh: &PKH| {
                        try_option(|| -> Option<()> {
                            scroller("Sign for Address", |w| Ok(write!(w, "{pkh}")?))?;
                            Some(())
                        }())
                    })
                    .ok()?;
                    *destination = Some(path);
                    Some(())
                },
            ),
        ),
    ),
    mkfn(
        |(hash, path): &(Option<Zeroizing<Hash<32>>>, Option<ArrayVec<u32, 10>>),
         destination: &mut _| {
            final_accept_prompt(&["Sign Transaction?"])?;

            // By the time we get here, we've approved and just need to do the signature.
            let sig = eddsa_sign(path.as_ref()?, &hash.as_ref()?.0[..]).ok()?;
            let mut rv = ArrayVec::<u8, 128>::new();
            rv.try_extend_from_slice(&sig.0[..]).ok()?;
            *destination = Some(rv);
            Some(())
        },
    ),
);
*/
pub type APDUsFuture = impl Future<Output = ()>;

#[inline(never)]
pub fn handle_apdu_async(io: HostIO, ins: Ins) -> APDUsFuture {
    trace!("Constructing future");
    async move {
        trace!("Dispatching");
    match ins {
        Ins::GetVersion => {

        }
        Ins::GetPubkey => {
            get_address_apdu(io).await;
            trace!("APDU complete");
            // run_fut(trampoline(), move || get_address_apdu(io)).await
        }
        Ins::Sign => {
            trace!("Handling sign");
            sign_apdu(io).await;
        }
        Ins::GetVersionStr => {
        }
        Ins::Exit => nanos_sdk::exit_app(0),
        _ => { }
    }
    }
}

