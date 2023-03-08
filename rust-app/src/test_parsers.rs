use crate::utils::*;
use alamgu_async_block::*;
use alamgu_async_block::prompts::*;
use arrayvec::ArrayVec;
use core::cell::*;
use core::fmt::Write;
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::core_parsers::*;
use ledger_parser_combinators::endianness::*;
use ledger_parser_combinators::interp::*;

// Try out all possible param types
pub type TestParsersSchema = ((BytesParams, U16Params), (U64Params, DArrayParams));

pub type BytesParams = (Byte, Array<Byte, 32>);
pub type U16Params = (U16<{ Endianness::Big }>, U16<{ Endianness::Little }>);
pub type U32Params = (U32<{ Endianness::Big }>, U32<{ Endianness::Little }>);
pub type U64Params = (U64<{ Endianness::Big }>, U64<{ Endianness::Little }>);
pub type DArrayParams = (DArray<Byte, Byte, 24>, DArray<Byte, U32Params, 4>);

pub type TestParsersImplT<'a, BS: 'a + Readable> =
impl AsyncParser<TestParsersSchema, BS> + HasOutput<TestParsersSchema, Output = ()> + 'a;
pub fn test_parsers_parser<'a, BS: 'a + Readable>(pq1: &'a RefCell<PromptQueue>, pq2: &'a RefCell<PromptQueue>) -> TestParsersImplT<'a, BS> {
    FutAction(
        (
            (bytes_params_parser(pq2), u16_params_parser(pq1)),
            (u64_params_parser(pq2), darray_params_parser(pq1)),
        ),
        move |_| async move {
            pq2.borrow_mut().add_prompt("Parse done", format_args!("")).await.ok()
        },
    )
}

pub type BytesParamsT<'a, BS: 'a + Readable> =
    impl AsyncParser<BytesParams, BS> + HasOutput<BytesParams, Output = ()> + 'a;
const fn bytes_params_parser<'a, BS: 'a + Readable>(pq: &'a RefCell<PromptQueue>) -> BytesParamsT<'a, BS> {
    FutAction(
        (DefaultInterp, DefaultInterp),
        move |(v1, v2): (u8, [u8; 32])| async move {
            pq.borrow_mut().add_prompt("Got Bytes", format_args!("v1: {v1:?}, v2: {v2:02x?}")).await.ok()
        },
    )
}

pub type U16ParamsT<'a, BS: 'a + Readable> =
    impl AsyncParser<U16Params, BS> + HasOutput<U16Params, Output = ()> + 'a;
const fn u16_params_parser<'a, BS: 'a + Readable>(pq: &'a RefCell<PromptQueue>) -> U16ParamsT<'a, BS> {
    FutAction((DefaultInterp, DefaultInterp), move |(v1, v2): (u16, u16)| async move {
        pq.borrow_mut().add_prompt("Got U16", format_args!("v1: {v1:?}, v2: {v2:?}")).await.ok()
    })
}

pub type U32ParamsT<'a, BS: 'a + Readable> =
    impl AsyncParser<U32Params, BS> + HasOutput<U32Params, Output = ()> + 'a;
const fn u32_params_parser<'a, BS: 'a + Readable>(pq: &'a RefCell<PromptQueue>) -> U32ParamsT<'a, BS> {
    FutAction((DefaultInterp, DefaultInterp), move |(v1, v2): (u32, u32)| async move {
        pq.borrow_mut().add_prompt("Got U32", format_args!("v1: {v1:?}, v2: {v2:?}")).await.ok()
    })
}

pub type U64ParamsT<'a, BS: 'a + Readable> =
    impl AsyncParser<U64Params, BS> + HasOutput<U64Params, Output = ()> + 'a;
const fn u64_params_parser<'a, BS: 'a + Readable>(pq: &'a RefCell<PromptQueue>) -> U64ParamsT<'a, BS> {
    FutAction((DefaultInterp, DefaultInterp), move |(v1, v2): (u64, u64)| async move {
        pq.borrow_mut().add_prompt("Got U64", format_args!("v1: {v1:?}, v2: {v2:?}")).await.ok()
    })
}

pub type DArrayParamsT<'a, BS: 'a + Readable> =
    impl AsyncParser<DArrayParams, BS> + HasOutput<DArrayParams, Output = ()> + 'a;
const fn darray_params_parser<'a, BS: 'a + Readable>(pq: &'a RefCell<PromptQueue>) -> DArrayParamsT<'a, BS> {
    FutAction(
        (SubInterp(DefaultInterp), SubInterp(u32_params_parser(pq))),
        move |(v1, _v2): (ArrayVec<u8, 24>, ArrayVec<(), 4>)| async move {
            pq.borrow_mut().add_prompt("Got Darray", format_args!("v1: {v1:02x?}")).await.ok()
        },
    )
}

pub async fn test_parsers2(io: HostIO) {
    let input = io.get_params::<1>().unwrap();
    let pq1 = RefCell::new(PromptQueue::new(io));
    let pq2 = RefCell::new(PromptQueue::new(io));
    // NoinlineFut((|mut bs: ByteStream, pq1: RefCell<PromptQueue>, pq2: RefCell<PromptQueue>| async move {
    //     {
    //         test_parsers_parser(&pq1, &pq2).parse(&mut bs).await;
    //     }
    // })(input[0].clone(), pq1, pq2))
    //     .await;
    test_parsers_parser(&pq1, &pq2).parse(&mut input[0].clone()).await;
    // NoinlineFut((|pq: RefCell<PromptQueue>| async move {
    //     {
    //         if pq.borrow_mut().show().await.ok() != Some(true) {
    //             reject::<()>().await;
    //         }
    //     }
    // })(pq1));
    if pq1.borrow_mut().show().await.ok() != Some(true) {
        reject::<()>().await;
    }
    if pq2.borrow_mut().show().await.ok() != Some(true) {
        reject::<()>().await;
    }
    io.result_final(&[]).await;
}

// pub type TestParsersImplT2<BS: Readable> =
// impl AsyncParser<TestParsersSchema, BS> + HasOutput<TestParsersSchema, Output = ()> + 'a;
pub async fn test_parsers(io: HostIO) {
    let input = io.get_params::<1>().unwrap();
    let pq1 = RefCell::new(PromptQueue::new(io));
    let pq2 = RefCell::new(PromptQueue::new(io));

    let parser =
        FutAction(
            (
                (bytes_params_parser(&pq2), u16_params_parser(&pq1)),
                (u64_params_parser(&pq2), darray_params_parser(&pq1)),
            ),
            move |_| async move {
                // &pq2.borrow_mut().add_prompt("Parse done", format_args!("")).await.ok()
                Some(())
            },
        );
    parser.parse(&mut input[0].clone()).await;
    if pq1.borrow_mut().show().await.ok() != Some(true) {
        reject::<()>().await;
    }
    if pq2.borrow_mut().show().await.ok() != Some(true) {
        reject::<()>().await;
    }
    io.result_final(&[]).await;
}
