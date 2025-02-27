#![cfg(feature = "dnssec-openssl")]

use std::str::FromStr;
use std::sync::{Arc, Mutex as StdMutex};

use futures::executor::block_on;
use tokio::runtime::Runtime;

use hickory_client::client::{Client, ClientHandle, MemoizeClientHandle};
use hickory_proto::dnssec::{Proof, TrustAnchor};
use hickory_proto::op::ResponseCode;
use hickory_proto::rr::rdata::A;
use hickory_proto::rr::Name;
use hickory_proto::rr::{DNSClass, RData, RecordType};
use hickory_proto::runtime::TokioRuntimeProvider;
use hickory_proto::tcp::TcpClientStream;
use hickory_proto::udp::UdpClientStream;
use hickory_proto::DnssecDnsHandle;
use hickory_server::authority::{Authority, Catalog};

use hickory_integration::example_authority::create_secure_example;
use hickory_integration::{TestClientStream, GOOGLE_V4};

#[test]
fn test_secure_query_example_nonet() {
    with_nonet(test_secure_query_example);
}

#[test]
#[ignore] // this getting finnicky responses with UDP
fn test_secure_query_example_udp() {
    with_udp(test_secure_query_example);
}

#[test]
fn test_secure_query_example_tcp() {
    with_tcp(test_secure_query_example);
}

fn test_secure_query_example<H>(mut client: DnssecDnsHandle<H>, io_loop: Runtime)
where
    H: ClientHandle + Sync + 'static,
{
    let name = Name::from_str("www.example.com").unwrap();
    let response = io_loop
        .block_on(client.query(name.clone(), DNSClass::IN, RecordType::A))
        .expect("query failed");

    println!("response records: {response:?}");
    assert!(
        response
            .extensions()
            .as_ref()
            .expect("edns not here")
            .flags()
            .dnssec_ok
    );

    assert!(!response.answers().is_empty());
    let record = &response.answers()[0];
    assert_eq!(record.name(), &name);
    assert_eq!(record.record_type(), RecordType::A);
    assert_eq!(record.dns_class(), DNSClass::IN);
    assert_eq!(record.proof(), Proof::Secure);

    if let RData::A(address) = *record.data() {
        assert_eq!(address, A::new(93, 184, 215, 14))
    } else {
        panic!();
    }
}

#[test]
fn test_nsec_query_example_nonet() {
    with_nonet(test_nsec_query_example);
}

#[test]
#[ignore]
fn test_nsec_query_example_udp() {
    with_udp(test_nsec_query_example);
}

#[test]
#[ignore]
fn test_nsec_query_example_tcp() {
    with_tcp(test_nsec_query_example);
}

fn test_nsec_query_example<H>(mut client: DnssecDnsHandle<H>, io_loop: Runtime)
where
    H: ClientHandle + Sync + 'static,
{
    let name = Name::from_str("none.example.com").unwrap();

    let response = io_loop
        .block_on(client.query(name, DNSClass::IN, RecordType::A))
        .expect("query failed");
    assert_eq!(response.response_code(), ResponseCode::NXDomain);
}

// TODO: NSEC response code wrong in Hickory DNS? Issue #53
// #[test]
// fn test_nsec_query_type_nonet() {
//   with_nonet(test_nsec_query_type);
// }

#[test]
#[ignore]
fn test_nsec_query_type_udp() {
    with_udp(test_nsec_query_type);
}

#[test]
#[ignore]
fn test_nsec_query_type_tcp() {
    with_tcp(test_nsec_query_type);
}

fn test_nsec_query_type<H>(mut client: DnssecDnsHandle<H>, io_loop: Runtime)
where
    H: ClientHandle + Sync + 'static,
{
    let name = Name::from_str("www.example.com").unwrap();

    let response = io_loop
        .block_on(client.query(name, DNSClass::IN, RecordType::NS))
        .expect("query failed");

    assert_eq!(response.response_code(), ResponseCode::NoError);
    assert!(response.answers().is_empty());
}

// // TODO: this test is flaky
// #[test]
// #[ignore]
// fn test_dnssec_rollernet_td_udp() {
//     with_udp(dnssec_rollernet_td_test);
// }

// // TODO: this test is flaky
// #[test]
// #[ignore]
// fn test_dnssec_rollernet_td_tcp() {
//     with_udp(dnssec_rollernet_td_test);
// }

// // TODO: this test is flaky
// #[test]
// #[ignore]
// fn test_dnssec_rollernet_td_tcp_mixed_case() {
//     with_tcp(dnssec_rollernet_td_mixed_case_test);
// }

// fn dnssec_rollernet_td_test<H>(mut client: DnssecDnsHandle<H>, io_loop: Runtime)
// where
//     H: ClientHandle + 'static,
// {
//     let name = Name::parse("rollernet.us.", None).unwrap();

//     let response = io_loop
//         .block_on(client.query(
//             name.clone(),
//             DNSClass::IN,
//             RecordType::DS),
//         ))
//         .expect("query failed");

//     assert_eq!(response.response_code(), ResponseCode::NoError);
//     // rollernet doesn't have any DS records...
//     //  would have failed validation
//     assert!(response.answers().is_empty());
// }

// fn dnssec_rollernet_td_mixed_case_test<H>(mut client: DnssecDnsHandle<H>, io_loop: Runtime)
// where
//     H: ClientHandle + 'static,
// {
//     let name = Name::parse("RollErnet.Us.", None).unwrap();

//     let response = io_loop
//         .block_on(client.query(
//             name.clone(),
//             DNSClass::IN,
//             RecordType::DS),
//         ))
//         .expect("query failed");

//     assert_eq!(response.response_code(), ResponseCode::NoError);
//     // rollernet doesn't have any DS records...
//     //  would have failed validation
//     assert!(response.answers().is_empty());
// }

fn with_nonet<F>(test: F)
where
    F: Fn(DnssecDnsHandle<MemoizeClientHandle<Client>>, Runtime),
{
    let succeeded = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let succeeded_clone = succeeded.clone();
    let join = std::thread::Builder::new()
        .name("thread_killer".to_string())
        .spawn(move || {
            let succeeded = succeeded_clone;
            for _ in 0..15 {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if succeeded.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
            }

            println!("Thread Killer has been awoken, killing process");
            std::process::exit(-1);
        })
        .unwrap();

    let authority = create_secure_example();

    let trust_anchor = {
        let signers = block_on(authority.secure_keys());
        let public_key = signers
            .first()
            .expect("expected a key in the authority")
            .key()
            .to_public_key()
            .expect("could not convert keypair to public_key");

        let mut trust_anchor = TrustAnchor::new();
        trust_anchor.insert_trust_anchor(&public_key);

        Arc::new(trust_anchor)
    };

    let mut catalog = Catalog::new();
    catalog.upsert(authority.origin().clone(), vec![Arc::new(authority)]);

    let io_loop = Runtime::new().unwrap();
    let (stream, sender) = TestClientStream::new(Arc::new(StdMutex::new(catalog)));
    let client = Client::new(stream, sender, None);

    let (client, bg) = io_loop
        .block_on(client)
        .expect("failed to create new client");

    hickory_proto::runtime::spawn_bg(&io_loop, bg);
    let client = MemoizeClientHandle::new(client);
    let secure_client = DnssecDnsHandle::with_trust_anchor(client, trust_anchor);

    test(secure_client, io_loop);
    succeeded.store(true, std::sync::atomic::Ordering::Relaxed);
    join.join().unwrap();
}

fn with_udp<F>(test: F)
where
    F: Fn(DnssecDnsHandle<MemoizeClientHandle<Client>>, Runtime),
{
    let succeeded = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let succeeded_clone = succeeded.clone();
    let join = std::thread::Builder::new()
        .name("thread_killer".to_string())
        .spawn(move || {
            let succeeded = succeeded_clone;
            for _ in 0..15 {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if succeeded.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
            }

            println!("Thread Killer has been awoken, killing process");
            std::process::exit(-1);
        })
        .unwrap();

    let io_loop = Runtime::new().unwrap();
    let stream = UdpClientStream::builder(GOOGLE_V4, TokioRuntimeProvider::new()).build();
    let client = Client::connect(stream);
    let (client, bg) = io_loop.block_on(client).expect("client failed to connect");
    hickory_proto::runtime::spawn_bg(&io_loop, bg);

    let client = MemoizeClientHandle::new(client);
    let secure_client = DnssecDnsHandle::new(client);

    test(secure_client, io_loop);
    succeeded.store(true, std::sync::atomic::Ordering::Relaxed);
    join.join().unwrap();
}

// TODO: just make this a Tokio test?
fn with_tcp<F>(test: F)
where
    F: Fn(DnssecDnsHandle<MemoizeClientHandle<Client>>, Runtime),
{
    let succeeded = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let succeeded_clone = succeeded.clone();
    let join = std::thread::Builder::new()
        .name("thread_killer".to_string())
        .spawn(move || {
            let succeeded = succeeded_clone;
            for _ in 0..15 {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if succeeded.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
            }

            println!("Thread Killer has been awoken, killing process");
            std::process::exit(-1);
        })
        .unwrap();

    let io_loop = Runtime::new().unwrap();
    let (stream, sender) = TcpClientStream::new(GOOGLE_V4, None, None, TokioRuntimeProvider::new());
    let client = Client::new(Box::new(stream), sender, None);
    let (client, bg) = io_loop.block_on(client).expect("client failed to connect");
    hickory_proto::runtime::spawn_bg(&io_loop, bg);

    let client = MemoizeClientHandle::new(client);
    let secure_client = DnssecDnsHandle::new(client);

    test(secure_client, io_loop);
    succeeded.store(true, std::sync::atomic::Ordering::Relaxed);
    join.join().unwrap();
}
