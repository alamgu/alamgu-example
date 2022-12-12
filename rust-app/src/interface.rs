use ledger_parser_combinators::core_parsers::*;
use ledger_parser_combinators::endianness::*;
use core::convert::TryFrom;

// Payload for a public key request
pub type Bip32Key = DArray<Byte, U32<{ Endianness::Little }>, 10>;

pub type Transaction = DArray<U32<{ Endianness::Little }>, Byte, { usize::MAX }>;

// Payload for a signature request, content-agnostic.
pub type SignParameters = (
    Transaction,
    Bip32Key,
);

#[repr(u8)]
#[derive(Debug)]
pub enum Ins {
    GetVersion,
    GetPubkey,
    Sign,
    GetVersionStr,
    Exit,
}

impl TryFrom<u8> for Ins {
    type Error = ();
    fn try_from(ins: u8) -> Result<Ins, ()>{
        Ok(match ins {
            0 => Ins::GetVersion,
            2 => Ins::GetPubkey,
            3 => Ins::Sign,
            0xfe => Ins::GetVersionStr,
            0xff => Ins::Exit,
            _ => return Err(()),
        })
    }
}
