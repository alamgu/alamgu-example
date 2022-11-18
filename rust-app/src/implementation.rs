use arrayvec::ArrayVec;
use core::fmt::Write;
use core::iter::FromIterator;
use ledger_crypto_helpers::hasher::{Hash, Hasher, SHA256, Blake2b};
use ledger_crypto_helpers::common::{try_option, Address};
use ledger_crypto_helpers::eddsa::{eddsa_sign, with_public_keys, ed25519_public_key_bytes, Ed25519RawPubKeyAddress};
use ledger_log::{info};
use ledger_prompts_ui::{final_accept_prompt, ScrollerError, PromptWrite};

use nanos_sdk::io;

use core::convert::TryFrom;
use zeroize::Zeroizing;

type PKH = Ed25519RawPubKeyAddress;

type PlatformHash = Zeroizing<Hash<32>>;
type PlatformHasher = SHA256;
type ChainHash = Zeroizing<Hash<32>>;
type ChainHasher = Blake2b;

// The hash of the chain of template hashes; this is the definition of what transactions the app
// will accept.
const TEMPLATES_CHAIN_HASH : Hash<32> = Hash([0; 32]);

// A couple type ascription functions to help the compiler along.
const fn mkfn<A,B,C>(q: fn(&A,&mut B)->C) -> fn(&A,&mut B)->C {
  q
}
const fn mkmvfn<A,B,C>(q: fn(A,&mut B)->Option<C>) -> fn(A,&mut B)->Option<C> {
    q
}
/*
const fn mkvfn<A>(q: fn(&A,&mut Option<()>)->Option<()>) -> fn(&A,&mut Option<()>)->Option<()> {
    q
}
*/

#[cfg(not(target_os = "nanos"))]
#[inline(never)]
fn scroller < F: for <'b> Fn(&mut PromptWrite<'b, 16>) -> Result<(), ScrollerError> > (title: &str, prompt_function: F) -> Option<()> {
    ledger_prompts_ui::write_scroller_three_rows(title, prompt_function)
}

#[cfg(target_os = "nanos")]
#[inline(never)]
fn scroller < F: for <'b> Fn(&mut PromptWrite<'b, 16>) -> Result<(), ScrollerError> > (title: &str, prompt_function: F) -> Option<()> {
    ledger_prompts_ui::write_scroller(title, prompt_function)
}

pub fn run_get_pubkey_apdu(comm: &mut io::Comm) -> Result<(), io::Reply> {
    let slice = comm.get_data()?;

    let path = ArrayVec::<u32, 10>::from_iter(slice[1..].as_chunks::<4>().0[0..core::cmp::min(10, slice[0]) as usize].iter().map(|a|u32::from_le_bytes(*a)));

    with_public_keys(&path, |key: &_, pkh: &PKH| { try_option(|| -> Option<()> {
        scroller("Provide Public Key", |w| Ok(write!(w, "For Address     {}", pkh)?))?;

        final_accept_prompt(&[])?;

        // Should return the format that the chain customarily uses for public keys; for
        // ed25519 that's usually r | s with no prefix, which isn't quite our internal
        // representation.
        let key_bytes = ed25519_public_key_bytes(key);

        comm.append(&[u8::try_from(key_bytes.len()).ok()?]);
        comm.append(key_bytes);

        // And we'll send the address along; in our case it happens to be the same as the
        // public key, but in general it's something computed from the public key.
        let binary_address = pkh.get_binary_address();
        comm.append(&[u8::try_from(binary_address.len()).ok()?]);
        comm.append(binary_address);
        Some(())
    }())}).or(Err(io::StatusWords::Unknown.into()))
}

#[repr(u8)]
pub enum Section {
    UserMessage(u8, [u8; 200]),
    LiteralText(u8, [u8; 200]),
    FixedLengthSubstitution(u8, u8, [u8; 16], [u8; 183]),
}

impl Section {
    pub fn from_bytes(slice: &[u8]) -> Option<Section> {
        if slice.len()!=core::mem::size_of::<Section>()
           || slice[0] as usize > core::mem::variant_count::<Section>() {
            None
        } else {
            let section = unsafe { core::mem::transmute(<[u8; 202]>::try_from(slice).ok()?) };
            let valid = match section {
                Section::UserMessage(n, _) => n < 200,
                Section::LiteralText(n, _) => n < 200,
                Section::FixedLengthSubstitution(substitution_len, title_len, _, _) => substitution_len < 183 && title_len < 16,
            };
            if valid {
                Some(section)
            } else {
                None
            }
        }
    }
    pub fn as_bytes(&self) -> &[u8;202] {
        unsafe { core::mem::transmute::<&Self, &[u8; 202]>(self) }
    }
    pub fn sig_bytes(&self) -> &[u8] {
        match self {
            Section::UserMessage(len, message) => &[],
            Section::LiteralText(len, message) => &message[0..*len as usize],
            Section::FixedLengthSubstitution(len, _, _, substitution) => &substitution[0..*len as usize],
        }
    }
    pub fn checksum_bytes(&self) -> &[u8] {
        match self {
            Section::UserMessage(_, _) => &self.as_bytes()[..],
            Section::LiteralText(_, _) => &self.as_bytes()[..],
            Section::FixedLengthSubstitution(substitution_len, title_len, title, message) => &self.as_bytes()[0..18],
        }
    }
    pub fn prompt(&self) -> Option<()> {
        // Show the message or substitution as appropriate. Don't show literal text at all.
        Some(())
    }
}

fn sign(_comm: &mut io::Comm, _bip32: &[u32], _chain_hash: ChainHash) {
    // Run the signature algorithm on the hash for this bip32 path, send the signature to the host.
}

pub fn run_sign_apdu(states: &mut ParsersState, comm: &mut io::Comm) -> Result<(), io::Reply> {
    let slice = comm.get_data()?;
    let state = get_sign_state(states);

    match state {
        SignState::HashingBody { ref mut template_checksum, ref mut chain_hash } => {
            if comm.get_p1() == 0 {
                let section = Section::from_bytes(slice).ok_or(io::StatusWords::Unknown)?;
                template_checksum.update(section.checksum_bytes());
                chain_hash.update(section.sig_bytes());
                section.prompt().ok_or(io::StatusWords::Unknown)?;
            } else {
                *state = SignState::ChecksumValidation {
                    template_checksum: template_checksum.finalize(),
                    template_list_checksum: PlatformHasher::new(),
                    chain_hash: chain_hash.finalize(),
                    found: false,
                };
            }
            Ok(())
        }
        SignState::ChecksumValidation { template_checksum, ref mut template_list_checksum, chain_hash, found } => {
            // Payload is a platform checksum to compare against.
            if comm.get_p1() == 0 {
                if slice.len() != 32 { Err(io::StatusWords::Unknown)?; }
                *found |= template_checksum.0 == &slice[0..32];
                template_list_checksum.update(&slice[0..32]);
            } else {
                if !*found || template_list_checksum.finalize().0 != TEMPLATES_CHAIN_HASH.0 { Err(io::StatusWords::Unknown)?; }
                *state = SignState::Signing { chain_hash: chain_hash.clone() };
            }
            Ok(())
        }
        SignState::Signing { chain_hash } => {
            let bip32 = ArrayVec::<u32, 10>::from_iter(slice[1..].as_chunks::<4>().0[0..core::cmp::min(10, slice[0] as usize)].iter().map(|a|u32::from_le_bytes(*a)));
            sign(comm, &bip32, chain_hash.clone());
            *states = ParsersState::NoState;
            Ok(())
        }
    }
}

// PlatformHasher and PlatformHash are a hash used for the protocol of sending Sections and
// building a transaction; ChainHasher and ChainHash is the blockchain-specific hashing for a
// signature. Don't use raw ed25519 on the message.

pub enum SignState {
    HashingBody {
        template_checksum: PlatformHasher,
        chain_hash: ChainHasher,
    },
    ChecksumValidation {
        template_checksum: PlatformHash,
        template_list_checksum: PlatformHasher,
        chain_hash: ChainHash,
        found: bool,
    },
    Signing {
        chain_hash: ChainHash
    },
}


// The global parser state enum; any parser above that'll be used as the implementation for an APDU
// must have a field here.

pub enum ParsersState {
    NoState,
    GetAddressState,
    SignState(SignState),
}

pub fn reset_parsers_state(state: &mut ParsersState) {
    *state = ParsersState::NoState;
}

#[inline(never)]
pub fn get_sign_state(
    s: &mut ParsersState,
) -> &mut SignState {
    match s {
        ParsersState::SignState(_) => {}
        _ => {
            info!("Non-same state found; initializing state.");
            *s = ParsersState::SignState(SignState::HashingBody { template_checksum: PlatformHasher::new(), chain_hash: ChainHasher::new() });
        }
    }
    match s {
        ParsersState::SignState(ref mut a) => a,
        _ => {
            panic!("")
        }
    }
}
