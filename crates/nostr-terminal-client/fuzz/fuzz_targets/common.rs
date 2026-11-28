// The source modules retain their crate-relative paths. No harness opens a
// connection or loads config: only the parsing functions below are called.
#[allow(dead_code)]
#[path = "../../src/error.rs"]
mod error;
#[allow(dead_code)]
mod config {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}
#[allow(dead_code)]
#[path = "../../src/api/nip11.rs"]
mod nip11;
#[allow(dead_code)]
mod api {
    pub(crate) use super::{event, nip44};
    pub(crate) mod actions {
        include!(concat!(env!("OUT_DIR"), "/actions.rs"));
    }
    pub(crate) mod media {
        include!(concat!(env!("OUT_DIR"), "/media.rs"));
    }
    include!(concat!(env!("OUT_DIR"), "/module.rs"));
}
#[allow(dead_code)]
#[path = "../../src/api/event.rs"]
mod event;
#[allow(dead_code)]
#[path = "../../src/api/nip11.rs"]
mod nip11;
#[allow(dead_code)]
#[path = "../../src/api/nip19.rs"]
mod nip19;
#[allow(dead_code)]
#[path = "../../src/api/nip44.rs"]
mod nip44;
#[allow(dead_code)]
#[path = "../../src/api/relay.rs"]
mod relay;
