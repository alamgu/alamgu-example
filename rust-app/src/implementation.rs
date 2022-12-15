use crate::interface::*;
use arrayvec::ArrayVec;
use core::fmt::Write;
use ledger_crypto_helpers::common::{try_option, Address, PubKey, CryptographyError};
use ledger_crypto_helpers::hasher::{Blake2b, Hash, Hasher, SHA256, Base64Hash};
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
        io.result_accumulating(&[]).await; // Trick to update the screen to "Working..."

        let path = BIP_PATH_PARSER.parse(&mut input[0].clone()).await;

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
            Some(())
        })();

        io.result_final(&rv).await;
    }
}

// Demonstrate some of the more unusual things we can do.
pub fn get_address_prompting_and_example_apdu(io: HostIO) -> impl Future<Output = ()> {
    async move {
        let input = io.get_params::<1>().unwrap();
        io.result_accumulating(&[]).await; // Trick to update the screen to "Working..."

        let path = BIP_PATH_PARSER.parse(&mut input[0].clone()).await;

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
            scroller("Reveal Address", |w| Ok(write!(w, "{}", address)?))?;
            Some(())
        })();

        let hash = io.put_chunk(&rv).await;
        let mut rv2 = ArrayVec::<u8, 220>::new();
        rv2.try_extend_from_slice(&io.get_chunk(hash).await.unwrap());

        io.result_accumulating(&rv2).await;
        io.result_final(&[]).await;
    }
}

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
        let hash: Base64Hash<32>; // Pick an appropriate hash type for the chain, or define some
                                  // TransactionID type that implements Hash<N>.

        {
            let mut txn = input[0].clone();
            hash = hasher_parser().parse(&mut txn).await.0.finalize();
            trace!("Hashed txn");
        }
        
        if scroller("Sign Transaction", |w| Ok(write!(w, "Hash: {}", hash)?)).is_none()
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
        }
        Ins::Sign => {
            trace!("Handling sign");
            sign_apdu(io).await;
        }
        Ins::RevealAddressAndExample => {
            get_address_prompting_and_example_apdu(io).await;
        }
        Ins::GetVersionStr => {
        }
        Ins::Exit => nanos_sdk::exit_app(0),
        _ => { }
    }
    }
}

